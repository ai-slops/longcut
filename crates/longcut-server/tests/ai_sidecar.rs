use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use longcut_server::{
    AppState,
    ai::{
        AccountAction, AgentRuntimeClient, AiProposal, AiProvider, AiRequest, AiScope,
        CodexAccountState, CodexAuthMode, CodexStatus, EvidenceSample, ProvidersResponse,
        RuntimeInfo, RuntimeJob,
    },
    error::AppResult,
    model::{CreateProject, MediaStatus, Source, SourceRole},
};
use tempfile::TempDir;
use tower::ServiceExt;

struct FakeRuntime {
    cancellations: Arc<AtomicUsize>,
}

#[async_trait]
impl AgentRuntimeClient for FakeRuntime {
    async fn codex_status(&self) -> AppResult<CodexStatus> {
        Ok(ready_status())
    }

    async fn account(&self, _action: AccountAction) -> AppResult<CodexStatus> {
        Ok(ready_status())
    }

    async fn providers(&self) -> AppResult<ProvidersResponse> {
        Ok(ProvidersResponse { providers: vec![] })
    }

    async fn runtime_info(&self) -> AppResult<RuntimeInfo> {
        Ok(RuntimeInfo {
            engine: "@slop-lab/part".into(),
            version: "0.1.0".into(),
            node_version: "24.22.0".into(),
            protocol_version: 1,
        })
    }

    async fn job_runtime(&self, _id: &str) -> AppResult<RuntimeJob> {
        panic!("runtime observation is unused")
    }

    async fn generate(
        &self,
        _provider: AiProvider,
        _id: &str,
        _prompt: &str,
        _schema: serde_json::Value,
        _samples: &[EvidenceSample],
    ) -> AppResult<AiProposal> {
        Ok(AiProposal {
            title: "No changes".into(),
            summary: "No safe changes found".into(),
            warnings: vec![],
            operations: vec![],
        })
    }

    async fn cancel(&self, _id: &str) -> AppResult<()> {
        self.cancellations.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn compatible_provider_cancellation_requests_runtime_cleanup() {
    // Given
    let data = TempDir::new().expect("create temp directory");
    let cancellations = Arc::new(AtomicUsize::new(0));
    let state = AppState::initialize_with_agent_runtime(
        data.path(),
        Arc::new(FakeRuntime {
            cancellations: Arc::clone(&cancellations),
        }),
    )
    .await
    .expect("initialize state");
    let project = state
        .store
        .create(CreateProject {
            title: "Cancellation".into(),
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
                provider: AiProvider::OpenaiCompatible,
                base_revision: project.revision,
                instruction: "Cancel this".into(),
                consent: true,
                scope: AiScope::Source {
                    source_id: "source".into(),
                    start_frame: "0".into(),
                    end_frame: "120".into(),
                },
            },
        )
        .await
        .expect("enqueue API run");
    // When
    let response = longcut_server::app(state.clone())
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/projects/{}/ai/runs/{}/cancel",
                    project.id, run.id
                ))
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("cancel request");
    // Then
    assert!(response.status().is_success());
    assert_eq!(
        state
            .store
            .ai_run(&project.id, &run.id)
            .await
            .expect("cancelled run")
            .status,
        longcut_server::ai::AiRunStatus::Cancelled
    );
    assert_eq!(cancellations.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn codex_status_route_uses_explicit_client_boundary() {
    // Given
    let data = TempDir::new().expect("create temp directory");
    let state = AppState::initialize_with_agent_runtime(
        data.path(),
        Arc::new(FakeRuntime {
            cancellations: Arc::new(AtomicUsize::new(0)),
        }),
    )
    .await
    .expect("initialize state");
    // When
    let response = longcut_server::app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/codex/status")
                .body(Body::empty())
                .expect("build request"),
        )
        .await
        .expect("route request");
    // Then
    assert!(response.status().is_success());
    let status: CodexStatus = serde_json::from_slice(
        &to_bytes(response.into_body(), 4096)
            .await
            .expect("read response"),
    )
    .expect("parse response");
    assert!(status.installed);
    assert!(matches!(status.auth_mode, CodexAuthMode::Chatgpt));
}

fn ready_status() -> CodexStatus {
    CodexStatus {
        installed: true,
        version: Some("0.155.1".into()),
        state: CodexAccountState::Ready,
        auth_mode: CodexAuthMode::Chatgpt,
        login: None,
        error: None,
    }
}
