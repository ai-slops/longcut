use longcut_session_contract::Conformance;
use std::path::Path;
use tokio::fs;

use super::Service;
use crate::{
    AppState,
    error::{AppError, AppResult},
    model::*,
    store::Store,
};

pub(super) async fn remove_directory(path: &Path) -> AppResult<()> {
    match fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

pub(super) async fn cleanup(
    service: &Service,
    artifacts: &Path,
    store: &Store,
    job: &SessionImportJob,
) -> AppResult<()> {
    match store.prepared_import(&job.project_id, &job.id).await {
        Ok(prepared) => {
            for name in prepared.artifacts {
                if name.starts_with('.') || name.contains('/') || name.contains("..") {
                    return Err(AppError::Invalid("unsafe persisted artifact plan".into()));
                }
                match fs::remove_file(artifacts.join(name)).await {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        Err(AppError::Conflict(_)) => {}
        Err(error) => return Err(error),
    }
    remove_directory(&service.packages.join(&job.id)).await?;
    remove_directory(&service.root.join(&job.id)).await
}

pub(crate) async fn confirm(
    state: &AppState,
    project_id: &str,
    import_id: &str,
    input: ConfirmImport,
) -> AppResult<Project> {
    let job = state.store.get_import(project_id, import_id).await?;
    if job.status == ImportStatus::Confirmed {
        return state.store.get(project_id).await;
    }
    let mut prepared = state.store.prepared_import(project_id, import_id).await?;
    if prepared.artifacts.iter().any(|name| {
        name.is_empty()
            || name.starts_with('.')
            || name.contains("..")
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    }) {
        return Err(AppError::Invalid("unsafe persisted artifact plan".into()));
    }
    if prepared.record.summary.conformance != Conformance::Conformant && !input.allow_degraded {
        return Err(AppError::Invalid(
            "acknowledge degraded capabilities before confirming".into(),
        ));
    }
    let current = state.store.get(project_id).await?;
    if current.revision != input.base_revision {
        return Err(AppError::Stale);
    }
    let publishing = state
        .store
        .transition_import(project_id, import_id, ImportStatus::Publishing, None)
        .await?;
    let root = state.sessions.root.join(import_id);
    let publish = async {
        fs::rename(
            root.join("package"),
            state.sessions.packages.join(import_id),
        )
        .await?;
        for artifact in &prepared.artifacts {
            fs::rename(
                root.join("work").join(artifact),
                state.artifacts.join(artifact),
            )
            .await?;
        }
        prepared.record.summary.imported_at = chrono::Utc::now().to_rfc3339();
        let report_name = format!("{import_id}-session-report.json");
        prepared.record.summary.verification_url = prepared
            .artifacts
            .contains(&report_name)
            .then(|| format!("/api/v1/media/{report_name}"));
        state
            .store
            .commit_import(&publishing, &prepared, &input.base_revision)
            .await
    }
    .await;
    match publish {
        Ok(project) => {
            if let Err(error) = remove_directory(&root).await {
                tracing::warn!(import_id = %job.id, error = ?error, "session_import.staging_cleanup_failed");
            }
            Ok(project)
        }
        Err(error) => {
            cleanup(&state.sessions, &state.artifacts, &state.store, &publishing).await?;
            state
                .store
                .transition_import(
                    project_id,
                    import_id,
                    ImportStatus::Failed,
                    Some(error.to_string()),
                )
                .await?;
            Err(error)
        }
    }
}
