#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use longcut_server::{
    AppState, app,
    asr::AsrConfig,
    model::{CreateProject, MediaStatus, Source, SourceRole},
};
use serde_json::{Value, json};
use tempfile::TempDir;
use tower::ServiceExt;

fn executable(path: &std::path::Path, contents: &str) {
    std::fs::write(path, contents).expect("write fake executable");
    let mut permissions = std::fs::metadata(path)
        .expect("fake metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("make fake executable");
}

async fn request(state: AppState, method: &str, uri: &str, value: Value) -> Value {
    let response = app(state)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&value).expect("serialize request"),
                ))
                .expect("build request"),
        )
        .await
        .expect("route request");
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::ACCEPTED);
    serde_json::from_slice(
        &to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("read response"),
    )
    .expect("parse response")
}

#[tokio::test]
async fn oversized_cue_fails_job_without_publishing_partial_transcript() {
    // Given
    let data = TempDir::new().expect("create temp directory");
    let cli = data.path().join("fake-whisper");
    let ffmpeg = data.path().join("fake-ffmpeg");
    let model = data.path().join("model.gguf");
    let payload = data.path().join("oversized.json");
    std::fs::write(&model, b"fixture model").expect("write model");
    std::fs::write(
        &payload,
        serde_json::to_vec(&json!({
            "transcription": [{
                "offsets": {"from": 0, "to": 1000},
                "text": "x".repeat((16 * 1024) + 1)
            }],
            "result": {"language": "ko"}
        }))
        .expect("serialize whisper output"),
    )
    .expect("write whisper output");
    executable(
        &ffmpeg,
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then exit 0; fi\nfor last do :; done\nprintf wav > \"$last\"\n",
    );
    executable(
        &cli,
        &format!(
            "#!/bin/sh\nprefix=\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"-of\" ]; then shift; prefix=$1; fi\n  shift\ndone\ncp \"{}\" \"${{prefix}}.json\"\n",
            payload.display()
        ),
    );
    let config = AsrConfig::from_paths(cli, model).with_ffmpeg(ffmpeg);
    let state = AppState::initialize_with_asr_config(data.path(), config)
        .await
        .expect("initialize state");
    let project = state
        .store
        .create(CreateProject {
            title: "ASR limit".into(),
            frame_rate: 30,
        })
        .await
        .expect("create project");
    state
        .store
        .add_source(
            &project.id,
            Source {
                id: "source".into(),
                name: "source".into(),
                role: SourceRole::Program,
                duration_frames: "90".into(),
                width: 1920,
                height: 1080,
                has_audio: true,
                media_url: "/api/v1/media/source.mp4".into(),
                proxy_url: None,
                thumbnail_url: None,
                waveform: Vec::new(),
                status: MediaStatus::Ready,
                error: None,
            },
        )
        .await
        .expect("add source");
    tokio::fs::write(state.artifacts.join("source.mp4"), b"managed fixture")
        .await
        .expect("write source");
    // When
    let accepted = request(
        state.clone(),
        "POST",
        &format!("/api/v1/projects/{}/sources/source/asr", project.id),
        json!({}),
    )
    .await;
    let job_id = accepted["id"].as_str().expect("job id");
    let failed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let job = request(
                state.clone(),
                "GET",
                &format!(
                    "/api/v1/projects/{}/sources/source/asr/{job_id}",
                    project.id
                ),
                json!(null),
            )
            .await;
            if job["status"] == "failed" {
                break job;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("job timeout");
    // Then
    assert!(
        failed["error"]
            .as_str()
            .is_some_and(|error| error.contains("16384"))
    );
    let transcript = request(
        state,
        "GET",
        &format!("/api/v1/projects/{}/sources/source/transcript", project.id),
        json!(null),
    )
    .await;
    assert!(transcript.is_null());
}
