use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use longcut_server::{
    AppState,
    ai::{
        AccountAction, AiProposal, CodexAccountState, CodexAuthMode, CodexClient, CodexStatus,
        EvidenceSample,
    },
    error::AppResult,
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
        Ok(())
    }
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
