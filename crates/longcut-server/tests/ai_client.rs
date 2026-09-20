use std::{
    io::{Read, Write},
    net::TcpListener,
};

use longcut_server::ai::{CodexClient, HttpCodexClient};

#[tokio::test]
async fn sidecar_public_error_envelope_is_bounded_and_exposed() {
    // Given
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request = [0_u8; 2048];
        let request_bytes = stream.read(&mut request).expect("read request");
        assert!(request_bytes > 0, "request must not be empty");
        let body = r#"{"error":{"code":"AUTH_REQUIRED","message":"Sign in to Codex."}}"#;
        let response = format!(
            "HTTP/1.1 401 Unauthorized\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).expect("respond");
    });
    let directory = tempfile::tempdir().expect("tempdir");
    let token_path = directory.path().join("token");
    tokio::fs::write(&token_path, "test-token")
        .await
        .expect("token");
    let client = HttpCodexClient::new(format!("http://{address}"), token_path).expect("client");
    // When
    let error = client.status().await.expect_err("typed sidecar error");
    // Then
    assert_eq!(
        error.to_string(),
        "conflict: AUTH_REQUIRED: Sign in to Codex."
    );
    server.join().expect("server thread");
}
