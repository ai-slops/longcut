use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use longcut_server::{AppState, app, domain, error::AppError, model::*};
use tempfile::TempDir;
use tower::ServiceExt;

async fn test_state() -> (TempDir, AppState) {
    let data = TempDir::new().expect("create temp directory");
    let state = AppState::initialize(data.path())
        .await
        .expect("initialize state");
    (data, state)
}

async fn json_request(
    router: axum::Router,
    method: &str,
    uri: &str,
    value: serde_json::Value,
) -> axum::response::Response {
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

async fn artifact_request(
    state: AppState,
    artifact: &str,
    range: Option<&str>,
) -> axum::response::Response {
    let mut request = Request::builder().uri(format!("/api/v1/media/{artifact}"));
    if let Some(value) = range {
        request = request.header(header::RANGE, value);
    }
    app(state)
        .oneshot(request.body(Body::empty()).expect("build request"))
        .await
        .expect("route request")
}

#[tokio::test]
async fn artifact_range_supports_suffix_and_clamps_explicit_end() {
    // Given
    let (_data, state) = test_state().await;
    tokio::fs::write(state.artifacts.join("clip.bin"), b"0123456789")
        .await
        .expect("write artifact");
    // When
    let suffix = artifact_request(state.clone(), "clip.bin", Some("bytes=-4")).await;
    let clamped = artifact_request(state, "clip.bin", Some("bytes=7-99")).await;
    // Then
    assert_eq!(suffix.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(suffix.headers()[header::CONTENT_RANGE], "bytes 6-9/10");
    assert_eq!(
        to_bytes(suffix.into_body(), 16).await.expect("read suffix"),
        "6789"
    );
    assert_eq!(clamped.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(clamped.headers()[header::CONTENT_RANGE], "bytes 7-9/10");
    assert_eq!(
        to_bytes(clamped.into_body(), 16)
            .await
            .expect("read clamped range"),
        "789"
    );
}

#[tokio::test]
async fn unsatisfiable_artifact_range_returns_416_with_size() {
    // Given
    let (_data, state) = test_state().await;
    tokio::fs::write(state.artifacts.join("clip.bin"), b"0123456789")
        .await
        .expect("write artifact");
    // When
    let response = artifact_request(state, "clip.bin", Some("bytes=10-")).await;
    // Then
    assert_eq!(response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes */10");
}

#[tokio::test]
async fn artifact_route_rejects_in_progress_files() {
    // Given
    let (_data, state) = test_state().await;
    tokio::fs::write(state.artifacts.join(".clip.bin.upload"), b"partial")
        .await
        .expect("write temporary artifact");
    // When
    let response = artifact_request(state, ".clip.bin.upload", None).await;
    // Then
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn command_is_idempotent_when_payload_matches() {
    // Given
    let (_data, state) = test_state().await;
    let project = state
        .store
        .create(CreateProject {
            title: "Test".into(),
            frame_rate: 30,
        })
        .await
        .expect("create project");
    let command_id = uuid::Uuid::new_v4().to_string();
    let request = CommandRequest {
        command_id,
        base_revision: "0".into(),
        command: Command::SetBrief {
            brief: "Keep it".into(),
        },
    };
    // When
    let first = state
        .store
        .command(&project.id, &request)
        .await
        .expect("first command");
    let duplicate = state
        .store
        .command(&project.id, &request)
        .await
        .expect("duplicate command");
    // Then
    assert_eq!(first.revision, "1");
    assert_eq!(duplicate.revision, "1");
    assert_eq!(duplicate.brief, "Keep it");
}

#[tokio::test]
async fn stale_command_returns_contract_error() {
    // Given
    let (_data, state) = test_state().await;
    let project = state
        .store
        .create(CreateProject {
            title: "Test".into(),
            frame_rate: 30,
        })
        .await
        .expect("create project");
    let request = CommandRequest {
        command_id: uuid::Uuid::new_v4().to_string(),
        base_revision: "9".into(),
        command: Command::SetBrief { brief: "No".into() },
    };
    // When
    let response = json_request(
        app(state),
        "POST",
        &format!("/api/v1/projects/{}/commands", project.id),
        serde_json::to_value(request).expect("serialize command"),
    )
    .await;
    // Then
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body: serde_json::Value = serde_json::from_slice(
        &to_bytes(response.into_body(), 4096)
            .await
            .expect("read body"),
    )
    .expect("parse body");
    assert_eq!(body["error"]["code"], "stale_revision");
}

#[tokio::test]
async fn restart_marks_interrupted_media_work_as_actionable_errors() {
    // Given
    let data = TempDir::new().expect("create temp directory");
    let state = AppState::initialize(data.path())
        .await
        .expect("initialize state");
    let project = state
        .store
        .create(CreateProject {
            title: "Recovery".into(),
            frame_rate: 30,
        })
        .await
        .expect("create project");
    let source = Source {
        id: "source".into(),
        name: "source.mp4".into(),
        role: SourceRole::Program,
        duration_frames: "90".into(),
        width: 1920,
        height: 1080,
        has_audio: true,
        media_url: "/api/v1/media/source.mp4".into(),
        proxy_url: None,
        thumbnail_url: None,
        waveform: vec![],
        status: MediaStatus::Processing,
        error: None,
    };
    let project = state
        .store
        .add_source(&project.id, source)
        .await
        .expect("add source");
    let render = Render {
        id: "render".into(),
        revision: project.revision.clone(),
        kind: RenderKind::Final,
        status: RenderStatus::Queued,
        progress: 0.0,
        media_url: None,
        srt_url: None,
        vtt_url: None,
        lineage_url: None,
        error: None,
    };
    state
        .store
        .enqueue_render(&project.id, render, &project.revision)
        .await
        .expect("enqueue render");
    drop(state);
    // When
    let reopened = AppState::initialize(data.path())
        .await
        .expect("reopen state")
        .store
        .get(&project.id)
        .await
        .expect("get project");
    // Then
    assert_eq!(reopened.sources[0].status, MediaStatus::Error);
    assert!(
        reopened.sources[0]
            .error
            .as_deref()
            .is_some_and(|message| message.contains("interrupted"))
    );
    assert_eq!(reopened.renders[0].status, RenderStatus::Error);
    assert!(
        reopened.renders[0]
            .error
            .as_deref()
            .is_some_and(|message| message.contains("interrupted"))
    );
}

#[test]
fn locked_scene_rejects_trim_without_mutation() {
    // Given
    let mut project = fixture_project();
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    project.scenes[0].locked = true;
    let before = project.scenes[0].clone();
    // When
    let result = domain::apply(
        &mut project,
        &mut history,
        Command::TrimScene {
            scene_id: before.id.clone(),
            in_frame: "1".into(),
            out_frame: "20".into(),
        },
    );
    // Then
    assert!(matches!(result, Err(AppError::Locked(_))));
    assert_eq!(project.scenes[0].in_frame, before.in_frame);
    assert_eq!(project.revision, "0");
}

#[test]
fn undo_and_redo_create_monotonic_revisions() {
    // Given
    let mut project = fixture_project();
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    domain::apply(
        &mut project,
        &mut history,
        Command::SetBrief {
            brief: "new".into(),
        },
    )
    .expect("edit");
    // When
    domain::apply(&mut project, &mut history, Command::Undo).expect("undo");
    let undo_revision = project.revision.clone();
    domain::apply(&mut project, &mut history, Command::Redo).expect("redo");
    // Then
    assert_eq!(undo_revision, "2");
    assert_eq!(project.revision, "3");
    assert_eq!(project.brief, "new");
}

#[test]
fn undo_and_redo_preserve_operational_media_state() {
    // Given
    let mut project = fixture_project();
    project.sources[0].status = MediaStatus::Processing;
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    domain::apply(
        &mut project,
        &mut history,
        Command::SetBrief {
            brief: "edited".into(),
        },
    )
    .expect("edit");
    project.sources[0].status = MediaStatus::Ready;
    project.sources[0].proxy_url = Some("/api/v1/media/source-proxy.mp4".into());
    project.renders.push(Render {
        id: "render".into(),
        revision: project.revision.clone(),
        kind: RenderKind::Final,
        status: RenderStatus::Ready,
        progress: 1.0,
        media_url: Some("/api/v1/media/render.mp4".into()),
        srt_url: None,
        vtt_url: None,
        lineage_url: None,
        error: None,
    });
    // When
    domain::apply(&mut project, &mut history, Command::Undo).expect("undo");
    // Then
    assert_eq!(project.sources[0].status, MediaStatus::Ready);
    assert_eq!(
        project.sources[0].proxy_url.as_deref(),
        Some("/api/v1/media/source-proxy.mp4")
    );
    assert_eq!(project.renders.len(), 1);
    assert_eq!(project.renders[0].status, RenderStatus::Ready);
    // When
    domain::apply(&mut project, &mut history, Command::Redo).expect("redo");
    // Then
    assert_eq!(project.brief, "edited");
    assert_eq!(project.sources[0].status, MediaStatus::Ready);
    assert_eq!(
        project.sources[0].proxy_url.as_deref(),
        Some("/api/v1/media/source-proxy.mp4")
    );
    assert_eq!(project.renders.len(), 1);
    assert_eq!(project.renders[0].status, RenderStatus::Ready);
}

#[test]
fn undo_of_source_import_removes_only_the_imported_source() {
    // Given
    let before_import = fixture_project();
    let mut project = before_import.clone();
    let mut imported = project.sources[0].clone();
    imported.id = "imported".into();
    imported.media_url = "/api/v1/media/imported.mp4".into();
    project.sources.push(imported);
    project.revision = "1".into();
    let mut history = History {
        undo: vec![before_import],
        redo: vec![],
    };
    // When
    domain::apply(&mut project, &mut history, Command::Undo).expect("undo import");
    // Then
    assert_eq!(project.sources.len(), 1);
    assert_eq!(project.sources[0].id, "source");
}

#[test]
fn split_preserves_feedback_on_each_resulting_scene() {
    // Given
    let mut project = fixture_project();
    project.feedback.push(Feedback {
        id: "feedback".into(),
        body: "crossing".into(),
        scene_id: "scene".into(),
        start_frame: "10".into(),
        end_frame: "50".into(),
        revision: "0".into(),
        render_id: None,
        pin: None,
        resolved: false,
    });
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    // When
    domain::apply(
        &mut project,
        &mut history,
        Command::SplitScene {
            scene_id: "scene".into(),
            offset_frame: "30".into(),
        },
    )
    .expect("split scene");
    // Then
    assert_eq!(project.feedback.len(), 2);
    assert_eq!(project.feedback[0].end_frame, "30");
    assert_eq!(project.feedback[1].scene_id, project.scenes[1].id);
    assert_eq!(project.feedback[1].end_frame, "20");
}

#[test]
fn split_offsets_camera_timing_without_rewriting_render_feedback() {
    // Given
    let mut project = fixture_project();
    project.scenes[0].camera = Some(Camera {
        source_id: "source".into(),
        in_frame: "5".into(),
        x: 0.5,
        y: 0.0,
        width: 0.5,
        height: 0.5,
        opacity: 1.0,
    });
    project.feedback.push(Feedback {
        id: "render-feedback".into(),
        body: "frozen evidence".into(),
        scene_id: "scene".into(),
        start_frame: "40".into(),
        end_frame: "50".into(),
        revision: "0".into(),
        render_id: Some("render".into()),
        pin: None,
        resolved: false,
    });
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    // When
    domain::apply(
        &mut project,
        &mut history,
        Command::SplitScene {
            scene_id: "scene".into(),
            offset_frame: "30".into(),
        },
    )
    .expect("split scene");
    // Then
    assert_eq!(
        project.scenes[1]
            .camera
            .as_ref()
            .expect("second camera")
            .in_frame,
        "35"
    );
    assert_eq!(project.feedback[0].scene_id, "scene");
    assert_eq!(project.feedback[0].start_frame, "40");
    assert_eq!(project.feedback[0].end_frame, "50");
}

#[test]
fn trim_rejects_scene_when_camera_media_is_too_short() {
    // Given
    let mut project = fixture_project();
    project.scenes[0].out_frame = "40".into();
    project.scenes[0].camera = Some(Camera {
        source_id: "source".into(),
        in_frame: "50".into(),
        x: 0.5,
        y: 0.0,
        width: 0.5,
        height: 0.5,
        opacity: 1.0,
    });
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    // When
    let result = domain::apply(
        &mut project,
        &mut history,
        Command::TrimScene {
            scene_id: "scene".into(),
            in_frame: "0".into(),
            out_frame: "60".into(),
        },
    );
    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
    assert_eq!(project.scenes[0].out_frame, "40");
}

#[test]
fn camera_rejects_non_finite_or_out_of_bounds_geometry() {
    // Given
    let mut project = fixture_project();
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    let invalid = Camera {
        source_id: "source".into(),
        in_frame: "0".into(),
        x: f32::NAN,
        y: 0.0,
        width: 0.5,
        height: 0.5,
        opacity: 1.0,
    };
    // When
    let non_finite = domain::apply(
        &mut project,
        &mut history,
        Command::SetCamera {
            scene_id: "scene".into(),
            camera: Some(invalid),
        },
    );
    let outside = domain::apply(
        &mut project,
        &mut history,
        Command::SetCamera {
            scene_id: "scene".into(),
            camera: Some(Camera {
                source_id: "source".into(),
                in_frame: "0".into(),
                x: 0.75,
                y: 0.0,
                width: 0.5,
                height: 0.5,
                opacity: 1.0,
            }),
        },
    );
    // Then
    assert!(matches!(non_finite, Err(AppError::Invalid(_))));
    assert!(matches!(outside, Err(AppError::Invalid(_))));
}

#[test]
fn visual_scene_rejects_audio_only_source() {
    // Given
    let mut project = fixture_project();
    let mut audio = project.sources[0].clone();
    audio.id = "audio".into();
    audio.role = SourceRole::Audio;
    audio.width = 0;
    audio.height = 0;
    project.sources.push(audio);
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    // When
    let result = domain::apply(
        &mut project,
        &mut history,
        Command::InsertScene {
            source_id: "audio".into(),
            in_frame: "0".into(),
            out_frame: "10".into(),
            title: "Audio".into(),
        },
    );
    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
    assert_eq!(project.scenes.len(), 1);
}

#[test]
fn split_rejects_frame_overflow_without_panicking() {
    // Given
    let mut project = fixture_project();
    project.scenes[0].in_frame = (u64::MAX - 5).to_string();
    project.scenes[0].out_frame = u64::MAX.to_string();
    project.sources[0].duration_frames = u64::MAX.to_string();
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    // When
    let result = domain::apply(
        &mut project,
        &mut history,
        Command::SplitScene {
            scene_id: "scene".into(),
            offset_frame: "10".into(),
        },
    );
    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
}

#[test]
fn change_set_can_apply_a_subset_atomically() {
    // Given
    let mut project = fixture_project();
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    let scene_id = project.scenes[0].id.clone();
    let operations = vec![
        Operation {
            id: "a".into(),
            scene_id: scene_id.clone(),
            kind: OperationType::TrimScene,
            camera: None,
            in_frame: Some("10".into()),
            out_frame: Some("50".into()),
            source_id: None,
            title: None,
            purpose: None,
            caption: None,
            rationale: "tighten".into(),
        },
        Operation {
            id: "b".into(),
            scene_id: scene_id.clone(),
            kind: OperationType::SetCamera,
            camera: None,
            in_frame: None,
            out_frame: None,
            source_id: None,
            title: None,
            purpose: None,
            caption: None,
            rationale: "keep program only".into(),
        },
    ];
    domain::apply(
        &mut project,
        &mut history,
        Command::CreateChangeSet {
            title: "Proposal".into(),
            intent: "pace".into(),
            operations,
            evidence: vec![],
        },
    )
    .expect("create proposal");
    let id = project.change_sets[0].id.clone();
    // When
    domain::apply(
        &mut project,
        &mut history,
        Command::ApplyChangeSet {
            change_set_id: id.clone(),
            operation_ids: vec!["a".into()],
        },
    )
    .expect("apply subset");
    // Then
    assert_eq!(project.scenes[0].in_frame, "10");
    assert_eq!(
        project.change_sets[0].status,
        ChangeSetStatus::PartiallyApplied
    );
    assert!(
        !project.scenes[0].manual,
        "agent proposal must not mark a direct manual edit"
    );
    // When
    let revision = project.revision.clone();
    domain::apply(
        &mut project,
        &mut history,
        Command::ApplyChangeSet {
            change_set_id: id,
            operation_ids: vec!["b".into()],
        },
    )
    .expect("apply independent operation");
    // Then
    assert_ne!(project.revision, revision);
    assert_eq!(project.change_sets[0].status, ChangeSetStatus::Applied);
}

#[test]
fn change_set_allows_unrelated_scene_edits_after_creation() {
    // Given
    let mut project = fixture_project();
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    let scene_id = project.scenes[0].id.clone();
    domain::apply(
        &mut project,
        &mut history,
        Command::CreateChangeSet {
            title: "Proposal".into(),
            intent: "pace".into(),
            operations: vec![Operation {
                id: "trim".into(),
                scene_id: scene_id.clone(),
                kind: OperationType::TrimScene,
                camera: None,
                in_frame: Some("10".into()),
                out_frame: Some("50".into()),
                source_id: None,
                title: None,
                purpose: None,
                caption: None,
                rationale: "tighten".into(),
            }],
            evidence: vec![],
        },
    )
    .expect("create proposal");
    let change_set_id = project.change_sets[0].id.clone();
    domain::apply(
        &mut project,
        &mut history,
        Command::UpdateScene {
            scene_id,
            title: "Renamed".into(),
            purpose: "Updated".into(),
            caption: "Caption".into(),
        },
    )
    .expect("unrelated edit");
    // When
    domain::apply(
        &mut project,
        &mut history,
        Command::ApplyChangeSet {
            change_set_id,
            operation_ids: vec!["trim".into()],
        },
    )
    .expect("apply proposal");
    // Then
    assert_eq!(project.scenes[0].title, "Renamed");
    assert_eq!(project.scenes[0].in_frame, "10");
    assert_eq!(project.scenes[0].out_frame, "50");
}

#[test]
fn change_set_rejects_an_operation_when_its_target_property_changed() {
    // Given
    let mut project = fixture_project();
    let mut history = History {
        undo: vec![],
        redo: vec![],
    };
    let scene_id = project.scenes[0].id.clone();
    domain::apply(
        &mut project,
        &mut history,
        Command::CreateChangeSet {
            title: "Proposal".into(),
            intent: "pace".into(),
            operations: vec![Operation {
                id: "trim".into(),
                scene_id: scene_id.clone(),
                kind: OperationType::TrimScene,
                camera: None,
                in_frame: Some("10".into()),
                out_frame: Some("50".into()),
                source_id: None,
                title: None,
                purpose: None,
                caption: None,
                rationale: "tighten".into(),
            }],
            evidence: vec![],
        },
    )
    .expect("create proposal");
    let change_set_id = project.change_sets[0].id.clone();
    domain::apply(
        &mut project,
        &mut history,
        Command::TrimScene {
            scene_id,
            in_frame: "5".into(),
            out_frame: "80".into(),
        },
    )
    .expect("direct trim");
    // When
    let result = domain::apply(
        &mut project,
        &mut history,
        Command::ApplyChangeSet {
            change_set_id,
            operation_ids: vec!["trim".into()],
        },
    );
    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert_eq!(project.scenes[0].in_frame, "5");
    assert_eq!(project.scenes[0].out_frame, "80");
}

fn fixture_project() -> Project {
    Project {
        id: "p".into(),
        title: "Test".into(),
        revision: "0".into(),
        frame_rate: 30,
        updated_at: String::new(),
        brief: String::new(),
        sources: vec![Source {
            id: "source".into(),
            name: "source.mp4".into(),
            role: SourceRole::Program,
            duration_frames: "100".into(),
            width: 1920,
            height: 1080,
            has_audio: true,
            media_url: "/api/v1/media/source.mp4".into(),
            proxy_url: None,
            thumbnail_url: None,
            waveform: vec![],
            status: MediaStatus::Ready,
            error: None,
        }],
        scenes: vec![Scene {
            id: "scene".into(),
            title: "Scene".into(),
            purpose: String::new(),
            source_id: "source".into(),
            in_frame: "0".into(),
            out_frame: "90".into(),
            camera: None,
            caption: String::new(),
            locked: false,
            manual: false,
        }],
        feedback: vec![],
        change_sets: vec![],
        renders: vec![],
        can_undo: false,
        can_redo: false,
    }
}
