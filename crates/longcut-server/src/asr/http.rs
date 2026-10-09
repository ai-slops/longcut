use axum::{
    Json,
    extract::rejection::JsonRejection,
    extract::{Path, State},
    http::StatusCode,
};

use crate::{AppState, error::AppResult};

use super::{AsrJob, AsrRuntimeStatus, StartAsr};

pub async fn status(State(state): State<AppState>) -> Json<AsrRuntimeStatus> {
    Json(state.asr.status().await)
}

pub async fn start(
    State(state): State<AppState>,
    Path((project_id, source_id)): Path<(String, String)>,
    payload: Result<Json<StartAsr>, JsonRejection>,
) -> AppResult<(StatusCode, Json<AsrJob>)> {
    let Json(request) = payload.map_err(|error| {
        crate::error::AppError::Invalid(format!("invalid ASR request: {}", error.body_text()))
    })?;
    let job = state
        .asr
        .enqueue(&state, &project_id, &source_id, request)
        .await?;
    Ok((StatusCode::ACCEPTED, Json(job)))
}

pub async fn get(
    State(state): State<AppState>,
    Path((project_id, source_id, job_id)): Path<(String, String, String)>,
) -> AppResult<Json<AsrJob>> {
    state
        .store
        .get_asr(&project_id, &source_id, &job_id)
        .await
        .map(Json)
}

pub async fn cancel(
    State(state): State<AppState>,
    Path((project_id, source_id, job_id)): Path<(String, String, String)>,
) -> AppResult<Json<AsrJob>> {
    let job = state
        .store
        .cancel_asr(&project_id, &source_id, &job_id)
        .await?;
    state.asr.cancel(&job_id).await;
    Ok(Json(job))
}

pub async fn resume(
    State(state): State<AppState>,
    Path((project_id, source_id, job_id)): Path<(String, String, String)>,
) -> AppResult<(StatusCode, Json<AsrJob>)> {
    let job = state
        .asr
        .resume(&state, &project_id, &source_id, &job_id)
        .await?;
    Ok((StatusCode::ACCEPTED, Json(job)))
}
