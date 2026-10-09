#![allow(dead_code)]

use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
    response::Response,
};
use longcut_server::{
    AppState, app,
    model::{Command, CommandRequest, CreateProject, MediaStatus, Project, Source, SourceRole},
    transcript::{ImportTranscript, TranscriptPage},
};
use tempfile::TempDir;
use tower::ServiceExt;

pub struct Fixture {
    pub _root: TempDir,
    pub state: AppState,
    pub project: Project,
}

pub async fn fixture() -> Fixture {
    let root = TempDir::new().expect("create temp directory");
    let state = AppState::initialize(root.path())
        .await
        .expect("initialize state");
    let project = state
        .store
        .create(CreateProject {
            title: "Moments".into(),
            frame_rate: 30,
        })
        .await
        .expect("create project");
    let project = state
        .store
        .add_source(&project.id, visual_source("source"))
        .await
        .expect("add source");
    Fixture {
        _root: root,
        state,
        project,
    }
}

pub fn visual_source(id: &str) -> Source {
    Source {
        id: id.into(),
        name: format!("{id}.mp4"),
        role: SourceRole::Program,
        duration_frames: "300".into(),
        width: 1920,
        height: 1080,
        has_audio: true,
        media_url: format!("/api/v1/media/{id}.mp4"),
        proxy_url: None,
        thumbnail_url: None,
        waveform: vec![],
        status: MediaStatus::Ready,
        error: None,
    }
}

pub fn audio_source(id: &str) -> Source {
    Source {
        width: 0,
        height: 0,
        role: SourceRole::Audio,
        ..visual_source(id)
    }
}

pub async fn transcript(fixture: &Fixture) -> TranscriptPage {
    fixture
        .state
        .store
        .import_transcript(
            &fixture.project.id,
            "source",
            ImportTranscript {
                filename: "evidence.srt".into(),
                text: "1\n00:00:01,000 --> 00:00:02,000\nFirst exact quote\n\n2\n00:00:02,000 --> 00:00:03,000\nSecond exact quote".into(),
            },
        )
        .await
        .expect("import transcript")
}

pub async fn command(fixture: &Fixture, base_revision: &str, command: Command) -> Project {
    fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: base_revision.into(),
                command,
            },
        )
        .await
        .expect("apply command")
}

pub async fn json_request(
    router: Router,
    method: &str,
    uri: &str,
    value: serde_json::Value,
) -> Response {
    router
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
        .expect("route request")
}

pub async fn body_json(response: Response) -> serde_json::Value {
    serde_json::from_slice(
        &to_bytes(response.into_body(), 1 << 20)
            .await
            .expect("read body"),
    )
    .expect("parse response")
}

pub fn router(fixture: &Fixture) -> Router {
    app(fixture.state.clone())
}
