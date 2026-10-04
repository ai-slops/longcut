use std::collections::BTreeMap;

use super::{Context, intervals::StreamRanges, unit, valid_range};
use crate::{Report, model::*};

pub(super) fn validate_health(context: &Context<'_>, maps: &StreamRanges, report: &mut Report) {
    let Some(track) = &context.package.media_health else {
        report.warning(
            "MEDIA_HEALTH_MISSING",
            "media-health.json",
            "media availability is unknown",
        );
        return;
    };
    let mut states: BTreeMap<MediaRef, Vec<&HealthInterval>> = BTreeMap::new();
    let mut complete = true;
    for (position, interval) in track.intervals.iter().enumerate() {
        let path = format!("media-health.json/intervals/{position}");
        if !valid_range(&interval.session_range, context.duration) || !unit(interval.confidence) {
            report.error("INVALID_INTERVAL", &path, "invalid range or confidence");
            continue;
        }
        let matching: Vec<_> = context
            .index
            .streams
            .keys()
            .filter(|key| {
                key.asset_id == interval.media_ref.asset_id
                    && interval
                        .media_ref
                        .stream_index
                        .is_none_or(|stream| stream == key.stream_index)
            })
            .collect();
        if matching.is_empty() {
            report.error(
                "UNKNOWN_STREAM",
                &path,
                "health interval must resolve to registered streams",
            );
        }
        for key in matching {
            states.entry(key.clone()).or_default().push(interval);
        }
        match interval.status {
            HealthStatus::Available => {}
            HealthStatus::Missing
            | HealthStatus::Truncated
            | HealthStatus::Corrupt
            | HealthStatus::DecodeError
            | HealthStatus::TimestampGap
            | HealthStatus::Unknown => report.warning(
                "MEDIA_UNAVAILABLE",
                &path,
                "declared media damage, absence or uncertainty requires review",
            ),
        }
    }
    for (key, intervals) in &mut states {
        intervals.sort_by_key(|interval| interval.session_range.bounds());
        for pair in intervals.windows(2) {
            if pair[0].session_range.bounds().1 > pair[1].session_range.bounds().0 {
                report.error(
                    "HEALTH_OVERLAP",
                    "media-health.json",
                    format!(
                        "overlapping asset/stream health for {}/{}",
                        key.asset_id.0, key.stream_index
                    ),
                );
            }
        }
    }
    for (key, ranges) in maps {
        let intervals = states.get(key).map(Vec::as_slice).unwrap_or(&[]);
        for &range in ranges {
            if !covered(
                range,
                intervals
                    .iter()
                    .map(|interval| interval.session_range.bounds()),
            ) {
                complete = false;
                report.warning(
                    "HEALTH_COVERAGE_GAP",
                    "media-health.json",
                    format!(
                        "mapped stream {}/{} has unknown health",
                        key.asset_id.0, key.stream_index
                    ),
                );
            }
            for interval in intervals {
                let (start, end) = interval.session_range.bounds();
                if start < range.1
                    && range.0 < end
                    && matches!(
                        interval.status,
                        HealthStatus::Missing | HealthStatus::TimestampGap
                    )
                {
                    report.error(
                        "HEALTH_MAP_CONFLICT",
                        "media-health.json",
                        "missing media cannot also have a real time mapping",
                    );
                }
            }
        }
        for pair in ranges.windows(2) {
            let gap = (pair[0].1, pair[1].0);
            if gap.0 < gap.1
                && !covered(
                    gap,
                    intervals
                        .iter()
                        .filter(|interval| {
                            matches!(
                                interval.status,
                                HealthStatus::Missing | HealthStatus::TimestampGap
                            )
                        })
                        .map(|interval| interval.session_range.bounds()),
                )
            {
                complete = false;
                report.warning(
                    "HEALTH_GAP_UNDECLARED",
                    "media-health.json",
                    "time map gaps require explicit missing or timestamp-gap health",
                );
            }
        }
        for interval in intervals {
            if interval.status == HealthStatus::Available
                && !covered(interval.session_range.bounds(), ranges.iter().copied())
            {
                report.error(
                    "HEALTH_MAP_CONFLICT",
                    "media-health.json",
                    "available media must have a matching time map",
                );
            }
        }
    }
    report.capabilities.media_health_ranges =
        complete && !maps.is_empty() && report.capabilities.session_time_map;
}

fn covered(target: (i64, i64), ranges: impl Iterator<Item = (i64, i64)>) -> bool {
    let mut cursor = target.0;
    for (start, end) in ranges {
        if end <= cursor {
            continue;
        }
        if start > cursor {
            return false;
        }
        cursor = end;
        if cursor >= target.1 {
            return true;
        }
    }
    false
}
