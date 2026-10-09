#![cfg(target_os = "linux")]

use std::os::unix::fs::PermissionsExt;

use axum::{
    body::{Body, to_bytes},
    http::Request,
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
    serde_json::from_slice(
        &to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("read response"),
    )
    .expect("parse response")
}

#[tokio::test]
async fn cancellation_kills_and_reaps_active_whisper_process() {
    // Given
    let data = TempDir::new().expect("create temp directory");
    let cli = data.path().join("blocking-whisper");
    let ffmpeg = data.path().join("fake-ffmpeg");
    let model = data.path().join("model.gguf");
    let marker = data.path().join("whisper.pid");
    std::fs::write(&model, b"fixture model").expect("write model");
    executable(
        &ffmpeg,
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then exit 0; fi\nfor last do :; done\nprintf wav > \"$last\"\n",
    );
    executable(
        &cli,
        &format!(
            "#!/bin/sh\nprintf '%s' \"$$\" > \"{}\"\nexec sleep 30\n",
            marker.display()
        ),
    );
    let state = AppState::initialize_with_asr_config(
        data.path(),
        AsrConfig::from_paths(cli, model).with_ffmpeg(ffmpeg),
    )
    .await
    .expect("initialize state");
    let project = state
        .store
        .create(CreateProject {
            title: "ASR cancellation".into(),
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
    let accepted = request(
        state.clone(),
        "POST",
        &format!("/api/v1/projects/{}/sources/source/asr", project.id),
        json!({}),
    )
    .await;
    let job_id = accepted["id"].as_str().expect("job id");
    let pid = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Ok(raw) = tokio::fs::read_to_string(&marker).await
                && let Ok(pid) = raw.parse::<u32>()
            {
                break pid;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("whisper process did not start");
    // When
    let cancelled = request(
        state.clone(),
        "POST",
        &format!(
            "/api/v1/projects/{}/sources/source/asr/{job_id}/cancel",
            project.id
        ),
        json!(null),
    )
    .await;
    // Then
    assert_eq!(cancelled["status"], "cancelled");
    let process = std::path::Path::new("/proc").join(pid.to_string());
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while process.exists() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("whisper child was not reaped");
    let transcript = request(
        state,
        "GET",
        &format!("/api/v1/projects/{}/sources/source/transcript", project.id),
        json!(null),
    )
    .await;
    assert!(transcript.is_null());
}
