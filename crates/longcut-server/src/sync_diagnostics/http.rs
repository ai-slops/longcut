use axum::{
    Json,
    extract::{Path, State},
};

use crate::{AppState, error::AppResult};

use super::SyncDiagnostics;

pub async fn get(
    State(state): State<AppState>,
    Path((project, session)): Path<(String, String)>,
) -> AppResult<Json<SyncDiagnostics>> {
    let record = state.store.source_session(&project, &session).await?;
    Ok(Json(super::calculate(&record)))
}
