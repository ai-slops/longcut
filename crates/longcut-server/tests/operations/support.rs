use longcut_server::{domain, error::AppError, model::*};

pub(super) struct Approval<'a> {
    pub change_set_id: &'a str,
    pub operation_ids: &'a [&'a str],
}

pub(super) fn operation(kind: OperationType, scene_id: &str, id: &str) -> Operation {
    Operation {
        id: id.into(),
        scene_id: scene_id.into(),
        kind,
        camera: None,
        in_frame: None,
        out_frame: None,
        source_id: None,
        title: None,
        purpose: None,
        caption: None,
        rationale: "because".into(),
    }
}

pub(super) fn create_change_set(
    project: &mut Project,
    history: &mut History,
    operations: Vec<Operation>,
) {
    try_create_change_set(project, history, operations).expect("create change set");
}

pub(super) fn try_create_change_set(
    project: &mut Project,
    history: &mut History,
    operations: Vec<Operation>,
) -> Result<(), AppError> {
    domain::apply(
        project,
        history,
        Command::CreateChangeSet {
            title: "Proposal".into(),
            intent: "Improve cut".into(),
            operations,
            evidence: vec![],
        },
    )
}

pub(super) fn apply_operations(
    project: &mut Project,
    history: &mut History,
    approval: Approval<'_>,
) -> Result<(), AppError> {
    domain::apply(
        project,
        history,
        Command::ApplyChangeSet {
            change_set_id: approval.change_set_id.into(),
            operation_ids: approval
                .operation_ids
                .iter()
                .map(|id| (*id).into())
                .collect(),
        },
    )
}

pub(super) fn valid_camera() -> Camera {
    Camera {
        source_id: "source".into(),
        in_frame: "0".into(),
        x: 0.0,
        y: 0.0,
        width: 0.5,
        height: 0.5,
        opacity: 1.0,
    }
}

pub(super) fn history() -> History {
    History {
        undo: vec![],
        redo: vec![],
    }
}

pub(super) fn fixture_project() -> Project {
    Project {
        id: "project".into(),
        title: "Test".into(),
        revision: "0".into(),
        frame_rate: 30,
        updated_at: String::new(),
        brief: String::new(),
        source_sessions: vec![],
        moments: vec![],
        story: Story::default(),
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
            moment: None,
            beat_id: None,
        }],
        feedback: vec![],
        change_sets: vec![],
        renders: vec![],
        can_undo: false,
        can_redo: false,
    }
}
