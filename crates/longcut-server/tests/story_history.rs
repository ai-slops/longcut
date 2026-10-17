mod story_support;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use longcut_server::{
    ai::{
        AccountAction, AgentRuntimeClient, AiProposal, AiProvider, CodexStatus, EvidenceSample,
        ProvidersResponse, RuntimeInfo, RuntimeJob,
    },
    app,
    model::{Command, StoryCommand},
};
use story_support::{chapter, command, fixture, story};
use tower::ServiceExt;

#[tokio::test]
async fn undo_redo_and_rebranch_use_unique_project_revision_versions() {
    // Given
    let fixture = fixture().await;
    let first = chapter(&fixture, &fixture.project, "One").await;
    let chapter_id = first.story.chapters[0].id.clone();
    let second = command(
        &fixture,
        &first,
        story(StoryCommand::UpdateChapter {
            expected_story_version: first.story.version.clone(),
            chapter_id,
            title: "Two".into(),
            purpose: String::new(),
        }),
    )
    .await
    .expect("update chapter");
    let undone = command(&fixture, &second, Command::Undo)
        .await
        .expect("undo");
    let redone = command(&fixture, &undone, Command::Redo)
        .await
        .expect("redo");

    // When
    let branched = command(
        &fixture,
        &redone,
        story(StoryCommand::CreateChapter {
            expected_story_version: redone.story.version.clone(),
            title: "Branch".into(),
            purpose: String::new(),
            before_chapter_id: None,
        }),
    )
    .await
    .expect("branch story");

    // Then
    assert_eq!(first.story.version, "1");
    assert_eq!(second.story.version, "2");
    assert_eq!(undone.story.version, "1");
    assert_eq!(redone.story.version, "2");
    assert_eq!(branched.story.version, "5");
}

#[tokio::test]
async fn story_version_endpoint_returns_exact_immutable_snapshot() {
    // Given
    let fixture = fixture().await;
    let first = chapter(&fixture, &fixture.project, "Frozen").await;
    let chapter_id = first.story.chapters[0].id.clone();
    let current = command(
        &fixture,
        &first,
        story(StoryCommand::UpdateChapter {
            expected_story_version: first.story.version.clone(),
            chapter_id,
            title: "Current".into(),
            purpose: String::new(),
        }),
    )
    .await
    .expect("update story");

    // When
    let response = app(longcut_server::AppState::initialize_with_agent_runtime(
        fixture.root.path(),
        std::sync::Arc::new(TestRuntime),
    )
    .await
    .expect("state"))
    .oneshot(
        Request::get(format!(
            "/api/v1/projects/{}/story/versions/{}",
            current.id, first.story.version
        ))
        .body(Body::empty())
        .expect("request"),
    )
    .await
    .expect("response");
    let body = to_bytes(response.into_body(), 1 << 20).await.expect("body");
    let value: serde_json::Value = serde_json::from_slice(&body).expect("json");

    // Then
    assert_eq!(value["snapshot"]["chapters"][0]["title"], "Frozen");
}

#[tokio::test]
async fn story_version_zero_has_no_synthetic_immutable_record() {
    // Given
    let fixture = fixture().await;
    let state = longcut_server::AppState::initialize_with_agent_runtime(
        fixture.root.path(),
        std::sync::Arc::new(TestRuntime),
    )
    .await
    .expect("state");

    // When
    let response = app(state)
        .oneshot(
            Request::get(format!(
                "/api/v1/projects/{}/story/versions/0",
                fixture.project.id
            ))
            .body(Body::empty())
            .expect("request"),
        )
        .await
        .expect("response");

    // Then
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

struct TestRuntime;

#[async_trait::async_trait]
impl AgentRuntimeClient for TestRuntime {
    async fn codex_status(&self) -> longcut_server::error::AppResult<CodexStatus> {
        Err(longcut_server::error::AppError::Invalid("unused".into()))
    }

    async fn account(&self, _: AccountAction) -> longcut_server::error::AppResult<CodexStatus> {
        Err(longcut_server::error::AppError::Invalid("unused".into()))
    }

    async fn providers(&self) -> longcut_server::error::AppResult<ProvidersResponse> {
        Err(longcut_server::error::AppError::Invalid("unused".into()))
    }

    async fn runtime_info(&self) -> longcut_server::error::AppResult<RuntimeInfo> {
        Err(longcut_server::error::AppError::Invalid("unused".into()))
    }

    async fn job_runtime(&self, _: &str) -> longcut_server::error::AppResult<RuntimeJob> {
        Err(longcut_server::error::AppError::Invalid("unused".into()))
    }

    async fn generate(
        &self,
        _: AiProvider,
        _: &str,
        _: &str,
        _: serde_json::Value,
        _: &[EvidenceSample],
    ) -> longcut_server::error::AppResult<AiProposal> {
        Err(longcut_server::error::AppError::Invalid("unused".into()))
    }

    async fn cancel(&self, _: &str) -> longcut_server::error::AppResult<()> {
        Ok(())
    }
}
