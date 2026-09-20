use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use super::{AccountAction, AiRequest, AiRun, CodexStatus, job};
use crate::{AppState, error::AppResult};

pub async fn status(State(state): State<AppState>) -> AppResult<Json<CodexStatus>> {
    Ok(Json(state.codex.status().await?))
}

pub async fn login(State(state): State<AppState>) -> AppResult<Json<CodexStatus>> {
    Ok(Json(state.codex.account(AccountAction::Login).await?))
}

pub async fn cancel_login(State(state): State<AppState>) -> AppResult<Json<CodexStatus>> {
    Ok(Json(state.codex.account(AccountAction::CancelLogin).await?))
}

pub async fn logout(State(state): State<AppState>) -> AppResult<Json<CodexStatus>> {
    Ok(Json(state.codex.account(AccountAction::Logout).await?))
}

pub async fn list_runs(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> AppResult<Json<Vec<AiRun>>> {
    state.store.get(&project_id).await?;
    Ok(Json(state.store.list_ai_runs(&project_id).await?))
}

pub async fn create_run(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Json(request): Json<AiRequest>,
) -> AppResult<(StatusCode, Json<AiRun>)> {
    let run = state.store.enqueue_ai(&project_id, request).await?;
    let cancellation = state.register_ai_work(&run.id).await;
    let worker_state = state.clone();
    let worker_run = run.clone();
    tokio::spawn(async move { job::execute(worker_state, worker_run, cancellation).await });
    Ok((StatusCode::ACCEPTED, Json(run)))
}

pub async fn cancel_run(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(String, String)>,
) -> AppResult<Json<AiRun>> {
    let run = state.store.cancel_ai(&project_id, &run_id).await?;
    state.cancel_ai_work(&run_id).await;
    if let Err(error) = state.codex.cancel(&run_id).await {
        tracing::warn!(run_id, error = ?error, "ai.sidecar_cancel_failed");
    }
    Ok(Json(run))
}
