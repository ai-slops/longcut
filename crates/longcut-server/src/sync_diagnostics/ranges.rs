use std::collections::BTreeMap;

use longcut_session_contract::model::TimeMapSegment;

use super::dto::{CoverageDiagnostic, ExactRange, StreamOverlap};

pub(super) struct RangeAnalysis {
    pub coverage: CoverageDiagnostic,
    pub overlaps: Vec<StreamOverlap>,
    pub has_gaps: bool,
}

pub(super) fn analyze(segments: &[TimeMapSegment], duration_ns: i64) -> RangeAnalysis {
    let domain_end = duration_ns.max(0);
    let intervals: Vec<(i64, i64)> = segments
        .iter()
        .filter_map(|segment| {
            let (start, end) = segment.session_range.bounds();
            let clamped = (start.clamp(0, domain_end), end.clamp(0, domain_end));
            (clamped.0 < clamped.1).then_some(clamped)
        })
        .collect();
    let merged = merge(intervals);
    let gaps = gaps(&merged, domain_end);
    let overlaps = stream_overlaps(segments, domain_end);
    RangeAnalysis {
        coverage: CoverageDiagnostic::Exact {
            domain: ExactRange::session_ns(0, domain_end),
            ranges: exact_ranges(&merged),
            gaps: exact_ranges(&gaps),
        },
        overlaps,
        has_gaps: !gaps.is_empty(),
    }
}

fn merge(mut ranges: Vec<(i64, i64)>) -> Vec<(i64, i64)> {
    ranges.sort_unstable();
    let mut merged: Vec<(i64, i64)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            Some(_) | None => merged.push((start, end)),
        }
    }
    merged
}

fn gaps(ranges: &[(i64, i64)], domain_end: i64) -> Vec<(i64, i64)> {
    let mut cursor = 0;
    let mut gaps = Vec::new();
    for &(start, end) in ranges {
        if cursor < start {
            gaps.push((cursor, start));
        }
        cursor = cursor.max(end);
    }
    if cursor < domain_end {
        gaps.push((cursor, domain_end));
    }
    gaps
}

fn stream_overlaps(segments: &[TimeMapSegment], domain_end: i64) -> Vec<StreamOverlap> {
    let mut streams: BTreeMap<(String, u32), Vec<(i64, i64)>> = BTreeMap::new();
    for segment in segments {
        let (start, end) = segment.session_range.bounds();
        let range = (start.clamp(0, domain_end), end.clamp(0, domain_end));
        if range.0 < range.1 {
            streams
                .entry((
                    segment.media_ref.asset_id.0.clone(),
                    segment.media_ref.stream_index,
                ))
                .or_default()
                .push(range);
        }
    }
    let mut overlaps = Vec::new();
    for ((asset_id, stream_index), mut ranges) in streams {
        ranges.sort_unstable();
        let mut prior_end = None;
        for (start, end) in ranges {
            if let Some(previous) = prior_end {
                if start < previous {
                    overlaps.push(StreamOverlap {
                        asset_id: asset_id.clone(),
                        stream_index,
                        session_range: ExactRange::session_ns(start, end.min(previous)),
                    });
                }
                prior_end = Some(previous.max(end));
            } else {
                prior_end = Some(end);
            }
        }
    }
    overlaps
}

fn exact_ranges(ranges: &[(i64, i64)]) -> Vec<ExactRange> {
    ranges
        .iter()
        .map(|&(start, end)| ExactRange::session_ns(start, end))
        .collect()
}
