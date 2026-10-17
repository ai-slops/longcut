use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::Redirect,
    routing::{get, post},
};
use longcut_server::ai::{AgentRuntimeClient, AiProvider, EvidenceSample, HttpAgentRuntimeClient};
use serde_json::{Value, json};
use tempfile::TempDir;
use tokio::{
    sync::{Mutex, mpsc},
    time::{Duration, timeout},
};

#[derive(Clone)]
struct RuntimeFixture {
    requests: mpsc::Sender<Value>,
    cancellations: mpsc::Sender<String>,
    polls: Arc<Mutex<u8>>,
}

async fn start_job(
    State(state): State<RuntimeFixture>,
    headers: HeaderMap,
    Json(mut body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    body["test_authorization"] = Value::String(
        headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned(),
    );
    state.requests.send(body).await.expect("capture request");
    (StatusCode::ACCEPTED, Json(running_job()))
}

async fn get_job(State(state): State<RuntimeFixture>, Path(id): Path<String>) -> Json<Value> {
    let mut polls = state.polls.lock().await;
    *polls += 1;
    Json(if *polls == 1 {
        running_job()
    } else {
        json!({
            "id": id,
            "status": "succeeded",
            "result": {"title":"Runtime","summary":"Accepted","warnings":[],"operations":[]},
            "error": null,
            "runtime": runtime_metadata("longcut-openai-compatible-v1"),
            "progress": {"sequence":2,"phase":"terminal"}
        })
    })
}

async fn cancel_job(State(state): State<RuntimeFixture>, Path(id): Path<String>) -> Json<Value> {
    state
        .cancellations
        .send(id.clone())
        .await
        .expect("capture cancellation");
    Json(json!({
        "id": id,
        "status": "cancelled",
        "result": null,
        "error": null,
        "runtime": runtime_metadata("longcut-openai-compatible-v1"),
        "progress": {"sequence":3,"phase":"terminal"}
    }))
}

fn running_job() -> Value {
    json!({
        "id":"00000000-0000-0000-0000-000000000001",
        "status":"running",
        "result":null,
        "error":null,
        "runtime":runtime_metadata("longcut-openai-compatible-v1"),
        "progress":{"sequence":1,"phase":"provider"}
    })
}

fn runtime_metadata(profile_revision: &str) -> Value {
    json!({
        "engine":"@slop-lab/part",
        "engine_version":"0.1.0",
        "execution_id":"part-execution",
        "profile_revision":profile_revision
    })
}

async fn fixture() -> (
    HttpAgentRuntimeClient,
    mpsc::Receiver<Value>,
    mpsc::Receiver<String>,
    TempDir,
) {
    let directory = tempfile::tempdir().expect("tempdir");
    let token_path = directory.path().join("token");
    tokio::fs::write(&token_path, "runtime-test-token")
        .await
        .expect("token");
    let (sender, receiver) = mpsc::channel(1);
    let (cancellation_sender, cancellation_receiver) = mpsc::channel(4);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let app = Router::new()
        .route("/jobs", post(start_job))
        .route("/jobs/{id}", get(get_job))
        .route("/jobs/{id}/cancel", post(cancel_job))
        .with_state(RuntimeFixture {
            requests: sender,
            cancellations: cancellation_sender,
            polls: Arc::new(Mutex::new(0)),
        });
    tokio::spawn(async move { axum::serve(listener, app).await.expect("runtime") });
    (
        HttpAgentRuntimeClient::new(format!("http://{address}"), token_path)
            .expect("runtime client"),
        receiver,
        cancellation_receiver,
        directory,
    )
}

#[tokio::test]
async fn generation_routes_openai_compatible_through_runtime_job_contract() {
    // Given
    let (client, mut requests, mut cancellations, _directory) = fixture().await;
    let samples = [EvidenceSample {
        evidence_id: "frame_0001".into(),
        source_id: "source".into(),
        frame: 30,
        jpeg: vec![0xff, 0xd8, 0xff, 0xd9],
    }];
    // When
    let proposal = client
        .generate(
            AiProvider::OpenaiCompatible,
            "00000000-0000-0000-0000-000000000001",
            "selected context",
            json!({"type":"object"}),
            &samples,
        )
        .await
        .expect("proposal");
    let request = requests.recv().await.expect("runtime request");
    // Then
    assert_eq!(proposal.title, "Runtime");
    assert_eq!(request["provider"], "openai-compatible");
    assert_eq!(request["test_authorization"], "Bearer runtime-test-token");
    assert_eq!(request["images"][0], "/9j/2Q==");
    assert_eq!(request["schema"]["type"], "object");
    assert!(cancellations.try_recv().is_err());
}

#[tokio::test]
async fn structured_runtime_job_error_is_bounded_and_sanitized() {
    // Given
    let directory = tempfile::tempdir().expect("tempdir");
    let token_path = directory.path().join("token");
    tokio::fs::write(&token_path, "runtime-test-token")
        .await
        .expect("token");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let app = Router::new().route(
        "/jobs",
        post(|| async {
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "id":"00000000-0000-0000-0000-000000000002",
                    "status":"failed",
                    "result":null,
                    "error":{"code":"PROVIDER_FAILED\nsecret","message":"Safe message\u{0000}hidden"},
                    "runtime":runtime_metadata("longcut-codex-v1"),
                    "progress":{"sequence":2,"phase":"terminal"}
                })),
            )
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.expect("runtime") });
    let client = HttpAgentRuntimeClient::new(format!("http://{address}"), token_path)
        .expect("runtime client");
    // When
    let error = client
        .generate(
            AiProvider::Codex,
            "00000000-0000-0000-0000-000000000002",
            "context",
            json!({}),
            &[],
        )
        .await
        .expect_err("failed runtime job");
    // Then
    assert_eq!(
        error.to_string(),
        "conflict: PROVIDER_FAILEDsecret: Safe messagehidden"
    );
}

