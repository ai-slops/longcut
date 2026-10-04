use std::collections::BTreeMap;

use longcut_timebase::{Segment, SegmentSpec, SessionTime, StreamKey, TimeMap};

use super::{Context, unit, valid_range};
use crate::{Report, model::*};

pub(super) type StreamRanges = BTreeMap<MediaRef, Vec<(i64, i64)>>;

pub(super) fn time_maps(context: &Context<'_>, report: &mut Report) -> StreamRanges {
    let mut ranges = StreamRanges::new();
    let Some(track) = &context.package.time_maps else {
        report.warning(
            "TIME_MAPS_MISSING",
            "time-maps.json",
            "no session mapping provided",
        );
        return ranges;
    };
    let before = report.diagnostics.len();
    let mut segments = Vec::new();
    for (position, entry) in track.segments.iter().enumerate() {
        let path = format!("time-maps.json/segments/{position}");
        if !context.index.streams.contains_key(&entry.media_ref) {
            report.error(
                "UNKNOWN_STREAM",
                &path,
                "time map must reference a registered stream",
            );
            continue;
        }
        if !valid_range(&entry.session_range, context.duration) || !unit(entry.confidence) {
            report.error(
                "INVALID_INTERVAL",
                &path,
                "invalid session range or confidence",
            );
            continue;
        }
        let segment = (|| -> Result<Segment, longcut_timebase::MapError> {
            let (start, end) = entry.session_range.bounds();
            Segment::new(SegmentSpec {
                stream: StreamKey::new(
                    &entry.media_ref.asset_id.0,
                    &entry.media_ref.stream_index.to_string(),
                )?,
                media_start: entry.media_range.start.source_time()?,
                media_end: entry.media_range.end.source_time()?,
                session_start: SessionTime::new(start),
                session_end: SessionTime::new(end),
            })
        })();
        match segment {
            Ok(value) => {
                segments.push(value);
                ranges
                    .entry(entry.media_ref.clone())
                    .or_default()
                    .push(entry.session_range.bounds());
            }
            Err(error) => report.error("INVALID_TIME_MAP", &path, error.to_string()),
        }
    }
    if let Err(error) = TimeMap::new(segments) {
        report.error(
            "INVALID_TIME_MAP",
            "time-maps.json/segments",
            error.to_string(),
        );
    }
    for source in context.index.sources.values() {
        for binding in &source.media_bindings {
            if !ranges.contains_key(&MediaRef {
                asset_id: binding.asset_id.clone(),
                stream_index: binding.stream_index,
            }) {
                report.warning(
                    "STREAM_TIME_MAP_MISSING",
                    "time-maps.json",
                    format!(
                        "no mapping for {}/{}",
                        binding.asset_id.0, binding.stream_index
                    ),
                );
            }
        }
    }
    for values in ranges.values_mut() {
        values.sort_unstable();
    }
    if let Some(program) = context
        .package
        .manifest
        .program_reference_source_id
        .as_ref()
        .and_then(|id| context.index.sources.get(id))
    {
        for kind in [StreamKind::Video, StreamKind::Audio] {
            let mut reference_ranges = Vec::new();
            for binding in &program.media_bindings {
                let key = MediaRef {
                    asset_id: binding.asset_id.clone(),
                    stream_index: binding.stream_index,
                };
                if context
                    .index
                    .streams
                    .get(&key)
                    .is_some_and(|stream| stream.kind == kind)
                {
                    reference_ranges.extend(ranges.get(&key).into_iter().flatten().copied());
                }
            }
            reference_ranges.sort_unstable();
            let mut cursor = 0;
            for (start, end) in reference_ranges {
                if start > cursor {
                    break;
                }
                cursor = cursor.max(end);
            }
            if cursor < context.duration {
                report.warning(
                    "PROGRAM_REFERENCE_COVERAGE_GAP",
                    "time-maps.json",
                    "Program reference video and audio must each cover the complete session",
                );
            }
        }
    }
    report.capabilities.session_time_map = before == report.diagnostics.len();
    ranges
}

pub(super) fn track_coverage(
    ranges: &mut [(i64, i64)],
    duration: i64,
    path: &str,
) -> Result<bool, String> {
    ranges.sort_unstable();
    let mut cursor = 0;
    let mut covered = true;
    for &(start, end) in ranges.iter() {
        if start < cursor {
            return Err(format!("overlapping intervals in {path}"));
        }
        covered &= start == cursor;
        cursor = end;
    }
    Ok(covered && cursor == duration)
}
