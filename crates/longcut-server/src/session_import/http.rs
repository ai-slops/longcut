use std::collections::HashSet;

use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
};
use tokio::{fs, io::AsyncWriteExt};
use tokio_util::sync::CancellationToken;

use crate::{
    AppState,
    error::{AppError, AppResult},
    model::*,
};

pub async fn upload(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    mut body: Multipart,
) -> AppResult<(StatusCode, Json<SessionImportJob>)> {
    let job = state.store.create_import(&project_id).await?;
    let root = state.sessions.root.join(&job.id).join("package");
    let received = async {
        fs::create_dir_all(&root).await?;
        let mut names = HashSet::new();
        let mut total = 0_u64;
        while let Some(mut field) = body
            .next_field()
            .await
            .map_err(|error| AppError::Invalid(error.to_string()))?
        {
            if field.name() != Some("file") {
                return Err(AppError::Invalid(
                    "package multipart fields must be named file".into(),
                ));
            }
            let name = field
                .file_name()
                .ok_or_else(|| {
                    AppError::Invalid("each package file requires a relative filename".into())
                })?
                .to_string();
            if !safe_path(&name) || !names.insert(name.clone()) || names.len() > 4096 {
                return Err(AppError::Invalid(
                    "unsafe, duplicate or excessive package paths".into(),
                ));
            }
            let path = root.join(&name);
            let parent = path
                .parent()
                .ok_or_else(|| AppError::Invalid("invalid package path".into()))?;
            fs::create_dir_all(parent).await?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .await?;
            while let Some(bytes) = field
                .chunk()
                .await
                .map_err(|error| AppError::Invalid(error.to_string()))?
            {
                total = total
                    .checked_add(
                        u64::try_from(bytes.len())
                            .map_err(|_| AppError::Invalid("upload too large".into()))?,
                    )
                    .ok_or_else(|| AppError::Invalid("upload too large".into()))?;
                if total > 256 * 1024 * 1024 * 1024 {
                    return Err(AppError::Invalid(
                        "package exceeds 256 GiB upload limit".into(),
                    ));
                }
                file.write_all(&bytes).await?;
            }
            file.sync_all().await?;
        }
        if !names.contains("manifest.json") {
            return Err(AppError::Invalid(
                "package root manifest.json is required".into(),
            ));
        }
        Ok::<(), AppError>(())
    }
    .await;
    if let Err(error) = received {
        super::publication::cleanup(&state.sessions, &state.artifacts, &state.store, &job).await?;
        match state
            .store
            .transition_import(
                &project_id,
                &job.id,
                ImportStatus::Failed,
                Some(error.to_string()),
            )
            .await
        {
            Ok(_) | Err(AppError::Conflict(_)) => {}
            Err(save_error) => return Err(save_error),
        }
        return Err(error);
    }
    if state.store.get_import(&project_id, &job.id).await?.status != ImportStatus::Validating {
        super::publication::cleanup(&state.sessions, &state.artifacts, &state.store, &job).await?;
        return Err(AppError::Conflict("upload was cancelled".into()));
    }
    let token = CancellationToken::new();
    state
        .sessions
        .tokens
        .lock()
        .await
        .insert(job.id.clone(), token.clone());
    let worker = state.clone();
    let work_job = job.clone();
    tokio::spawn(async move {
        super::prepare::run(&worker, &work_job, &token).await;
        worker.sessions.tokens.lock().await.remove(&work_job.id);
    });
    Ok((StatusCode::ACCEPTED, Json(job)))
}

fn safe_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains(['\\', ':', '%', '?', '#'])
        && !value.chars().any(char::is_control)
        && value
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

pub async fn list(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Vec<SessionImportJob>>> {
    Ok(Json(state.store.list_imports(&id).await?))
}

pub async fn get(
    State(state): State<AppState>,
    Path((id, import_id)): Path<(String, String)>,
) -> AppResult<Json<SessionImportJob>> {
    Ok(Json(state.store.get_import(&id, &import_id).await?))
}

pub async fn confirm(
    State(state): State<AppState>,
    Path((id, import_id)): Path<(String, String)>,
    Json(input): Json<ConfirmImport>,
) -> AppResult<Json<Project>> {
    super::publication::confirm(&state, &id, &import_id, input)
        .await
        .map(Json)
}

pub async fn cancel(
    State(state): State<AppState>,
    Path((id, import_id)): Path<(String, String)>,
) -> AppResult<Json<SessionImportJob>> {
    let before = state.store.get_import(&id, &import_id).await?;
    let job = state
        .store
        .transition_import(&id, &import_id, ImportStatus::Cancelled, None)
        .await?;
    if let Some(token) = state.sessions.tokens.lock().await.get(&import_id) {
        token.cancel();
    }
    if before.status == ImportStatus::Prepared {
        super::publication::cleanup(&state.sessions, &state.artifacts, &state.store, &job).await?;
    }
    Ok(Json(job))
}

pub async fn session(
    State(state): State<AppState>,
    Path((id, session_id)): Path<(String, String)>,
) -> AppResult<Json<SourceSessionRecord>> {
    state.store.source_session(&id, &session_id).await.map(Json)
}

pub async fn asset(
    State(state): State<AppState>,
    Path((project, id, asset_id)): Path<(String, String, String)>,
    headers: axum::http::HeaderMap,
) -> AppResult<axum::response::Response> {
    uuid::Uuid::parse_str(&id).map_err(|_| AppError::NotFound)?;
    let record = state.store.source_session(&project, &id).await?;
    let asset = record
        .package
        .registry
        .assets
        .iter()
        .find(|asset| asset.asset_id.0 == asset_id)
        .ok_or(AppError::NotFound)?;
    if !safe_path(&asset.uri) {
        return Err(AppError::NotFound);
    }
    let mut path = state.sessions.packages.join(id);
    for part in asset.uri.split('/') {
        path.push(part);
        let metadata = fs::symlink_metadata(&path)
            .await
            .map_err(|_| AppError::NotFound)?;
        if metadata.file_type().is_symlink() {
            return Err(AppError::NotFound);
        }
    }
    crate::media_http::serve_path(path, headers).await
}

pub async fn render_session(
    State(state): State<AppState>,
    Path((project, render_id, session_id)): Path<(String, String, String)>,
) -> AppResult<Json<SourceSessionRecord>> {
    state
        .store
        .render_source_session(&project, &render_id, &session_id)
        .await
        .map(Json)
}
