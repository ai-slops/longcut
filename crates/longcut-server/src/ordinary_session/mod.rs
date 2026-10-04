mod probe;

use std::{collections::BTreeMap, path::PathBuf};

use chrono::Utc;
use longcut_session_contract::{
    Conformance,
    model::{
        Asset, AssetId, Contract, Duration, Integer, Manifest, Package, PackageState, Registry,
        Source as PackageSource, SourceId,
    },
};
use tokio::fs;

use crate::{
    AppState,
    error::{AppError, AppResult},
    media,
    model::{
        AssetAvailability, ConfirmImport, ImportStatus, PreparedImport, Project, SessionAsset,
        SessionImportJob, SessionSource, SessionWarning, Source, SourceRole, SourceSessionRecord,
        SourceSessionSummary,
    },
};

struct Context<'a> {
    state: &'a AppState,
    project: &'a Project,
    source: &'a Source,
    job: &'a SessionImportJob,
    artifact: String,
    root: PathBuf,
}

pub(crate) async fn register(
    state: &AppState,
    project: &Project,
    source: &Source,
) -> AppResult<Project> {
    let artifact = media::artifact_name(&source.media_url)?.to_string();
    let job = state.store.create_import(&project.id).await?;
    let context = Context {
        state,
        project,
        source,
        root: state.sessions.root.join(&job.id),
        job: &job,
        artifact,
    };
    let result = execute(&context).await;
    if result.is_err() {
        let cleanup = cleanup(&context).await;
        let transition = state
            .store
            .transition_import(
                &project.id,
                &job.id,
                ImportStatus::Failed,
                result.as_ref().err().map(ToString::to_string),
            )
            .await;
        cleanup?;
        transition?;
    }
    result
}

async fn execute(context: &Context<'_>) -> AppResult<Project> {
    let package_root = context.root.join("package");
    let work = context.root.join("work");
    fs::create_dir_all(package_root.join("original")).await?;
    fs::create_dir_all(&work).await?;
    let work_original = work.join(&context.artifact);
    fs::rename(
        context.state.artifacts.join(&context.artifact),
        &work_original,
    )
    .await?;
    let package_original = package_root.join("original").join(&context.artifact);
    if fs::hard_link(&work_original, &package_original)
        .await
        .is_err()
    {
        fs::copy(&work_original, &package_original).await?;
    }
    let asset_id = AssetId(uuid::Uuid::new_v4().to_string());
    let inspected = probe::inspect(&work_original, &asset_id).await?;
    let package = package(context, asset_id, inspected).await?;
    let validation = longcut_session_contract::validate(&package);
    if validation.metadata_conformance == Conformance::Rejected {
        return Err(AppError::Invalid(format!(
            "internally generated provenance is invalid: {:?}",
            validation.diagnostics
        )));
    }
    fs::write(
        package_root.join("manifest.json"),
        serde_json::to_vec_pretty(&package.manifest)?,
    )
    .await?;
    fs::write(
        package_root.join("sources.json"),
        serde_json::to_vec_pretty(&package.registry)?,
    )
    .await?;
    let report_name = format!("{}-session-report.json", context.job.id);
    fs::write(
        work.join(&report_name),
        serde_json::to_vec_pretty(&validation)?,
    )
    .await?;
    let mut warnings = vec![SessionWarning {
        code: "ORDINARY_PROBE_ONLY".into(),
        path: package.registry.assets[0].uri.clone(),
        message: "ordinary media has probe-derived stream identity but no exact session TimeMaps"
            .into(),
    }];
    warnings.extend(
        validation
            .diagnostics
            .iter()
            .map(|diagnostic| SessionWarning {
                code: diagnostic.code.into(),
                path: diagnostic.path.clone(),
                message: diagnostic.message.clone(),
            }),
    );
    let package_source = &package.registry.sources[0];
    let prepared = PreparedImport {
        record: SourceSessionRecord {
            summary: SourceSessionSummary {
                id: context.job.id.clone(),
                session_id: package.manifest.session_id.clone(),
                title: context.source.name.clone(),
                duration_ns: package.manifest.duration.ticks.get().to_string(),
                conformance: Conformance::Degraded,
                imported_at: Utc::now().to_rfc3339(),
                assets: vec![SessionAsset {
                    asset_id: package.registry.assets[0].asset_id.0.clone(),
                    uri: package.registry.assets[0].uri.clone(),
                    availability: AssetAvailability::Online,
                }],
                sources: vec![SessionSource {
                    source_id: package_source.source_id.0.clone(),
                    display_name: package_source.display_name.clone(),
                    semantic_role: package_source.semantic_role.clone(),
                    kind: package_source.kind,
                    editor_source_id: Some(context.source.id.clone()),
                    reason: None,
                }],
                warnings,
                verification_url: None,
            },
            package,
            mappings: vec![],
        },
        sources: vec![context.source.clone()],
        artifacts: vec![context.artifact.clone(), report_name],
    };
    context
        .state
        .store
        .prepare_import(context.job, &prepared)
        .await?;
    crate::session_import::confirm_prepared(
        context.state,
        &context.project.id,
        &context.job.id,
        ConfirmImport {
            base_revision: context.project.revision.clone(),
            allow_degraded: true,
        },
    )
    .await
}

async fn package(
    context: &Context<'_>,
    asset_id: AssetId,
    inspected: probe::MediaProbe,
) -> AppResult<Package> {
    let size = context
        .root
        .join("package/original")
        .join(&context.artifact)
        .metadata()?
        .len();
    let size =
        i64::try_from(size).map_err(|_| AppError::Invalid("ordinary media is too large".into()))?;
    let source_id = SourceId(context.source.id.clone());
    let program_reference_source_id =
        (context.source.role == SourceRole::Program).then_some(source_id.clone());
    let semantic_role = match context.source.role {
        SourceRole::Program => "video.program_reference",
        SourceRole::Primary => "video.primary",
        SourceRole::Camera => "video.camera",
        SourceRole::Audio => "audio.primary",
    };
    Ok(Package {
        manifest: Manifest {
            contract: Contract::EditSessionPackage,
            version: "0.1.0".into(),
            package_state: PackageState::Complete,
            session_id: context.job.id.clone(),
            duration: Duration {
                ticks: Integer::new(inspected.duration_ns),
                timescale: Integer::new(1_000_000_000),
            },
            sources_file: "sources.json".into(),
            program_reference_source_id,
            time_maps_file: None,
            visual_composition_file: None,
            audio_mix_file: None,
            media_health_file: None,
            extensions: BTreeMap::from([(
                "org.longcut.ordinary".into(),
                serde_json::json!({"probe_only": true}),
            )]),
        },
        registry: Registry {
            sources: vec![PackageSource {
                source_id,
                display_name: context.source.name.clone(),
                semantic_role: semantic_role.into(),
                kind: inspected.kind,
                media_bindings: inspected.bindings,
            }],
            assets: vec![Asset {
                asset_id,
                uri: format!("original/{}", context.artifact),
                size_bytes: Some(Integer::new(size)),
                content_hash: None,
                streams: inspected.streams,
            }],
            extensions: BTreeMap::new(),
        },
        time_maps: None,
        visual_composition: None,
        audio_mix: None,
        media_health: None,
    })
}

async fn cleanup(context: &Context<'_>) -> AppResult<()> {
    match fs::remove_file(context.state.artifacts.join(&context.artifact)).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    for path in [
        context.state.sessions.packages.join(&context.job.id),
        context.root.clone(),
    ] {
        match fs::remove_dir_all(path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
