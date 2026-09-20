use longcut_server::{
    ai::{AiOperation, AiProposal, AiRequest, AiScope, build_prompt, validate_proposal},
    model::{Camera, MediaStatus, Project, Scene, Source, SourceRole},
};

#[test]
fn operation_with_empty_evidence_is_rejected() {
    // Given
    let project = scene_project();
    let proposal = AiProposal {
        title: "Invalid".into(),
        summary: "Invalid".into(),
        warnings: vec![],
        operations: vec![AiOperation::DeleteScene {
            scene_id: "scene".into(),
            rationale: "No evidence".into(),
            evidence_ids: vec![],
        }],
    };
    // When
    let result = validate_proposal(
        &project,
        &AiScope::Scenes {
            scene_ids: vec!["scene".into()],
        },
        &[],
        &proposal,
    );
    // Then
    assert!(result.is_err());
}

#[test]
fn scene_scope_camera_may_use_only_context_sources() {
    // Given
    let project = scene_project();
    let scope = AiScope::Scenes {
        scene_ids: vec!["scene".into()],
    };
    let permitted = camera_proposal("available-camera");
    let private = camera_proposal("unrelated");
    // When
    let permitted_result = validate_proposal(&project, &scope, &[], &permitted);
    let private_result = validate_proposal(&project, &scope, &[], &private);
    // Then
    assert!(permitted_result.is_ok());
    assert!(private_result.is_err());
}

#[test]
fn scene_prompt_contains_only_selected_and_permitted_camera_metadata() {
    // Given
    let project = scene_project();
    let request = AiRequest {
        base_revision: "0".into(),
        instruction: "Use an appropriate angle".into(),
        consent: true,
        scope: AiScope::Scenes {
            scene_ids: vec!["scene".into()],
        },
    };
    // When
    let prompt = build_prompt(&project, &request, &[]).expect("build prompt");
    let metadata = prompt
        .split_once("<untrusted_metadata>\n")
        .and_then(|(_, suffix)| suffix.split_once("\n</untrusted_metadata>"))
        .map(|(json, _)| json)
        .expect("structured metadata block");
    let context: serde_json::Value = serde_json::from_str(metadata).expect("parse context");
    let sources = context["sources"].as_array().expect("source metadata");
    let ids = sources
        .iter()
        .map(|source| source["id"].as_str().expect("source id"))
        .collect::<Vec<_>>();
    // Then
    assert_eq!(ids, ["selected", "existing-camera", "available-camera"]);
    assert!(sources.iter().all(|source| source["role"].is_string()));
    assert!(!ids.contains(&"unrelated"));
}

fn camera_proposal(source_id: &str) -> AiProposal {
    AiProposal {
        title: "Camera".into(),
        summary: "Camera".into(),
        warnings: vec![],
        operations: vec![AiOperation::SetCamera {
            scene_id: "scene".into(),
            camera: Some(Camera {
                source_id: source_id.into(),
                in_frame: "0".into(),
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
                opacity: 1.0,
            }),
            rationale: "Alternate angle".into(),
            evidence_ids: vec!["scene_scene".into()],
        }],
    }
}

fn scene_project() -> Project {
    let source = |id: &str, role: SourceRole| Source {
        id: id.into(),
        name: format!("{id}.mp4"),
        role,
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
    };
    Project {
        id: "project".into(),
        title: "Project".into(),
        revision: "0".into(),
        frame_rate: 30,
        updated_at: String::new(),
        brief: String::new(),
        sources: vec![
            source("selected", SourceRole::Primary),
            source("existing-camera", SourceRole::Primary),
            source("available-camera", SourceRole::Camera),
            source("unrelated", SourceRole::Program),
        ],
        scenes: vec![Scene {
            id: "scene".into(),
            title: "Scene".into(),
            purpose: String::new(),
            source_id: "selected".into(),
            in_frame: "0".into(),
            out_frame: "120".into(),
            camera: Some(Camera {
                source_id: "existing-camera".into(),
                in_frame: "0".into(),
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
                opacity: 1.0,
            }),
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
