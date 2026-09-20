use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use longcut_server::{
    AppState,
    ai::{
        AccountAction, AiProposal, AiProvider, AiRequest, AiScope, CodexAccountState,
        CodexAuthMode, CodexClient, CodexStatus, EvidenceSample,
    },
    error::AppResult,
    model::{CreateProject, MediaStatus, Source, SourceRole},
};
use tempfile::TempDir;
use tower::ServiceExt;

struct FakeCodex;

#[async_trait]
impl CodexClient for FakeCodex {
    async fn status(&self) -> AppResult<CodexStatus> {
        Ok(ready_status())
    }

    async fn account(&self, _action: AccountAction) -> AppResult<CodexStatus> {
        Ok(ready_status())
    }

    async fn generate(
        &self,
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
        panic!("OpenAI-compatible cancellation must not contact Codex")
    }
}

#[tokio::test]
async fn compatible_provider_cancellation_only_drops_its_local_work() {
    // Given
    let data = TempDir::new().expect("create temp directory");
    let state = AppState::initialize_with_codex(data.path(), Arc::new(FakeCodex))
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
}

#[tokio::test]
async fn codex_status_route_uses_explicit_client_boundary() {
    // Given
    let data = TempDir::new().expect("create temp directory");
    let state = AppState::initialize_with_codex(data.path(), Arc::new(FakeCodex))
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
