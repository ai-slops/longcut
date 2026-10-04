use longcut_session_contract::model::{Asset, StreamKind};
use longcut_timebase::TimeBase;

use crate::{AssetVerification, Check, probe::Probe};

pub(crate) fn compare(asset: &Asset, actual: &Probe, report: &mut AssetVerification) {
    let before = report.issues.len();
    for expected in &asset.streams {
        let Some(stream) = actual
            .streams
            .iter()
            .find(|stream| stream.index == expected.stream_index)
        else {
            report.issue(
                "STREAM_MISSING",
                format!("declared stream {} does not exist", expected.stream_index),
            );
            continue;
        };
        let kind = match expected.kind {
            StreamKind::Video => "video",
            StreamKind::Audio => "audio",
        };
        let actual_rate = stream
            .sample_rate
            .as_ref()
            .and_then(|value| value.parse::<u32>().ok());
        let usable = match expected.kind {
            StreamKind::Video => {
                stream.width.is_some_and(|value| value > 0)
                    && stream.height.is_some_and(|value| value > 0)
            }
            StreamKind::Audio => {
                actual_rate.is_some_and(|value| value > 0)
                    && stream.channels.is_some_and(|value| value > 0)
            }
        };
        let mismatch = !usable
            || stream.codec_type != kind
            || stream.codec_name.as_ref() != Some(&expected.codec)
            || expected
                .width
                .is_some_and(|value| stream.width != Some(value))
            || expected
                .height
                .is_some_and(|value| stream.height != Some(value))
            || expected
                .sample_rate
                .is_some_and(|value| actual_rate != Some(value))
            || expected
                .channels
                .is_some_and(|value| stream.channels != Some(value))
            || expected
                .channel_layout
                .as_ref()
                .is_some_and(|value| stream.channel_layout.as_ref() != Some(value));
        if mismatch {
            report.issue(
                "STREAM_METADATA_MISMATCH",
                format!(
                    "stream {} codec/kind/dimensions/audio metadata differs from declaration",
                    expected.stream_index
                ),
            );
        }
        if let Some(base) = &expected.time_base {
            let actual = stream.time_base.as_deref().and_then(parse_base);
            if base.time_base().ok() != actual || actual.is_none() {
                report.issue(
                    "TIME_BASE_MISMATCH",
                    format!(
                        "stream {} time base differs from declaration",
                        expected.stream_index
                    ),
                );
            }
        }
        if expected.frame_rate.is_some() {
            report
                .unchecked_fields
                .push(format!("streams/{}/frame_rate", expected.stream_index));
        }
        if expected.orientation.is_some() {
            report
                .unchecked_fields
                .push(format!("streams/{}/orientation", expected.stream_index));
        }
    }
    report.stream_metadata = if report.issues.len() == before {
        Check::Passed
    } else {
        Check::Failed
    };
}

pub(crate) fn parse_base(value: &str) -> Option<TimeBase> {
    let (num, den) = value.split_once('/')?;
    TimeBase::new(num.parse().ok()?, den.parse().ok()?).ok()
}
