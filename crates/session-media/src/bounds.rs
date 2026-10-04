use std::cmp::Ordering;

use longcut_session_contract::model::TimeMapSegment;
use longcut_timebase::{SourceTime, TimeError};

use crate::{
    Check, Issue, PacketBounds, SegmentBounds, packets::Scan, probe::Probe,
    report::aggregate_bounds, streams::parse_base,
};

#[cfg(test)]
mod tests;

pub(crate) struct BoundsReport {
    pub check: Check,
    pub packets: Vec<PacketBounds>,
    pub segments: Vec<SegmentBounds>,
    pub issues: Vec<Issue>,
}

pub(crate) fn compare(
    segments: &[(usize, &TimeMapSegment)],
    probe: &Probe,
    scan: &Scan,
) -> BoundsReport {
    let mut report = BoundsReport {
        check: Check::NotDeclared,
        packets: Vec::new(),
        segments: Vec::new(),
        issues: Vec::new(),
    };
    for (&index, envelope) in scan {
        let time_base = probe
            .streams
            .iter()
            .find(|stream| stream.index == index)
            .and_then(|stream| stream.time_base.clone());
        let timing_complete =
            envelope.complete && time_base.as_deref().and_then(parse_base).is_some();
        let coverage = if !timing_complete || !envelope.coverage_resolved || envelope.count == 0 {
            Check::Unverifiable
        } else if envelope.gap_count > 0 {
            Check::Failed
        } else {
            Check::Passed
        };
        report.packets.push(PacketBounds {
            stream_index: index,
            start_pts: envelope.start.map(|value| value.to_string()),
            end_pts: envelope.end.map(|value| value.to_string()),
            time_base,
            packet_count: envelope.count.to_string(),
            timing_complete,
            coverage,
            pts_ordered: envelope.ordered,
            gap_count: envelope.gap_count.to_string(),
            overlap_count: envelope.overlap_count.to_string(),
            max_gap_pts: envelope.max_gap.map(|value| value.to_string()),
        });
    }
    for &(index, segment) in segments {
        let stream_index = segment.media_ref.stream_index;
        let times = (|| {
            let envelope = scan.get(&stream_index)?;
            if !envelope.complete {
                return None;
            }
            let base = probe
                .streams
                .iter()
                .find(|stream| stream.index == stream_index)?
                .time_base
                .as_deref()
                .and_then(parse_base)?;
            Some((
                SourceTime::new(envelope.start?, base),
                SourceTime::new(envelope.end?, base),
            ))
        })();
        let inside = times.map(|(start, end)| -> Result<bool, TimeError> {
            Ok(
                segment.media_range.start.source_time()?.compare(start)? != Ordering::Less
                    && segment.media_range.end.source_time()?.compare(end)? != Ordering::Greater,
            )
        });
        let check = match inside {
            Some(Ok(true)) => Check::Passed,
            Some(Ok(false)) => {
                report.issues.push(Issue { code: "TIME_MAP_RANGE_OUT_OF_BOUNDS", message: format!("time-maps.json/segments/{index} exceeds stream {stream_index}'s observed packet envelope") });
                Check::Failed
            }
            None | Some(Err(_)) => {
                report.issues.push(Issue { code: "TIME_MAP_BOUNDS_UNVERIFIABLE", message: format!("time-maps.json/segments/{index} lacks complete exact packet timing or its comparison overflows") });
                Check::Unverifiable
            }
        };
        report.segments.push(SegmentBounds {
            segment_index: index,
            stream_index,
            check,
        });
    }
    report.check = aggregate_bounds(report.segments.iter().map(|segment| segment.check));
    report
}
