use serde::{Deserialize, Serialize};

use crate::model::Camera;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AiProvider {
    #[default]
    Codex,
    OpenaiCompatible,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiRequest {
    #[serde(default)]
    pub provider: AiProvider,
    pub base_revision: String,
    pub instruction: String,
    pub consent: bool,
    pub scope: AiScope,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AiScope {
    Source {
        source_id: String,
        start_frame: String,
        end_frame: String,
    },
    Scenes {
        scene_ids: Vec<String>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AiRunStatus {
    Queued,
    Sampling,
    Generating,
    Ready,
    Error,
    Cancelled,
    Stale,
}

impl AiRunStatus {
    pub const fn active(self) -> bool {
        matches!(self, Self::Queued | Self::Sampling | Self::Generating)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AiRun {
    pub id: String,
    pub project_id: String,
    pub base_revision: String,
    pub instruction: String,
    #[serde(default)]
    pub provider: AiProvider,
    pub scope: AiScope,
    pub status: AiRunStatus,
    pub created_at: String,
    pub updated_at: String,
    pub change_set_id: Option<String>,
    pub summary: Option<String>,
    pub warnings: Vec<String>,
    pub sample_count: u8,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProvidersResponse {
    pub providers: Vec<ProviderStatus>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProviderStatus {
    pub id: AiProvider,
    pub name: String,
    pub configured: bool,
    pub model: Option<String>,
    pub endpoint: Option<String>,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuntimeInfo {
    pub engine: String,
    pub version: String,
    pub node_version: String,
    pub protocol_version: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuntimeMetadata {
    pub engine: String,
    pub engine_version: String,
    pub execution_id: String,
    pub profile_revision: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePhase {
    Admitted,
    Provider,
    Validating,
    CleaningUp,
    Terminal,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuntimeProgress {
    pub sequence: u64,
    pub phase: RuntimePhase,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuntimeError {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeJobStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuntimeJob {
    pub id: String,
    pub status: RuntimeJobStatus,
    pub result: Option<AiProposal>,
    pub error: Option<RuntimeError>,
    pub runtime: RuntimeMetadata,
    pub progress: RuntimeProgress,
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeAvailability {
    pub available: bool,
    pub engine: String,
    pub version: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RunRuntimeAvailability {
    pub run_id: String,
    pub available: bool,
    pub runtime: Option<RuntimeMetadata>,
    pub progress: Option<RuntimeProgress>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AiProposal {
    pub title: String,
    pub summary: String,
    pub warnings: Vec<String>,
    pub operations: Vec<AiOperation>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum AiOperation {
    InsertScene {
        source_id: String,
        in_frame: String,
        out_frame: String,
        title: String,
        rationale: String,
        evidence_ids: Vec<String>,
    },
    TrimScene {
        scene_id: String,
        in_frame: String,
        out_frame: String,
        rationale: String,
        evidence_ids: Vec<String>,
    },
    SetCamera {
        scene_id: String,
        camera: Option<Camera>,
        rationale: String,
        evidence_ids: Vec<String>,
    },
    UpdateScene {
        scene_id: String,
        title: String,
        purpose: String,
        caption: String,
        rationale: String,
        evidence_ids: Vec<String>,
    },
    DeleteScene {
        scene_id: String,
        rationale: String,
        evidence_ids: Vec<String>,
    },
}

impl AiOperation {
    pub fn evidence_ids(&self) -> &[String] {
        match self {
            Self::InsertScene { evidence_ids, .. }
            | Self::TrimScene { evidence_ids, .. }
            | Self::SetCamera { evidence_ids, .. }
            | Self::UpdateScene { evidence_ids, .. }
            | Self::DeleteScene { evidence_ids, .. } => evidence_ids,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EvidenceSample {
    pub evidence_id: String,
    pub source_id: String,
    pub frame: u64,
    pub jpeg: Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CodexStatus {
    pub installed: bool,
    pub version: Option<String>,
    pub state: CodexAccountState,
    pub auth_mode: CodexAuthMode,
    pub login: Option<CodexLogin>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexAccountState {
    LoggedOut,
    LoginPending,
    Ready,
    Busy,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CodexAuthMode {
    None,
    Chatgpt,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CodexLogin {
    pub url: String,
    pub code: String,
    pub expires_at: String,
}
