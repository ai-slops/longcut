use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use longcut_server::{
    AppState,
    ai::{
        AccountAction, AgentRuntimeClient, AiProposal, AiProvider, AiRequest, AiScope, CodexStatus,
        EvidenceSample, ProviderStatus, ProvidersResponse, RuntimeInfo, RuntimeJob,
        RuntimeJobStatus, RuntimeMetadata, RuntimePhase, RuntimeProgress,
    },
    app,
    error::{AppError, AppResult},
    model::{CreateProject, MediaStatus, Source, SourceRole},
};
use serde_json::Value;
use tower::ServiceExt;

struct MetadataRuntime {
    observations: Arc<AtomicUsize>,
    generations: Arc<AtomicUsize>,
    configured: bool,
    runtime_mode: RuntimeMode,
    job_profile: Option<&'static str>,
}

#[derive(Clone, Copy)]
enum RuntimeMode {
    Unavailable,
    Invalid,
}

#[async_trait]
impl AgentRuntimeClient for MetadataRuntime {
    async fn codex_status(&self) -> AppResult<CodexStatus> {
        Err(AppError::Conflict("unused".into()))
    }

    async fn account(&self, _action: AccountAction) -> AppResult<CodexStatus> {
        Err(AppError::Conflict("unused".into()))
    }

    async fn providers(&self) -> AppResult<ProvidersResponse> {
        Ok(ProvidersResponse {
            providers: vec![ProviderStatus {
                id: AiProvider::OpenaiCompatible,
                name: "Runtime fixture".into(),
                configured: self.configured,
                model: Some("fixture-model".into()),
                endpoint: None,
                detail: None,
            }],
        })
    }

    async fn runtime_info(&self) -> AppResult<RuntimeInfo> {
        match self.runtime_mode {
            RuntimeMode::Unavailable => Err(AppError::Conflict(
                "private token and provider details must not escape".into(),
            )),
            RuntimeMode::Invalid => Ok(RuntimeInfo {
                engine: "unexpected-engine".into(),
                version: "0.1.0".into(),
                node_version: "24.22.0".into(),
                protocol_version: 1,
            }),
        }
    }

    async fn job_runtime(&self, id: &str) -> AppResult<RuntimeJob> {
        self.observations.fetch_add(1, Ordering::SeqCst);
        let Some(profile_revision) = self.job_profile else {
            return Err(AppError::NotFound);
        };
        Ok(RuntimeJob {
            id: id.into(),
            status: RuntimeJobStatus::Running,
            result: None,
            error: None,
            runtime: RuntimeMetadata {
                engine: "@slop-lab/part".into(),
                engine_version: "0.1.0".into(),
                execution_id: "part-execution".into(),
                profile_revision: profile_revision.into(),
            },
            progress: RuntimeProgress {
                sequence: 1,
                phase: RuntimePhase::Provider,
            },
        })
    }

    async fn generate(
        &self,
        _provider: AiProvider,
        _id: &str,
        _prompt: &str,
        _schema: Value,
        _samples: &[EvidenceSample],
    ) -> AppResult<AiProposal> {
        self.generations.fetch_add(1, Ordering::SeqCst);
        Err(AppError::Conflict("unused".into()))
    }

    async fn cancel(&self, _id: &str) -> AppResult<()> {
        Ok(())
    }
}

async fn state() -> (tempfile::TempDir, AppState, Arc<AtomicUsize>) {
    let (directory, state, observations, _generations) =
        state_with(true, RuntimeMode::Unavailable, None).await;
    (directory, state, observations)
}

async fn state_with(
    configured: bool,
    runtime_mode: RuntimeMode,
    job_profile: Option<&'static str>,
) -> (
    tempfile::TempDir,
    AppState,
    Arc<AtomicUsize>,
    Arc<AtomicUsize>,
) {
    let directory = tempfile::tempdir().expect("tempdir");
    let observations = Arc::new(AtomicUsize::new(0));
    let generations = Arc::new(AtomicUsize::new(0));
    let state = AppState::initialize_with_agent_runtime(
        directory.path(),
        Arc::new(MetadataRuntime {
            observations: Arc::clone(&observations),
            generations: Arc::clone(&generations),
            configured,
            runtime_mode,
            job_profile,
        }),
    )
    .await
    .expect("state");
    (directory, state, observations, generations)
}

async fn persisted_run(state: &AppState, title: &str) -> (String, String) {
    let project = state
        .store
        .create(CreateProject {
            title: title.into(),
            frame_rate: 30,
        })
        .await
        .expect("project");
    let project = state
        .store
        .add_source(
            &project.id,
            Source {
                id: "source".into(),
                name: "source.mp4".into(),
                role: SourceRole::Program,
                duration_frames: "120".into(),
                width: 1280,
                height: 720,
                has_audio: false,
                media_url: "/api/v1/media/source.mp4".into(),
                proxy_url: None,
                thumbnail_url: None,
                waveform: vec![],
                status: MediaStatus::Ready,
                error: None,
            },
        )
        .await
        .expect("source");
    let run = state
        .store
        .enqueue_ai(
            &project.id,
            AiRequest {
                provider: AiProvider::Codex,
                base_revision: project.revision,
                instruction: "Observe safely".into(),
                consent: true,
                scope: AiScope::Source {
                    source_id: "source".into(),
                    start_frame: "0".into(),
                    end_frame: "120".into(),
                },
            },
        )
        .await
        .expect("run");
    (project.id, run.id)
}

