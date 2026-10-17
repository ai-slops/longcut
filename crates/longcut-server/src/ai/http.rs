use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};

use super::{
    AccountAction, AiProvider, AiRequest, AiRun, CodexStatus, ProvidersResponse,
    RunRuntimeAvailability, RuntimeAvailability, job, runtime_contract::validate_runtime_info,
};
use crate::{
    AppState,
    error::{AppError, AppResult},
};

pub async fn providers(State(state): State<AppState>) -> AppResult<Json<ProvidersResponse>> {
    Ok(Json(state.agent_runtime.providers().await?))
}

pub async fn status(State(state): State<AppState>) -> AppResult<Json<CodexStatus>> {
    Ok(Json(state.agent_runtime.codex_status().await?))
}

pub async fn login(State(state): State<AppState>) -> AppResult<Json<CodexStatus>> {
    Ok(Json(
        state.agent_runtime.account(AccountAction::Login).await?,
    ))
}

pub async fn cancel_login(State(state): State<AppState>) -> AppResult<Json<CodexStatus>> {
    Ok(Json(
        state
            .agent_runtime
            .account(AccountAction::CancelLogin)
            .await?,
    ))
}

pub async fn logout(State(state): State<AppState>) -> AppResult<Json<CodexStatus>> {
    Ok(Json(
        state.agent_runtime.account(AccountAction::Logout).await?,
    ))
}

pub async fn runtime(State(state): State<AppState>) -> Json<RuntimeAvailability> {
    Json(
        match state
            .agent_runtime
            .runtime_info()
            .await
            .and_then(validate_runtime_info)
        {
            Ok(info) => RuntimeAvailability {
                available: true,
                engine: info.engine,
                version: Some(info.version),
                error: None,
            },
            Err(_) => RuntimeAvailability {
                available: false,
                engine: "@slop-lab/part".into(),
                version: None,
                error: Some("Agent runtime is unavailable".into()),
            },
        },
    )
}

pub async fn run_runtime(
    State(state): State<AppState>,
    Path((project_id, run_id)): Path<(String, String)>,
) -> AppResult<Json<RunRuntimeAvailability>> {
    let run = state.store.ai_run(&project_id, &run_id).await?;
    let response = match state
        .agent_runtime
        .job_runtime_for(run.provider, &run_id)
        .await
    {
        Ok(job) => RunRuntimeAvailability {
            run_id,
            available: true,
            runtime: Some(job.runtime),
            progress: Some(job.progress),
        },
        Err(_) => RunRuntimeAvailability {
            run_id,
            available: false,
            runtime: None,
            progress: None,
        },
    };
    Ok(Json(response))
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
    if request.provider == AiProvider::OpenaiCompatible {
        let configured = state
            .agent_runtime
            .providers()
            .await?
            .providers
            .into_iter()
            .any(|provider| provider.id == AiProvider::OpenaiCompatible && provider.configured);
        if !configured {
            return Err(AppError::Conflict(
                "OpenAI-compatible provider is not configured".into(),
            ));
        }
    }
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
    if let Err(error) = state.agent_runtime.cancel_for(run.provider, &run_id).await {
        tracing::warn!(run_id, error = ?error, "ai.runtime_cancel_failed");
    }
    Ok(Json(run))
}
