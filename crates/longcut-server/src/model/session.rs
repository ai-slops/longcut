use super::Source;
use longcut_session_contract::{
    Conformance,
    model::{MediaTime, Package, SessionRange, SourceKind},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionWarning {
    pub code: String,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionAsset {
    pub asset_id: String,
    pub uri: String,
    pub availability: AssetAvailability,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetAvailability {
    Online,
    Offline,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionSource {
    pub source_id: String,
    pub display_name: String,
    pub semantic_role: String,
    pub kind: SourceKind,
    pub editor_source_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceSessionSummary {
    pub id: String,
    pub session_id: String,
    pub title: String,
    pub duration_ns: String,
    pub conformance: Conformance,
    pub imported_at: String,
    pub assets: Vec<SessionAsset>,
    pub sources: Vec<SessionSource>,
    pub warnings: Vec<SessionWarning>,
    pub verification_url: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceSessionRecord {
    pub summary: SourceSessionSummary,
    pub package: Package,
    pub mappings: Vec<EditorSourceMapping>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditorSourceMapping {
    pub editor_source_id: String,
    pub package_source_id: String,
    pub bindings: Vec<StreamMapping>,
    pub session_range: SessionRange,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamMapping {
    pub asset_id: String,
    pub stream_index: u32,
    pub media_start: MediaTime,
    pub media_end: MediaTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    Validating,
    Prepared,
    Publishing,
    Confirmed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionImportJob {
    pub id: String,
    pub project_id: String,
    pub status: ImportStatus,
    pub created_at: String,
    pub error: Option<String>,
    pub session: Option<SourceSessionSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedImport {
    pub record: SourceSessionRecord,
    pub sources: Vec<Source>,
    pub artifacts: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmImport {
    pub base_revision: String,
    pub allow_degraded: bool,
}
