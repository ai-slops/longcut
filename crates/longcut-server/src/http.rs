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
    model::{
        CommandRequest, CreateProject, MomentVersionRecord, Project, ProjectSummary,
        StoryVersionRecord,
    },
};

pub fn router(state: AppState) -> Router {
    Router::new()
        .nest(
            "/api/v1",
            Router::new()
                .route("/health", get(health))
                .route("/asr/status", get(crate::asr::http::status))
                .route("/ai/providers", get(crate::ai::http::providers))
                .route("/codex/status", get(crate::ai::http::status))
                .route("/codex/login", post(crate::ai::http::login))
                .route("/codex/login/cancel", post(crate::ai::http::cancel_login))
                .route("/codex/logout", post(crate::ai::http::logout))
                .route("/projects", get(list_projects).post(create_project))
                .route("/projects/{id}", get(get_project))
                .route("/projects/{id}/commands", post(command))
                .route(
                    "/projects/{id}/moments/{moment_id}/versions/{version}",
                    get(moment_version),
                )
                .route(
                    "/projects/{id}/story/versions/{version}",
                    get(story_version),
                )
                .route(
                    "/projects/{id}/ai/runs",
                    get(crate::ai::http::list_runs).post(crate::ai::http::create_run),
                )
                .route(
                    "/projects/{id}/ai/runs/{run_id}/cancel",
                    post(crate::ai::http::cancel_run),
                )
                .route(
                    "/projects/{id}/imports",
                    post(crate::media_http::import).layer(DefaultBodyLimit::disable()),
                )
                .route("/projects/{id}/demo", post(crate::media_http::demo))
                .route(
                    "/projects/{id}/session-imports",
                    get(crate::session_import::list)
                        .post(crate::session_import::upload)
                        .layer(DefaultBodyLimit::disable()),
                )
                .route(
                    "/projects/{id}/session-imports/{import_id}",
                    get(crate::session_import::get),
                )
                .route(
                    "/projects/{id}/session-imports/{import_id}/confirm",
                    post(crate::session_import::confirm),
                )
                .route(
                    "/projects/{id}/session-imports/{import_id}/cancel",
                    post(crate::session_import::cancel),
                )
                .route(
                    "/projects/{id}/source-sessions/{session_id}",
                    get(crate::session_import::session),
                )
                .route(
                    "/projects/{id}/source-sessions/{session_id}/sync-diagnostics",
                    get(crate::sync_diagnostics::http::get),
                )
                .route(
                    "/projects/{id}/source-sessions/{session_id}/assets/{asset_id}",
                    get(crate::session_import::asset),
                )
                .route(
                    "/projects/{id}/renders/{render_id}/source-sessions/{session_id}",
                    get(crate::session_import::render_session),
                )
                .route(
                    "/projects/{id}/sources/{source_id}/transcripts",
                    post(crate::transcript::import),
                )
                .route(
                    "/projects/{id}/sources/{source_id}/transcript",
                    get(crate::transcript::get),
                )
                .route(
                    "/projects/{id}/sources/{source_id}/asr",
                    post(crate::asr::http::start),
                )
                .route(
                    "/projects/{id}/sources/{source_id}/asr/{job_id}",
                    get(crate::asr::http::get),
                )
                .route(
                    "/projects/{id}/sources/{source_id}/asr/{job_id}/cancel",
                    post(crate::asr::http::cancel),
                )
                .route(
                    "/projects/{id}/sources/{source_id}/asr/{job_id}/resume",
                    post(crate::asr::http::resume),
                )
                .route(
                    "/projects/{id}/renders",
                    post(crate::media_http::create_render),
                )
                .route("/media/{artifact}", get(crate::media_http::serve_artifact)),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn story_version(
    State(state): State<AppState>,
    Path((project_id, version)): Path<(String, String)>,
) -> AppResult<Json<StoryVersionRecord>> {
    version
        .parse::<u64>()
        .map_err(|_| crate::error::AppError::Invalid("story version must be decimal".into()))?;
    Ok(Json(
        state
            .store
            .story_version(crate::store::StoryVersionKey {
                project_id: &project_id,
                version: &version,
            })
            .await?,
    ))
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

async fn moment_version(
    State(state): State<AppState>,
    Path((project_id, moment_id, version)): Path<(String, String, String)>,
) -> AppResult<Json<MomentVersionRecord>> {
    version
        .parse::<u64>()
        .map_err(|_| crate::error::AppError::Invalid("moment version must be decimal".into()))?;
    Ok(Json(
        state
            .store
            .moment_version(crate::store::MomentVersionKey {
                project_id: &project_id,
                moment_id: &moment_id,
                version: &version,
            })
            .await?,
    ))
}
