use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{AssetId, Extensions, MediaRef, MediaTime, Range, SessionRange, SourceId};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeMaps {
    pub segments: Vec<TimeMapSegment>,
    #[serde(default)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeMapSegment {
    pub media_ref: MediaRef,
    pub media_range: Range<MediaTime>,
    pub session_range: SessionRange,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisualComposition {
    pub intervals: Vec<VisualInterval>,
    #[serde(default)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisualInterval {
    pub session_range: SessionRange,
    pub canvas: Canvas,
    pub contributions: Vec<VisualContribution>,
    pub coverage: Option<Coverage>,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VisualContribution {
    pub source_id: SourceId,
    pub semantic_role: VisualRole,
    pub normalized_rect: Option<Rect>,
    pub crop: Option<Crop>,
    pub fit: Option<Fit>,
    pub z_order: Option<i32>,
    pub opacity: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Crop {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VisualRole {
    Primary,
    Secondary,
    Reaction,
    Background,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Fit {
    Contain,
    Cover,
    Stretch,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Coverage {
    Complete,
    Partial,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AudioMix {
    pub intervals: Vec<AudioInterval>,
    #[serde(default)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AudioInterval {
    pub session_range: SessionRange,
    pub contributions: Vec<AudioContribution>,
    pub coverage: Option<Coverage>,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AudioContribution {
    pub source_id: SourceId,
    pub audible: bool,
    pub gain_db: Option<f64>,
    pub pan: Option<f64>,
    pub semantic_role: Option<AudioRole>,
    pub confidence: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AudioRole {
    Dialogue,
    Content,
    Background,
    Music,
    Effect,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MediaHealth {
    pub intervals: Vec<HealthInterval>,
    #[serde(default)]
    pub extensions: Extensions,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HealthInterval {
    pub media_ref: HealthRef,
    pub session_range: SessionRange,
    pub status: HealthStatus,
    pub detail: Option<String>,
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HealthRef {
    pub asset_id: AssetId,
    pub stream_index: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum HealthStatus {
    Available,
    Missing,
    Truncated,
    Corrupt,
    DecodeError,
    TimestampGap,
    Unknown,
}
