#![cfg(target_os = "linux")]

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
use serde_json::Value;
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

async fn request(state: AppState, method: &str, uri: &str) -> (StatusCode, Value) {
    let response = app(state)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .expect("build request"),
        )
        .await
        .expect("route request");
    let status = response.status();
    let value = serde_json::from_slice(
        &to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("read response"),
    )
    .expect("parse response");
    (status, value)
}

async fn terminal_job(state: &AppState, uri: &str) -> Value {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (_, job) = request(state.clone(), "GET", uri).await;
            if matches!(
                job["status"].as_str(),
                Some("ready" | "failed" | "cancelled")
            ) {
                break job;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("job timeout")
}

#[tokio::test]
async fn cancelled_job_resumes_after_checkpoint_without_repeating_completed_chunk() {
    // Given
    let data = TempDir::new().expect("create temp directory");
    let cli = data.path().join("fake-whisper");
    let ffmpeg = data.path().join("fake-ffmpeg");
    let model = data.path().join("model.gguf");
    let marker = data.path().join("blocked-once");
    let calls = data.path().join("calls");
    std::fs::write(&model, b"fixture model").expect("write model");
    executable(
        &ffmpeg,
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then exit 0; fi\nfor last do :; done\nprintf wav > \"$last\"\n",
    );
    executable(
        &cli,
        &format!(
            "#!/bin/sh\nprefix=\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"-of\" ]; then shift; prefix=$1; fi\n  shift\ndone\nname=${{prefix##*chunk-}}\nprintf '%s\\n' \"$name\" >> \"{}\"\nif [ \"$name\" = 1 ] && [ ! -f \"{}\" ]; then touch \"{}\"; exec sleep 30; fi\nif [ \"$name\" = 0 ]; then from=0; to=1000; else from=2000; to=3000; fi\nprintf '{{\"transcription\":[{{\"offsets\":{{\"from\":%s,\"to\":%s}},\"text\":\" word%s\"}}],\"result\":{{\"language\":\"en\"}}}}' \"$from\" \"$to\" \"$name\" > \"${{prefix}}.json\"\n",
            calls.display(),
            marker.display(),
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
            title: "ASR resume".into(),
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
                duration_frames: "36001".into(),
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
    let start_uri = format!("/api/v1/projects/{}/sources/source/asr", project.id);
    let (_, accepted) = request(state.clone(), "POST", &start_uri).await;
    let job_id = accepted["id"].as_str().expect("job id");
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while !marker.exists() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("second chunk did not start");
    let job_uri = format!("{start_uri}/{job_id}");
    let (_, cancelled) = request(state.clone(), "POST", &format!("{job_uri}/cancel")).await;
    assert_eq!(cancelled["status"], "cancelled");
    assert_eq!(cancelled["resume_eligible"], true);

    // When
    let resumed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let (status, job) = request(state.clone(), "POST", &format!("{job_uri}/resume")).await;
            if status == StatusCode::ACCEPTED {
                break job;
            }
            assert_eq!(status, StatusCode::CONFLICT);
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("worker did not release resume fence");
    assert_eq!(resumed["status"], "queued");
    let ready = terminal_job(&state, &job_uri).await;

    // Then
    assert_eq!(ready["status"], "ready");
    assert_eq!(ready["resume_eligible"], false);
    let calls = tokio::fs::read_to_string(calls).await.expect("read calls");
    assert_eq!(calls.lines().filter(|line| *line == "0").count(), 1);
    assert_eq!(calls.lines().filter(|line| *line == "1").count(), 2);
    assert_eq!(calls.lines().filter(|line| *line == "2").count(), 1);
}
