use longcut_session_contract::{Conformance, Report, model::AssetId};
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    Passed,
    Failed,
    NotPerformed,
    NotDeclared,
    Unverifiable,
}

pub(crate) fn aggregate_bounds(checks: impl Iterator<Item = Check>) -> Check {
    checks.fold(Check::NotDeclared, |left, right| match (left, right) {
        (Check::Failed, _) | (_, Check::Failed) => Check::Failed,
        (Check::Unverifiable, _) | (_, Check::Unverifiable) => Check::Unverifiable,
        (Check::NotPerformed, _) | (_, Check::NotPerformed) => Check::NotPerformed,
        (Check::Passed, _) | (_, Check::Passed) => Check::Passed,
        (Check::NotDeclared, Check::NotDeclared) => Check::NotDeclared,
    })
}

#[derive(Debug, Serialize)]
pub struct PacketBounds {
    pub stream_index: u32,
    pub start_pts: Option<String>,
    pub end_pts: Option<String>,
    pub time_base: Option<String>,
    pub packet_count: String,
    pub timing_complete: bool,
    pub coverage: Check,
    pub pts_ordered: bool,
    pub gap_count: String,
    pub overlap_count: String,
    pub max_gap_pts: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SegmentBounds {
    pub segment_index: usize,
    pub stream_index: u32,
    pub check: Check,
}

#[derive(Debug, Serialize)]
pub struct Issue {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct AssetVerification {
    pub asset_id: AssetId,
    pub uri: String,
    pub file: Check,
    pub size: Check,
    pub checksum: Check,
    pub probe: Check,
    pub stream_metadata: Check,
    pub full_decode: Check,
    pub packet_coverage: Check,
    pub time_map_bounds: Check,
    pub packet_bounds: Vec<PacketBounds>,
    pub time_map_segments: Vec<SegmentBounds>,
    pub actual_size_bytes: Option<String>,
    pub actual_sha256: Option<String>,
    pub ffprobe_version: Option<String>,
    pub unchecked_fields: Vec<String>,
    pub issues: Vec<Issue>,
}

impl AssetVerification {
    pub(crate) fn new(asset: &longcut_session_contract::model::Asset) -> Self {
        Self {
            asset_id: asset.asset_id.clone(),
            uri: asset.uri.clone(),
            file: Check::NotPerformed,
            size: Check::NotPerformed,
            checksum: Check::NotPerformed,
            probe: Check::NotPerformed,
            stream_metadata: Check::NotPerformed,
            full_decode: Check::NotPerformed,
            packet_coverage: Check::NotPerformed,
            time_map_bounds: Check::NotPerformed,
            packet_bounds: Vec::new(),
            time_map_segments: Vec::new(),
            actual_size_bytes: None,
            actual_sha256: None,
            ffprobe_version: None,
            unchecked_fields: Vec::new(),
            issues: Vec::new(),
        }
    }

    pub(crate) fn issue(&mut self, code: &'static str, message: impl Into<String>) {
        self.issues.push(Issue {
            code,
            message: message.into(),
        });
    }
}

#[derive(Debug, Serialize)]
pub struct Verification {
    pub metadata: Report,
    pub preflight_conformance: Conformance,
    pub verification_scope: &'static str,
    pub full_decode_verification: &'static str,
    pub time_map_media_bounds_verification: Check,
    pub time_map_bounds_method: &'static str,
    pub continuous_packet_coverage_verification: Check,
    pub packet_coverage_method: &'static str,
    pub alignment_verification: &'static str,
    pub assets: Vec<AssetVerification>,
    pub cancelled: bool,
}

impl Verification {
    pub(crate) fn new(metadata: Report) -> Self {
        Self {
            preflight_conformance: metadata.metadata_conformance,
            metadata,
            verification_scope: "file_integrity_stream_probe_and_packet_bounds",
            full_decode_verification: "not_performed",
            time_map_media_bounds_verification: Check::NotPerformed,
            time_map_bounds_method: "packet_pts_envelope",
            continuous_packet_coverage_verification: Check::NotPerformed,
            packet_coverage_method: "bounded_reordered_packet_pts_intervals_64",
            alignment_verification: "not_performed",
            assets: Vec::new(),
            cancelled: false,
        }
    }
}
