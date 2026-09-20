use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{Json, Router, extract::State, response::Redirect, routing::post};
use longcut_server::ai::{OpenAiProvider, proposal_schema};
use serde_json::{Value, json};

async fn sink(State(reached): State<Arc<AtomicBool>>) -> Json<Value> {
    reached.store(true, Ordering::SeqCst);
    Json(
        json!({"choices":[{"finish_reason":"stop","message":{"content":
            json!({"title":"Fixture","summary":"","warnings":[],"operations":[]}).to_string()
        }}]}),
    )
}

#[tokio::test]
async fn selected_context_is_not_forwarded_through_redirects() {
    let reached = Arc::new(AtomicBool::new(false));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let address = listener.local_addr().expect("address");
    let router = Router::new()
        .route(
            "/v1/chat/completions",
            post(|| async { Redirect::temporary("/sink") }),
        )
        .route("/sink", post(sink))
        .with_state(Arc::clone(&reached));
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.expect("server");
    });
    let provider = OpenAiProvider::from_values([
        ("OPENAI_API_KEY", "dummy-key".to_owned()),
        ("OPENAI_MODEL", "fixture".to_owned()),
        ("OPENAI_BASE_URL", format!("http://{address}/v1")),
    ]);
    let result = provider
        .generate("private selected context", proposal_schema(), &[])
        .await;
    server.abort();
    assert!(result.is_err());
    assert!(!reached.load(Ordering::SeqCst));
}