#[tokio::test]
async fn dropping_generation_after_post_cancels_the_expected_remote_job_once() {
    // Given
    let (client, mut requests, mut cancellations, _directory) = fixture().await;
    let generation = tokio::spawn(async move {
        client
            .generate(
                AiProvider::OpenaiCompatible,
                "00000000-0000-0000-0000-000000000001",
                "context",
                json!({}),
                &[],
            )
            .await
    });
    requests.recv().await.expect("runtime request");
    // When
    generation.abort();
    generation.await.expect_err("generation was aborted");
    // Then
    assert_eq!(
        timeout(Duration::from_secs(1), cancellations.recv())
            .await
            .expect("cancellation deadline")
            .expect("remote cancellation"),
        "00000000-0000-0000-0000-000000000001"
    );
    assert!(cancellations.try_recv().is_err());
}

#[tokio::test]
async fn generation_rejects_a_post_response_for_another_job() {
    // Given
    let directory = tempfile::tempdir().expect("tempdir");
    let token_path = directory.path().join("token");
    tokio::fs::write(&token_path, "runtime-test-token")
        .await
        .expect("token");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let app = Router::new().route(
        "/jobs",
        post(|| async {
            (
                StatusCode::ACCEPTED,
                Json(json!({
                    "id":"00000000-0000-0000-0000-000000000099",
                    "status":"succeeded",
                    "result":{"title":"Wrong","summary":"Wrong","warnings":[],"operations":[]},
                    "error":null,
                    "runtime":runtime_metadata("longcut-codex-v1"),
                    "progress":{"sequence":2,"phase":"terminal"}
                })),
            )
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.expect("runtime") });
    let client = HttpAgentRuntimeClient::new(format!("http://{address}"), token_path)
        .expect("runtime client");
    // When
    let result = client
        .generate(
            AiProvider::Codex,
            "00000000-0000-0000-0000-000000000003",
            "context",
            json!({}),
            &[],
        )
        .await;
    // Then
    assert_eq!(
        result.expect_err("identity mismatch").to_string(),
        "conflict: Agent runtime job identity mismatch"
    );
}

#[tokio::test]
async fn cancel_rejects_a_nonterminal_response_for_another_job() {
    // Given
    let directory = tempfile::tempdir().expect("tempdir");
    let token_path = directory.path().join("token");
    tokio::fs::write(&token_path, "runtime-test-token")
        .await
        .expect("token");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let app = Router::new().route(
        "/jobs/{id}/cancel",
        post(|| async {
            Json(json!({
                "id":"00000000-0000-0000-0000-000000000099",
                "status":"running",
                "result":null,
                "error":null,
                "runtime":runtime_metadata("longcut-codex-v1"),
                "progress":{"sequence":2,"phase":"cleaning_up"}
            }))
        }),
    );
    tokio::spawn(async move { axum::serve(listener, app).await.expect("runtime") });
    let client = HttpAgentRuntimeClient::new(format!("http://{address}"), token_path)
        .expect("runtime client");
    // When
    let result = client.cancel("00000000-0000-0000-0000-000000000004").await;
    // Then
    assert_eq!(
        result.expect_err("invalid cancellation").to_string(),
        "conflict: Agent runtime job identity mismatch"
    );
}

#[tokio::test]
async fn poll_failure_cancels_the_expected_remote_job_and_preserves_the_error() {
    // Given
    let directory = tempfile::tempdir().expect("tempdir");
    let token_path = directory.path().join("token");
    tokio::fs::write(&token_path, "runtime-test-token")
        .await
        .expect("token");
    let (cancellation_sender, mut cancellations) = mpsc::channel(1);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let app = Router::new()
        .route(
            "/jobs",
            post(|| async {
                (
                    StatusCode::ACCEPTED,
                    Json(json!({
                        "id":"00000000-0000-0000-0000-000000000005",
                        "status":"running",
                        "result":null,
                        "error":null,
                        "runtime":runtime_metadata("longcut-codex-v1"),
                        "progress":{"sequence":1,"phase":"provider"}
                    })),
                )
            }),
        )
        .route(
            "/jobs/{id}",
            get(|| async { Json(json!({"invalid":true})) }),
        )
        .route(
            "/jobs/{id}/cancel",
            post({
                let cancellation_sender = cancellation_sender.clone();
                move |Path(id): Path<String>| {
                    let cancellation_sender = cancellation_sender.clone();
                    async move {
                        cancellation_sender
                            .send(id.clone())
                            .await
                            .expect("capture cancellation");
                        Json(json!({
                            "id":id,
                            "status":"cancelled",
                            "result":null,
                            "error":null,
                            "runtime":runtime_metadata("longcut-codex-v1"),
                            "progress":{"sequence":2,"phase":"terminal"}
                        }))
                    }
                }
            }),
        );
    tokio::spawn(async move { axum::serve(listener, app).await.expect("runtime") });
    let client = HttpAgentRuntimeClient::new(format!("http://{address}"), token_path)
        .expect("runtime client");
    // When
    let result = client
        .generate(
            AiProvider::Codex,
            "00000000-0000-0000-0000-000000000005",
            "context",
            json!({}),
            &[],
        )
        .await;
    // Then
    assert_eq!(
        result.expect_err("invalid poll response").to_string(),
        "conflict: Agent runtime returned an invalid response"
    );
    assert_eq!(
        timeout(Duration::from_secs(1), cancellations.recv())
            .await
            .expect("cancellation deadline")
            .expect("remote cancellation"),
        "00000000-0000-0000-0000-000000000005"
    );
}

#[tokio::test]
async fn runtime_client_rejects_redirects_without_forwarding_authentication() {
    // Given
    let reached = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let app = Router::new()
        .route("/status", get(|| async { Redirect::temporary("/sink") }))
        .route(
            "/sink",
            get({
                let reached = Arc::clone(&reached);
                move || async move {
                    reached.store(true, std::sync::atomic::Ordering::SeqCst);
                    Json(json!({}))
                }
            }),
        );
    tokio::spawn(async move { axum::serve(listener, app).await.expect("runtime") });
    let directory = tempfile::tempdir().expect("tempdir");
    let token_path = directory.path().join("token");
    tokio::fs::write(&token_path, "private-token")
        .await
        .expect("token");
    let client =
        HttpAgentRuntimeClient::new(format!("http://{address}"), token_path).expect("client");
    // When
    let result = client.codex_status().await;
    // Then
    assert!(result.is_err());
    assert!(!reached.load(std::sync::atomic::Ordering::SeqCst));
}
