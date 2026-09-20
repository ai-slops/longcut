use serde::{Deserialize, Serialize};

use super::{Camera, Scene};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feedback {
    pub id: String,
    pub body: String,
    pub scene_id: String,
    pub start_frame: String,
    pub end_frame: String,
    pub revision: String,
    pub render_id: Option<String>,
    pub pin: Option<Pin>,
    pub resolved: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pin {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChangeSet {
    pub id: String,
    pub title: String,
    pub intent: String,
    pub base_revision: String,
    pub operations: Vec<Operation>,
    #[serde(default)]
    pub preconditions: Vec<OperationPrecondition>,
    pub status: ChangeSetStatus,
    pub applied_operation_ids: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OperationPrecondition {
    InsertScene {
        operation_id: String,
        scene_id: String,
        source_id: String,
        in_frame: String,
        out_frame: String,
    },
    SetCamera {
        operation_id: String,
        scene_id: String,
        camera: Option<Camera>,
    },
    TrimScene {
        operation_id: String,
        scene_id: String,
        in_frame: String,
        out_frame: String,
    },
    UpdateScene {
        operation_id: String,
        scene_id: String,
        title: String,
        purpose: String,
        caption: String,
    },
    DeleteScene {
        operation_id: String,
        scene: Scene,
    },
}

impl OperationPrecondition {
    pub fn operation_id(&self) -> &str {
        match self {
            Self::InsertScene { operation_id, .. }
            | Self::SetCamera { operation_id, .. }
            | Self::TrimScene { operation_id, .. }
            | Self::UpdateScene { operation_id, .. }
            | Self::DeleteScene { operation_id, .. } => operation_id,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ChangeSetStatus {
    Ready,
    PartiallyApplied,
    Applied,
    Rejected,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operation {
    pub id: String,
    pub scene_id: String,
    #[serde(rename = "type")]
    pub kind: OperationType,
    pub camera: Option<Camera>,
    pub in_frame: Option<String>,
    pub out_frame: Option<String>,
    pub source_id: Option<String>,
    pub title: Option<String>,
    pub purpose: Option<String>,
    pub caption: Option<String>,
    pub rationale: String,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    InsertScene,
    SetCamera,
    TrimScene,
    UpdateScene,
    DeleteScene,
}
