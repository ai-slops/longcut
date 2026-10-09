mod budget;
mod dto;
pub mod http;
mod ranges;
mod rational;

pub use dto::SyncDiagnostics;

use std::collections::{BTreeMap, BTreeSet};

use longcut_session_contract::model::{Binding, TimeMapSegment};

use crate::model::SourceSessionRecord;
use dto::{
    AdapterCapability, BindingDiagnostic, CalculationLimits, CoverageDiagnostic, ExactValue,
    SegmentDiagnostic, SourceDiagnostic,
};
use rational::{ArithmeticError, Rational};

pub fn calculate(record: &SourceSessionRecord) -> SyncDiagnostics {
    let segments = record
        .package
        .time_maps
        .as_ref()
        .map(|maps| maps.segments.as_slice());
    let limits = budget::measure(record, segments);
    let capability = AdapterCapability {
        exact_time_maps: segments.is_some(),
        ordinary_probe: "metadata_only_not_exact",
        measured_av_alignment: false,
        media_mutation: false,
    };
    if limits.exceeded {
        return SyncDiagnostics {
            package_id: record.summary.id.clone(),
            session_id: record.summary.session_id.clone(),
            relation: "metadata_relation_unmeasured_alignment",
            calculation_status: "unverifiable",
            adapter_capability: capability,
            limits,
            sources: Vec::new(),
            coverage: CoverageDiagnostic::Unverifiable {
                reason: "validation_limit_exceeded",
            },
            overlaps: Vec::new(),
            health_notes: Vec::new(),
        };
    }
    build(record, segments, limits, capability)
}

fn build(
    record: &SourceSessionRecord,
    segments: Option<&[TimeMapSegment]>,
    limits: CalculationLimits,
    adapter_capability: AdapterCapability,
) -> SyncDiagnostics {
    let index = segment_index(segments.unwrap_or_default());
    let sources: Vec<SourceDiagnostic> = record
        .package
        .registry
        .sources
        .iter()
        .map(|source| {
            let editor_source_ids: BTreeSet<String> = record
                .mappings
                .iter()
                .filter(|mapping| mapping.package_source_id == source.source_id.0)
                .map(|mapping| mapping.editor_source_id.clone())
                .chain(
                    record
                        .summary
                        .sources
                        .iter()
                        .filter(|summary| summary.source_id == source.source_id.0)
                        .filter_map(|summary| summary.editor_source_id.clone()),
                )
                .collect();
            let bindings = source
                .media_bindings
                .iter()
                .map(|binding| binding_diagnostic(binding, &index))
                .collect();
            SourceDiagnostic {
                package_source_id: source.source_id.0.clone(),
                display_name: source.display_name.clone(),
                semantic_role: source.semantic_role.clone(),
                editor_binding_available: !editor_source_ids.is_empty(),
                editor_source_ids: editor_source_ids.into_iter().collect(),
                bindings,
            }
        })
        .collect();
    let (coverage, overlaps, health_notes) = match segments {
        None => (
            CoverageDiagnostic::Unknown {
                reason: "time_maps_missing",
            },
            Vec::new(),
            Vec::new(),
        ),
        Some(items) => match record.package.manifest.duration.nanoseconds() {
            Ok(duration) => {
                let analysis = ranges::analyze(items, duration);
                let notes = if analysis.has_gaps {
                    vec!["observed_metadata_gap_not_media_corruption"]
                } else {
                    Vec::new()
                };
                (analysis.coverage, analysis.overlaps, notes)
            }
            Err(_) => (
                CoverageDiagnostic::Unverifiable {
                    reason: "declared_duration_unverifiable",
                },
                Vec::new(),
                Vec::new(),
            ),
        },
    };
    let calculation_status =
        if coverage.is_unverifiable() || sources.iter().any(SourceDiagnostic::is_unverifiable) {
            "unverifiable"
        } else {
            "complete"
        };
    SyncDiagnostics {
        package_id: record.summary.id.clone(),
        session_id: record.summary.session_id.clone(),
        relation: "metadata_relation_unmeasured_alignment",
        calculation_status,
        adapter_capability,
        limits,
        sources,
        coverage,
        overlaps,
        health_notes,
    }
}

fn segment_index(segments: &[TimeMapSegment]) -> BTreeMap<(&str, u32), Vec<&TimeMapSegment>> {
    let mut index: BTreeMap<(&str, u32), Vec<&TimeMapSegment>> = BTreeMap::new();
    for segment in segments {
        index
            .entry((
                &segment.media_ref.asset_id.0,
                segment.media_ref.stream_index,
            ))
            .or_default()
            .push(segment);
    }
    index
}

fn binding_diagnostic(
    binding: &Binding,
    index: &BTreeMap<(&str, u32), Vec<&TimeMapSegment>>,
) -> BindingDiagnostic {
    let segments = index
        .get(&(binding.asset_id.0.as_str(), binding.stream_index))
        .into_iter()
        .flat_map(|items| items.iter())
        .map(|segment| segment_diagnostic(segment))
        .collect();
    BindingDiagnostic {
        asset_id: binding.asset_id.0.clone(),
        stream_index: binding.stream_index,
        package_binding_available: true,
        segments,
    }
}

fn segment_diagnostic(segment: &TimeMapSegment) -> SegmentDiagnostic {
    let media_start = Rational::media(&segment.media_range.start);
    let media_end = Rational::media(&segment.media_range.end);
    let (session_start_ns, session_end_ns) = segment.session_range.bounds();
    let session_start = Rational::session_ns(session_start_ns);
    let session_end = Rational::session_ns(session_end_ns);
    let media_duration = subtract(media_end, media_start);
    let session_duration = subtract(session_end, session_start);
    let ratio = divide(session_duration, media_duration);
    let ppm = ratio.and_then(|value| {
        value
            .checked_sub(Rational::new(1, 1)?)
            .and_then(|delta| delta.checked_mul_integer(1_000_000))
    });
    SegmentDiagnostic {
        original_media_start: ExactValue::from_result(media_start),
        original_media_end: ExactValue::from_result(media_end),
        session_start: ExactValue::from_result(session_start),
        session_end: ExactValue::from_result(session_end),
        confidence: segment.confidence.to_string(),
        offset_at_start: ExactValue::from_result(subtract(session_start, media_start)),
        duration_delta: ExactValue::from_result(subtract(session_duration, media_duration)),
        relative_rate_ratio: ExactValue::from_result(ratio),
        relative_rate_ppm: ExactValue::from_result(ppm),
    }
}

fn subtract(
    left: Result<Rational, ArithmeticError>,
    right: Result<Rational, ArithmeticError>,
) -> Result<Rational, ArithmeticError> {
    left.and_then(|left| right.and_then(|right| left.checked_sub(right)))
}

fn divide(
    left: Result<Rational, ArithmeticError>,
    right: Result<Rational, ArithmeticError>,
) -> Result<Rational, ArithmeticError> {
    left.and_then(|left| right.and_then(|right| left.checked_div(right)))
}
