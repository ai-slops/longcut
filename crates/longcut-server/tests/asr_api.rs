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

#[cfg(unix)]
fn executable(path: &std::path::Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, contents).expect("write fake executable");
    let mut permissions = std::fs::metadata(path)
        .expect("fake metadata")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).expect("make fake executable");
}

async fn request(
    state: AppState,
    method: &str,
    uri: &str,
    body: Value,
) -> axum::response::Response {
    app(state)
        .oneshot(
            Request::builder()
                .method(method)
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&body).expect("serialize request"),
                ))
                .expect("build request"),
        )
        .await
        .expect("route request")
}

async fn body(response: axum::response::Response) -> Value {
    serde_json::from_slice(
        &to_bytes(response.into_body(), 64 * 1024)
            .await
            .expect("read response"),
    )
    .expect("parse response")
}

#[tokio::test]
async fn status_reports_missing_runtime_configuration() {
    // Given
    let data = TempDir::new().expect("create temp directory");
    let state = AppState::initialize(data.path())
        .await
        .expect("initialize state");
    // When
    let response = request(state, "GET", "/api/v1/asr/status", json!(null)).await;
    // Then
    assert_eq!(response.status(), StatusCode::OK);
    let status = body(response).await;
    assert_eq!(status["configured"], false);
    assert_eq!(status["available"], false);
    assert_eq!(
        status["missing"],
        json!(["LONGCUT_WHISPER_CLI", "LONGCUT_WHISPER_MODEL"])
    );
}

#[tokio::test]
async fn start_rejects_language_outside_closed_contract() {
    // Given
    let data = TempDir::new().expect("create temp directory");
    let state = AppState::initialize(data.path())
        .await
        .expect("initialize state");
    let project = state
        .store
        .create(CreateProject {
            title: "ASR".into(),
            frame_rate: 30,
        })
        .await
        .expect("create project");
    // When
    let response = request(
        state,
        "POST",
        &format!("/api/v1/projects/{}/sources/missing/asr", project.id),
        json!({"language": "ja"}),
    )
    .await;
    // Then
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body(response).await["error"]["code"], "invalid_input");
}

#[cfg(unix)]
#[tokio::test]
async fn local_job_publishes_searchable_transcript_without_edit_revision_change() {
    // Given
    let data = TempDir::new().expect("create temp directory");
    let cli = data.path().join("fake-whisper");
    let ffmpeg = data.path().join("fake-ffmpeg");
    let model = data.path().join("model.gguf");
    std::fs::write(&model, b"fixture, not a speech-accuracy model").expect("write model");
    executable(
        &ffmpeg,
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then exit 0; fi\nfor last do :; done\nprintf wav > \"$last\"\n",
    );
    executable(
        &cli,
        "#!/bin/sh\nprefix=\nmax_len=\nsplit_on_word=\nwhile [ $# -gt 0 ]; do\n  if [ \"$1\" = \"-of\" ]; then shift; prefix=$1; fi\n  if [ \"$1\" = \"-ml\" ]; then shift; max_len=$1; fi\n  if [ \"$1\" = \"-sow\" ]; then split_on_word=1; fi\n  shift\ndone\n[ \"$max_len\" = 1 ] && [ \"$split_on_word\" = 1 ] || exit 9\nprintf '%s' '{\"transcription\":[{\"offsets\":{\"from\":0,\"to\":400},\"text\":\" 로컬\"},{\"offsets\":{\"from\":400,\"to\":1000},\"text\":\" 음성\"}],\"result\":{\"language\":\"ko\"}}' > \"${prefix}.json\"\n",
    );
    let config = AsrConfig::from_paths(cli, model).with_ffmpeg(ffmpeg);
    let state = AppState::initialize_with_asr_config(data.path(), config)
        .await
        .expect("initialize state");
    let project = state
        .store
        .create(CreateProject {
            title: "ASR E2E".into(),
            frame_rate: 30,
        })
        .await
        .expect("create project");
    let project = state
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
        .expect("write managed source");
    // When
    let response = request(
        state.clone(),
        "POST",
        &format!("/api/v1/projects/{}/sources/source/asr", project.id),
        json!({}),
    )
    .await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let accepted = body(response).await;
    assert_eq!(accepted["resume_eligible"], false);
    let job_id = accepted["id"].as_str().expect("job id");
    let ready = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let response = request(
                state.clone(),
                "GET",
                &format!(
                    "/api/v1/projects/{}/sources/source/asr/{job_id}",
                    project.id
                ),
                json!(null),
            )
            .await;
            let job = body(response).await;
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
    .expect("job timeout");
    // Then
    assert_eq!(ready["status"], "ready");
    let transcript = request(
        state.clone(),
        "GET",
        &format!(
            "/api/v1/projects/{}/sources/source/transcript?query=로컬",
            project.id
        ),
        json!(null),
    )
    .await;
    let transcript = body(transcript).await;
    assert_eq!(transcript["origin"], "local_asr");
    assert_eq!(transcript["asr"]["engine"], "whisper.cpp");
    assert_eq!(transcript["asr"]["result_language"], "ko");
    assert_eq!(transcript["asr"]["overlap_seconds"], 2);
    assert_eq!(transcript["asr"]["ownership_policy"], "half_open_midpoint");
    assert_eq!(transcript["asr"]["timing_quality"], "model_estimated");
    assert_eq!(transcript["cues"][0]["start_frame"], "0");
    assert_eq!(transcript["cues"][0]["end_frame"], "30");
    assert_eq!(transcript["cues"][0]["text"], "로컬 음성");
    assert_eq!(
        transcript["cues"][0]["words"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        state
            .store
            .get(&project.id)
            .await
            .expect("project")
            .revision,
        project.revision
    );
}
