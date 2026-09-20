use std::sync::Arc;

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderMap, Request},
    routing::post,
};
use longcut_server::{
    AppState,
    ai::{EvidenceSample, OpenAiProvider, proposal_schema},
    app,
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::sync::{Mutex, mpsc};
use tower::ServiceExt;

struct Captured {
    authorization: String,
    body: Value,
}

#[derive(Clone)]
struct FakeState {
    captured: mpsc::Sender<Captured>,
    response: Arc<Mutex<Value>>,
}

async fn completion(
    State(state): State<FakeState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    state
        .captured
        .send(Captured {
            authorization: headers
                .get("authorization")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned(),
            body,
        })
        .await
        .expect("capture request");
    Json(state.response.lock().await.clone())
}

async fn fake_provider(response: Value) -> (OpenAiProvider, mpsc::Receiver<Captured>) {
    let (sender, receiver) = mpsc::channel(1);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake provider");
    let address = listener.local_addr().expect("fake provider address");
    let router = Router::new()
        .route("/v1/chat/completions", post(completion))
        .with_state(FakeState {
            captured: sender,
            response: Arc::new(Mutex::new(response)),
        });
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve fake provider");
    });
    let provider = OpenAiProvider::from_values([
        ("OPENAI_API_KEY", "dummy-test-key".to_owned()),
        ("OPENAI_BASE_URL", format!("http://{address}/v1")),
        ("OPENAI_MODEL", "vision-fixture".to_owned()),
        ("OPENAI_RESPONSE_FORMAT", "json_schema".to_owned()),
        ("OPENAI_MAX_OUTPUT_TOKENS", "1024".to_owned()),
        ("OPENAI_TOKEN_PARAMETER", "max_completion_tokens".to_owned()),
    ]);
    (provider, receiver)
}

#[tokio::test]
async fn chat_completion_sends_standard_text_images_and_schema() {
    // Given
    let proposal = json!({
        "title": "Fixture",
        "summary": "Safe result",
        "warnings": [],
        "operations": []
    });
    let (provider, mut captured) = fake_provider(json!({
        "choices": [{
            "finish_reason": "stop",
            "message": {"content": proposal.to_string()}
        }]
    }))
    .await;
    let samples = [EvidenceSample {
        evidence_id: "frame_0001".into(),
        source_id: "source".into(),
        frame: 30,
        jpeg: vec![0xff, 0xd8, 0xff, 0xd9],
    }];
    // When
    let result = provider
        .generate("structured context", proposal_schema(), &samples)
        .await
        .expect("generate proposal");
    let request = captured.recv().await.expect("captured request");
    // Then
    assert_eq!(result.title, "Fixture");
    assert_eq!(request.authorization, "Bearer dummy-test-key");
    assert_eq!(request.body["model"], "vision-fixture");
    assert_eq!(request.body["stream"], true);
    assert_eq!(request.body["max_completion_tokens"], 1024);
    assert!(request.body.get("max_tokens").is_none());
    assert_eq!(request.body["messages"][0]["role"], "user");
    assert_eq!(request.body["messages"][0]["content"][0]["type"], "text");
    assert_eq!(
        request.body["messages"][0]["content"][1]["image_url"]["url"],
        "data:image/jpeg;base64,/9j/2Q=="
    );
    assert_eq!(request.body["response_format"]["type"], "json_schema");
    assert_eq!(
        request.body["response_format"]["json_schema"]["strict"],
        true
    );
}

#[tokio::test]
async fn chat_completion_rejects_tool_calls_and_truncation() {
    // Given
    let content =
        json!({"title":"Fixture","summary":"Valid content","warnings":[],"operations":[]})
            .to_string();
    let (tool_provider, _tool_requests) = fake_provider(json!({
        "choices": [{
            "finish_reason": "stop",
            "message": {"content": content, "tool_calls": [{"id": "call"}]}
        }]
    }))
    .await;
    let (truncated_provider, _truncated_requests) = fake_provider(json!({
        "choices": [{
            "finish_reason": "length",
            "message": {"content": content}
        }]
    }))
    .await;
    // When
    let tool_result = tool_provider.generate("context", json!({}), &[]).await;
    let truncated_result = truncated_provider.generate("context", json!({}), &[]).await;
    // Then
    assert!(tool_result.is_err());
    assert!(truncated_result.is_err());
}

#[tokio::test]
async fn providers_route_reports_invalid_configuration_without_contacting_codex() {
    // Given
    let data = TempDir::new().expect("tempdir");
    let state = AppState::initialize_with_providers(
        data.path(),
        Arc::new(NoNetworkCodex),
        OpenAiProvider::from_values([
            ("OPENAI_API_KEY", "dummy-test-key"),
            (
                "OPENAI_BASE_URL",
                "https://user:password@example.test/v1?secret=yes",
            ),
            ("OPENAI_MODEL", "vision-fixture"),
        ]),
    )
    .await
    .expect("initialize state");
    // When
    let response = app(state)
        .oneshot(
            Request::builder()
                .uri("/api/v1/ai/providers")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("provider status request");
    let body: Value = serde_json::from_slice(
        &to_bytes(response.into_body(), 16_384)
            .await
            .expect("provider status body"),
    )
    .expect("provider status JSON");
    // Then
    assert_eq!(body["providers"][0]["id"], "codex");
    assert_eq!(body["providers"][1]["id"], "openai-compatible");
    assert_eq!(body["providers"][1]["configured"], false);
    assert_eq!(body["providers"][1]["endpoint"], Value::Null);
    let serialized = body.to_string();
    assert!(!serialized.contains("dummy-test-key"));
    assert!(!serialized.contains("password"));
    assert!(!serialized.contains("secret=yes"));
}

#[tokio::test]
async fn provider_error_is_rejected_even_when_it_contains_a_valid_choice() {
    let content = json!({"title":"Fixture","summary":"","warnings":[],"operations":[]});
    let (provider, _requests) = fake_provider(json!({
        "error": {"message":"Explicit fixture failure"},
        "choices": [{"finish_reason":"stop", "message":{"content":content.to_string()}}]
    }))
    .await;
    assert!(
        provider
            .generate("context", proposal_schema(), &[])
            .await
            .is_err()
    );
}

struct NoNetworkCodex;

#[async_trait::async_trait]
impl longcut_server::ai::CodexClient for NoNetworkCodex {
    async fn status(&self) -> longcut_server::error::AppResult<longcut_server::ai::CodexStatus> {
        panic!("provider listing must not contact Codex")
    }

    async fn account(
        &self,
        _action: longcut_server::ai::AccountAction,
    ) -> longcut_server::error::AppResult<longcut_server::ai::CodexStatus> {
        panic!("provider listing must not contact Codex")
    }

    async fn generate(
        &self,
        _id: &str,
        _prompt: &str,
        _schema: Value,
        _samples: &[EvidenceSample],
    ) -> longcut_server::error::AppResult<longcut_server::ai::AiProposal> {
        panic!("provider listing must not contact Codex")
    }

    async fn cancel(&self, _id: &str) -> longcut_server::error::AppResult<()> {
        panic!("provider listing must not contact Codex")
    }
}
