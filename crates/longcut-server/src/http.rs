use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    routing::{get, post},
};
use tower_http::trace::TraceLayer;

use crate::{
    AppState,
    error::AppResult,
    media,
    model::{CommandRequest, CreateProject, Project, ProjectSummary},
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .nest(
            "/api/v1",
            Router::new()
                .route("/health", get(health))
                .route("/projects", get(list_projects).post(create_project))
                .route("/projects/{id}", get(get_project))
                .route("/projects/{id}/commands", post(command))
                .route(
                    "/projects/{id}/imports",
                    post(crate::media_http::import).layer(DefaultBodyLimit::disable()),
                )
                .route("/projects/{id}/demo", post(crate::media_http::demo))
                .route(
                    "/projects/{id}/renders",
                    post(crate::media_http::create_render),
                )
                .route("/media/{artifact}", get(crate::media_http::serve_artifact)),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health(State(state): State<AppState>) -> Json<serde_json::Value> {
    let (ffmpeg, ffprobe) = tokio::join!(
        media::tool_available("ffmpeg"),
        media::tool_available("ffprobe")
    );
    Json(
        serde_json::json!({"status": if state.store.healthy().await && ffmpeg && ffprobe { "ok" } else { "degraded" },
        "database": state.store.healthy().await, "ffmpeg": ffmpeg, "ffprobe": ffprobe}),
    )
}

async fn list_projects(State(state): State<AppState>) -> AppResult<Json<Vec<ProjectSummary>>> {
    Ok(Json(state.store.list().await?))
}
async fn create_project(
    State(state): State<AppState>,
    Json(input): Json<CreateProject>,
) -> AppResult<Json<Project>> {
    Ok(Json(state.store.create(input).await?))
}
async fn get_project(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Project>> {
    Ok(Json(state.store.get(&id).await?))
}
async fn command(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<CommandRequest>,
) -> AppResult<Json<Project>> {
    uuid::Uuid::parse_str(&input.command_id)
        .map_err(|_| crate::error::AppError::Invalid("command_id must be a UUID".into()))?;
    Ok(Json(state.store.command(&id, &input).await?))
}
