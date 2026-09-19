use serde::{Deserialize, Serialize};

use super::{Camera, Operation, Pin, Project, RenderKind, SourceRole};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommandRequest {
    pub command_id: String,
    pub base_revision: String,
    pub command: Command,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Command {
    SetBrief {
        brief: String,
    },
    RenameProject {
        title: String,
    },
    SetSourceRole {
        source_id: String,
        role: SourceRole,
    },
    InsertScene {
        source_id: String,
        in_frame: String,
        out_frame: String,
        title: String,
    },
    TrimScene {
        scene_id: String,
        in_frame: String,
        out_frame: String,
    },
    SplitScene {
        scene_id: String,
        offset_frame: String,
    },
    MoveScene {
        scene_id: String,
        before_scene_id: Option<String>,
    },
    DeleteScene {
        scene_id: String,
    },
    UpdateScene {
        scene_id: String,
        title: String,
        purpose: String,
        caption: String,
    },
    SetCamera {
        scene_id: String,
        camera: Option<Camera>,
    },
    LockScene {
        scene_id: String,
        locked: bool,
    },
    CreateFeedback {
        scene_id: String,
        start_frame: String,
        end_frame: String,
        body: String,
        pin: Option<Pin>,
        render_id: Option<String>,
    },
    ResolveFeedback {
        feedback_id: String,
        resolved: bool,
    },
    CreateChangeSet {
        title: String,
        intent: String,
        operations: Vec<Operation>,
        evidence: Vec<String>,
    },
    ApplyChangeSet {
        change_set_id: String,
        operation_ids: Vec<String>,
    },
    RejectChangeSet {
        change_set_id: String,
    },
    Undo,
    Redo,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateProject {
    pub title: String,
    pub frame_rate: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateRender {
    pub base_revision: String,
    pub kind: RenderKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PersistedProject {
    pub project: Project,
}
