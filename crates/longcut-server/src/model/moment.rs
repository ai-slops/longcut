use serde::{Deserialize, Serialize};

pub const MAX_MOMENTS: usize = 500;
pub const MAX_MOMENT_LABEL_BYTES: usize = 200;
pub const MAX_MOMENT_NOTE_BYTES: usize = 4_000;
pub const MAX_EVIDENCE_IDS: usize = 500;
pub const MAX_MOMENT_EVIDENCE_QUOTE_BYTES: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MomentNarrativeRole {
    Context,
    Setup,
    Goal,
    Attempt,
    Failure,
    Insight,
    Payoff,
    Other,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MomentOrigin {
    Manual,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MomentReviewStatus {
    Draft,
    Reviewed,
    Rejected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MomentEvidenceOrigin {
    SubtitleImport,
    LocalAsr,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MomentTimingBasis {
    Cues,
    Words,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MomentEvidenceSelection {
    pub transcript_id: String,
    #[serde(default)]
    pub cue_ids: Vec<String>,
    #[serde(default)]
    pub word_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MomentEvidence {
    pub transcript_id: String,
    pub cue_ids: Vec<String>,
    pub word_ids: Vec<String>,
    pub quote: String,
    pub origin: MomentEvidenceOrigin,
    pub start_frame: String,
    pub end_frame: String,
    pub timing_basis: MomentTimingBasis,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MomentProvenance {
    pub created_command_id: String,
    pub updated_command_id: String,
    pub created_revision: String,
    pub updated_revision: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Moment {
    pub id: String,
    pub version: String,
    pub source_id: String,
    pub start_frame: String,
    pub end_frame: String,
    pub label: String,
    pub note: String,
    pub narrative_role: MomentNarrativeRole,
    pub origin: MomentOrigin,
    pub review_status: MomentReviewStatus,
    pub archived: bool,
    pub evidence: Vec<MomentEvidence>,
    pub provenance: MomentProvenance,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MomentVersionRef {
    pub moment_id: String,
    pub version: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct MomentVersionRecord {
    pub project_id: String,
    pub moment_id: String,
    pub version: String,
    pub snapshot: Moment,
}
