use chrono::Utc;
use longcut_session_contract::{Conformance, model::Package};
use longcut_session_media::{Check, DecodePolicy, Options};
use std::{path::Path, time::Duration};
use tokio::{fs, sync::SemaphorePermit};
use tokio_util::sync::CancellationToken;

use crate::{
    AppState,
    error::{AppError, AppResult},
    media,
    model::*,
};

pub(super) struct Context<'a> {
    pub package: &'a Package,
    pub root: &'a Path,
    pub work: &'a Path,
    pub frame_rate: u32,
    pub token: &'a CancellationToken,
}

pub(super) async fn run(state: &AppState, job: &SessionImportJob, token: &CancellationToken) {
    let result = execute(state, job, token).await;
    if let Err(error) = result {
        if let Err(cleanup_error) =
            super::publication::cleanup(&state.sessions, &state.artifacts, &state.store, job).await
        {
            tracing::error!(import_id = %job.id, error = ?cleanup_error, "session_import.cleanup_failed");
        }
        match state.store.get_import(&job.project_id, &job.id).await {
            Ok(current) if current.status == ImportStatus::Cancelled => {}
            _ => {
                if let Err(save_error) = state
                    .store
                    .transition_import(
                        &job.project_id,
                        &job.id,
                        ImportStatus::Failed,
                        Some(error.to_string()),
                    )
                    .await
                {
                    tracing::error!(import_id = %job.id, error = ?save_error, "session_import.failure_persist_failed");
                }
            }
        }
    }
}

async fn execute(
    state: &AppState,
    job: &SessionImportJob,
    token: &CancellationToken,
) -> AppResult<()> {
    let _permit: SemaphorePermit<'_> = tokio::select! {
        _ = token.cancelled() => return Err(AppError::Conflict("import cancelled".into())),
        permit = state.sessions.capacity.acquire() => permit.map_err(|_| AppError::Internal(anyhow::anyhow!("import scheduler closed")))?,
    };
    let root = state.sessions.root.join(&job.id).join("package");
    let work = state.sessions.root.join(&job.id).join("work");
    fs::create_dir_all(&work).await?;
    let load_root = root.clone();
    let loaded = tokio::task::spawn_blocking(move || longcut_session_contract::load(&load_root))
        .await
        .map_err(|error| AppError::Internal(error.into()))?
        .map_err(|report| {
            AppError::Invalid(format!(
                "invalid session metadata: {:?}",
                report.diagnostics
            ))
        })?;
    let (package, _) = loaded.into_parts();
    let options = Options {
        decode: DecodePolicy::Full,
        asset_timeout: Duration::from_secs(6 * 60 * 60),
        probe_timeout: Duration::from_secs(600),
        cancellation: token.clone(),
        ..Options::default()
    };
    let verification = longcut_session_media::verify(&root, &options).await;
    if token.is_cancelled() {
        return Err(AppError::Conflict("import cancelled".into()));
    }
    if verification.preflight_conformance == Conformance::Rejected {
        return Err(AppError::Invalid(format!(
            "session media verification rejected: {:?}",
            verification.assets
        )));
    }
    let project = state.store.get(&job.project_id).await?;
    let mut summary = SourceSessionSummary {
        id: job.id.clone(),
        session_id: package.manifest.session_id.clone(),
        title: package.manifest.session_id.clone(),
        duration_ns: package
            .manifest
            .duration
            .nanoseconds()
            .map_err(|error| AppError::Invalid(error.to_string()))?
            .to_string(),
        conformance: verification.preflight_conformance,
        imported_at: Utc::now().to_rfc3339(),
        assets: vec![],
        sources: vec![],
        warnings: vec![],
        verification_url: None,
    };
    for diagnostic in &verification.metadata.diagnostics {
        summary.warnings.push(SessionWarning {
            code: diagnostic.code.into(),
            path: diagnostic.path.clone(),
            message: diagnostic.message.clone(),
        });
    }
    for asset in &package.registry.assets {
        let result = verification
            .assets
            .iter()
            .find(|result| result.asset_id == asset.asset_id);
        summary.assets.push(SessionAsset {
            asset_id: asset.asset_id.0.clone(),
            uri: asset.uri.clone(),
            availability: if result.is_some_and(|result| result.file == Check::Passed) {
                AssetAvailability::Online
            } else {
                AssetAvailability::Offline
            },
        });
        if let Some(result) = result {
            for issue in &result.issues {
                summary.warnings.push(SessionWarning {
                    code: issue.code.into(),
                    path: asset.uri.clone(),
                    message: issue.message.clone(),
                });
            }
        }
    }
    let mut sources = Vec::new();
    let mut mappings = Vec::new();
    let mut artifacts = Vec::new();
    let context = Context {
        package: &package,
        root: &root,
        work: &work,
        frame_rate: project.frame_rate,
        token,
    };
    for source in &package.registry.sources {
        if token.is_cancelled() {
            return Err(AppError::Conflict("import cancelled".into()));
        }
        let verified = source.media_bindings.iter().all(|binding| {
            verification.assets.iter().any(|result| {
                result.asset_id == binding.asset_id
                    && result.file == Check::Passed
                    && result.full_decode == Check::Passed
                    && result.stream_metadata == Check::Passed
                    && result.time_map_bounds == Check::Passed
                    && result.packet_coverage == Check::Passed
                    && result.issues.is_empty()
            })
        });
        let adapted = if verified {
            super::adapter::build(&context, source).await
        } else {
            Err(AppError::Invalid(
                "source media/timing verification is incomplete; original bindings are preserved"
                    .into(),
            ))
        };
        let (editor_source_id, reason) = match adapted {
            Ok((source, mapping)) => {
                for url in [
                    Some(&source.media_url),
                    source.proxy_url.as_ref(),
                    source.thumbnail_url.as_ref(),
                ]
                .into_iter()
                .flatten()
                {
                    artifacts.push(media::artifact_name(url)?.to_string());
                }
                let id = source.id.clone();
                sources.push(source);
                mappings.push(mapping);
                (Some(id), None)
            }
            Err(error) => {
                summary.conformance = Conformance::Degraded;
                summary.warnings.push(SessionWarning {
                    code: "EDITOR_ADAPTER_UNAVAILABLE".into(),
                    path: source.source_id.0.clone(),
                    message: error.to_string(),
                });
                (None, Some(error.to_string()))
            }
        };
        summary.sources.push(SessionSource {
            source_id: source.source_id.0.clone(),
            display_name: source.display_name.clone(),
            semantic_role: source.semantic_role.clone(),
            kind: source.kind,
            editor_source_id,
            reason,
        });
    }
    let report_name = format!("{}-session-report.json", job.id);
    fs::write(
        work.join(&report_name),
        serde_json::to_vec_pretty(&verification)?,
    )
    .await?;
    artifacts.push(report_name);
    let prepared = PreparedImport {
        record: SourceSessionRecord {
            summary,
            package,
            mappings,
        },
        sources,
        artifacts,
    };
    state.store.prepare_import(job, &prepared).await
}
