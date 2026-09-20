use std::{convert::Infallible, sync::Arc};

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{Response, header::CONTENT_TYPE},
    routing::post,
};
use bytes::Bytes;
use futures_util::stream;
use longcut_server::ai::OpenAiProvider;
use serde_json::{Value, json};
use tokio::sync::{Mutex, mpsc};

#[derive(Clone)]
struct FakeStream {
    chunks: Arc<Mutex<Vec<Vec<u8>>>>,
    request: mpsc::Sender<Value>,
}

async fn completion(State(state): State<FakeStream>, Json(request): Json<Value>) -> Response<Body> {
    state.request.send(request).await.expect("capture request");
    let chunks = std::mem::take(&mut *state.chunks.lock().await);
    Response::builder()
        .header(CONTENT_TYPE, "text/event-stream")
        .body(Body::from_stream(stream::iter(
            chunks
                .into_iter()
                .map(|chunk| Ok::<_, Infallible>(Bytes::from(chunk))),
        )))
        .expect("stream response")
}

async fn provider(payload: String) -> (OpenAiProvider, mpsc::Receiver<Value>) {
    let bytes = payload.into_bytes();
    let first = bytes.len().min(3);
    let second = bytes.len().min(29);
    let chunks = vec![
        bytes[..first].to_vec(),
        bytes[first..second].to_vec(),
        bytes[second..].to_vec(),
    ];
    let (sender, receiver) = mpsc::channel(1);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake stream");
    let address = listener.local_addr().expect("fake stream address");
    let router = Router::new()
        .route("/v1/chat/completions", post(completion))
        .with_state(FakeStream {
            chunks: Arc::new(Mutex::new(chunks)),
            request: sender,
        });
    tokio::spawn(async move {
        axum::serve(listener, router)
            .await
            .expect("serve fake stream");
    });
    (
        OpenAiProvider::from_values([
            ("OPENAI_API_KEY", "dummy-stream-key".to_owned()),
            ("OPENAI_BASE_URL", format!("http://{address}/v1")),
            ("OPENAI_MODEL", "stream-fixture".to_owned()),
        ]),
        receiver,
    )
}

fn event(data: Value, space: bool) -> String {
    format!("data:{}{}\n\n", if space { " " } else { "" }, data)
}

#[tokio::test]
async fn split_sse_chunks_aggregate_into_one_proposal() {
    // Given
    let payload = [
        event(
            json!({"choices":[{"delta":{"content":"{\"title\":\"Stream\",\"summary\":"},"finish_reason":null}]}),
            false,
        ),
        event(
            json!({"choices":[{"delta":{"content":"\"Complete\",\"warnings\":[],\"operations\":[]}"},"finish_reason":null}]}),
            true,
        ),
        event(
            json!({"choices":[{"delta":{},"finish_reason":"stop"}]}),
            false,
        ),
        "data: [DONE]\n\n".into(),
    ]
    .concat();
    let (provider, mut request) = provider(payload).await;
    // When
    let proposal = provider
        .generate("context", json!({"type":"object"}), &[])
        .await
        .expect("stream proposal");
    let request = request.recv().await.expect("stream request");
    // Then
    assert_eq!(proposal.title, "Stream");
    assert_eq!(proposal.summary, "Complete");
    assert_eq!(request["stream"], true);
}

#[tokio::test]
async fn sse_rejects_incremental_tools_and_non_stop_finish_reasons() {
    // Given
    let tool_payload = [
        event(
            json!({"choices":[{"delta":{"tool_calls":[{"id":"call"}]},"finish_reason":null}]}),
            true,
        ),
        "data: [DONE]\n\n".into(),
    ]
    .concat();
    let truncated_payload = [
        event(
            json!({"choices":[{"delta":{"content":"{}"},"finish_reason":"length"}]}),
            true,
        ),
        "data: [DONE]\n\n".into(),
    ]
    .concat();
    let (tool_provider, _) = provider(tool_payload).await;
    let (truncated_provider, _) = provider(truncated_payload).await;
    // When
    let tool = tool_provider.generate("context", json!({}), &[]).await;
    let truncated = truncated_provider.generate("context", json!({}), &[]).await;
    // Then
    assert!(tool.is_err());
    assert!(truncated.is_err());
}
