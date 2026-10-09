use serde::Serialize;

use super::rational::{ArithmeticError, Rational};

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ExactValue {
    Exact {
        numerator: String,
        denominator: String,
    },
    Unknown {
        reason: &'static str,
    },
    Unverifiable {
        reason: &'static str,
    },
}

impl ExactValue {
    pub(super) fn from_result(value: Result<Rational, ArithmeticError>) -> Self {
        match value {
            Ok(value) => {
                let (numerator, denominator) = value.parts();
                Self::Exact {
                    numerator,
                    denominator,
                }
            }
            Err(ArithmeticError::Overflow) => Self::Unverifiable {
                reason: "arithmetic_overflow",
            },
            Err(ArithmeticError::ZeroDuration) => Self::Unverifiable {
                reason: "zero_physical_duration",
            },
        }
    }

    pub(super) fn session_ns(value: i64) -> Self {
        Self::from_result(Rational::session_ns(value))
    }

    const fn is_unverifiable(&self) -> bool {
        matches!(self, Self::Unverifiable { .. })
    }
}

#[derive(Debug, Serialize)]
pub struct ExactRange {
    pub start: ExactValue,
    pub end: ExactValue,
}

impl ExactRange {
    pub(super) fn session_ns(start: i64, end: i64) -> Self {
        Self {
            start: ExactValue::session_ns(start),
            end: ExactValue::session_ns(end),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SegmentDiagnostic {
    pub original_media_start: ExactValue,
    pub original_media_end: ExactValue,
    pub session_start: ExactValue,
    pub session_end: ExactValue,
    pub confidence: String,
    pub offset_at_start: ExactValue,
    pub duration_delta: ExactValue,
    pub relative_rate_ratio: ExactValue,
    pub relative_rate_ppm: ExactValue,
}

impl SegmentDiagnostic {
    pub(super) fn is_unverifiable(&self) -> bool {
        [
            &self.original_media_start,
            &self.original_media_end,
            &self.session_start,
            &self.session_end,
            &self.offset_at_start,
            &self.duration_delta,
            &self.relative_rate_ratio,
            &self.relative_rate_ppm,
        ]
        .into_iter()
        .any(ExactValue::is_unverifiable)
    }
}

#[derive(Debug, Serialize)]
pub struct BindingDiagnostic {
    pub asset_id: String,
    pub stream_index: u32,
    pub package_binding_available: bool,
    pub segments: Vec<SegmentDiagnostic>,
}

impl BindingDiagnostic {
    pub(super) fn is_unverifiable(&self) -> bool {
        self.segments.iter().any(SegmentDiagnostic::is_unverifiable)
    }
}

#[derive(Debug, Serialize)]
pub struct SourceDiagnostic {
    pub package_source_id: String,
    pub display_name: String,
    pub semantic_role: String,
    pub editor_binding_available: bool,
    pub editor_source_ids: Vec<String>,
    pub bindings: Vec<BindingDiagnostic>,
}

impl SourceDiagnostic {
    pub(super) fn is_unverifiable(&self) -> bool {
        self.bindings.iter().any(BindingDiagnostic::is_unverifiable)
    }
}

#[derive(Debug, Serialize)]
pub struct StreamOverlap {
    pub asset_id: String,
    pub stream_index: u32,
    pub session_range: ExactRange,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CoverageDiagnostic {
    Exact {
        domain: ExactRange,
        ranges: Vec<ExactRange>,
        gaps: Vec<ExactRange>,
    },
    Unknown {
        reason: &'static str,
    },
    Unverifiable {
        reason: &'static str,
    },
}

impl CoverageDiagnostic {
    pub(super) const fn is_unverifiable(&self) -> bool {
        matches!(self, Self::Unverifiable { .. })
    }
}

#[derive(Debug, Serialize)]
pub struct AdapterCapability {
    pub exact_time_maps: bool,
    pub ordinary_probe: &'static str,
    pub measured_av_alignment: bool,
    pub media_mutation: bool,
}

#[derive(Debug, Serialize)]
pub struct CalculationLimits {
    pub max_items: String,
    pub max_work: String,
    pub observed_items: String,
    pub observed_work: String,
    pub exceeded: bool,
}

#[derive(Debug, Serialize)]
pub struct SyncDiagnostics {
    pub package_id: String,
    pub session_id: String,
    pub relation: &'static str,
    pub calculation_status: &'static str,
    pub adapter_capability: AdapterCapability,
    pub limits: CalculationLimits,
    pub sources: Vec<SourceDiagnostic>,
    pub coverage: CoverageDiagnostic,
    pub overlaps: Vec<StreamOverlap>,
    pub health_notes: Vec<&'static str>,
}