async fn project_with_source(state: &AppState, title: &str) -> longcut_server::model::Project {
    let project = state
        .store
        .create(CreateProject {
            title: title.into(),
            frame_rate: 30,
        })
        .await
        .expect("project");
    state
        .store
        .add_source(
            &project.id,
            Source {
                id: "source".into(),
                name: "source.mp4".into(),
                role: SourceRole::Program,
                duration_frames: "120".into(),
                width: 1280,
                height: 720,
                has_audio: false,
                media_url: "/api/v1/media/source.mp4".into(),
                proxy_url: None,
                thumbnail_url: None,
                waveform: vec![],
                status: MediaStatus::Ready,
                error: None,
            },
        )
        .await
        .expect("source")
}

#[tokio::test]
async fn runtime_metadata_failure_is_a_safe_unavailable_response() {
    // Given
    let (_directory, state, _observations) = state().await;
    // When
    let response = app(state)
        .oneshot(
            Request::get("/api/v1/ai/runtime")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let body = to_bytes(response.into_body(), 4096).await.expect("body");
    let value: Value = serde_json::from_slice(&body).expect("json");
    // Then
    assert_eq!(value["available"], false);
    assert_eq!(value["engine"], "@slop-lab/part");
    assert_eq!(value["version"], Value::Null);
    assert_eq!(value["error"], "Agent runtime is unavailable");
    assert!(!String::from_utf8_lossy(&body).contains("private token"));
}

#[tokio::test]
async fn provider_statuses_are_proxied_from_the_runtime() {
    // Given
    let (_directory, state, _observations) = state().await;
    // When
    let response = app(state)
        .oneshot(
            Request::get("/api/v1/ai/providers")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let body = to_bytes(response.into_body(), 4096).await.expect("body");
    let value: Value = serde_json::from_slice(&body).expect("json");
    // Then
    assert_eq!(value["providers"][0]["id"], "openai-compatible");
    assert_eq!(value["providers"][0]["model"], "fixture-model");
}

#[tokio::test]
async fn run_observation_enforces_project_ownership_before_runtime_proxy() {
    // Given
    let (_directory, state, observations) = state().await;
    let (_owner_id, run_id) = persisted_run(&state, "Owner").await;
    let other = state
        .store
        .create(CreateProject {
            title: "Other".into(),
            frame_rate: 30,
        })
        .await
        .expect("other");
    // When
    let response = app(state)
        .oneshot(
            Request::get(format!(
                "/api/v1/projects/{}/ai/runs/{}/runtime",
                other.id, run_id
            ))
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    // Then
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(observations.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn evicted_runtime_job_does_not_delete_the_persisted_run() {
    // Given
    let (_directory, state, observations) = state().await;
    let (project_id, run_id) = persisted_run(&state, "Evicted").await;
    // When
    let response = app(state.clone())
        .oneshot(
            Request::get(format!(
                "/api/v1/projects/{project_id}/ai/runs/{run_id}/runtime"
            ))
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    let body = to_bytes(response.into_body(), 4096).await.expect("body");
    let value: Value = serde_json::from_slice(&body).expect("json");
    // Then
    assert_eq!(value["run_id"], run_id);
    assert_eq!(value["available"], false);
    assert_eq!(value["runtime"], Value::Null);
    assert_eq!(value["progress"], Value::Null);
    assert_eq!(observations.load(Ordering::SeqCst), 1);
    state
        .store
        .ai_run(&project_id, &run_id)
        .await
        .expect("persisted run remains");
}

#[tokio::test]
async fn unconfigured_openai_compatible_provider_is_rejected_before_enqueue() {
    // Given
    let (_directory, state, _observations, generations) =
        state_with(false, RuntimeMode::Unavailable, None).await;
    let project = project_with_source(&state, "Preflight").await;
    let body = serde_json::json!({
        "provider":"openai-compatible",
        "base_revision":project.revision,
        "instruction":"Draft a cut",
        "consent":true,
        "scope":{"type":"source","source_id":"source","start_frame":"0","end_frame":"120"}
    });
    // When
    let response = app(state.clone())
        .oneshot(
            Request::post(format!("/api/v1/projects/{}/ai/runs", project.id))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .expect("request"),
        )
        .await
        .expect("response");
    // Then
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(
        state
            .store
            .list_ai_runs(&project.id)
            .await
            .expect("runs")
            .is_empty()
    );
    assert_eq!(generations.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn invalid_runtime_identity_is_reported_as_unavailable() {
    // Given
    let (_directory, state, _observations, _generations) =
        state_with(true, RuntimeMode::Invalid, None).await;
    // When
    let response = app(state)
        .oneshot(
            Request::get("/api/v1/ai/runtime")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    let body = to_bytes(response.into_body(), 4096).await.expect("body");
    let value: Value = serde_json::from_slice(&body).expect("json");
    // Then
    assert_eq!(value["available"], false);
    assert_eq!(value["version"], Value::Null);
}

#[tokio::test]
async fn per_run_runtime_metadata_is_bound_to_the_persisted_provider() {
    // Given
    let (_directory, state, observations, _generations) = state_with(
        true,
        RuntimeMode::Unavailable,
        Some("longcut-openai-compatible-v1"),
    )
    .await;
    let (project_id, run_id) = persisted_run(&state, "Provider binding").await;
    // When
    let response = app(state)
        .oneshot(
            Request::get(format!(
                "/api/v1/projects/{project_id}/ai/runs/{run_id}/runtime"
            ))
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");
    let body = to_bytes(response.into_body(), 4096).await.expect("body");
    let value: Value = serde_json::from_slice(&body).expect("json");
    // Then
    assert_eq!(value["available"], false);
    assert_eq!(value["runtime"], Value::Null);
    assert_eq!(observations.load(Ordering::SeqCst), 1);
}
