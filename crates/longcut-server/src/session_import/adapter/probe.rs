use std::{path::Path, time::Duration};

use longcut_session_contract::model::MediaTime;
use longcut_timebase::{SourceTime, TimeBase};
use serde::Deserialize;
use tokio::process::Command;

use crate::error::{AppError, AppResult};

#[derive(Deserialize)]
struct AdapterProbe {
    streams: Vec<AdapterStream>,
    format: AdapterFormat,
}

#[derive(Deserialize)]
struct AdapterStream {
    codec_type: String,
    start_pts: Option<i64>,
}

#[derive(Deserialize)]
struct AdapterFormat {
    start_time: Option<String>,
}

#[derive(Deserialize)]
struct SourceProbe {
    packets: Vec<Packet>,
    streams: Vec<SourceStream>,
}

#[derive(Deserialize)]
struct Packet {
    pts: Option<i64>,
}

#[derive(Deserialize)]
struct SourceStream {
    time_base: Option<String>,
}

pub(super) async fn verify_source_start(
    path: &Path,
    stream_index: u32,
    expected: &MediaTime,
) -> AppResult<()> {
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new("ffprobe")
            .kill_on_drop(true)
            .args([
                "-v",
                "error",
                "-select_streams",
                &stream_index.to_string(),
                "-read_intervals",
                "%+#1",
                "-show_packets",
                "-show_entries",
                "packet=pts:stream=time_base",
                "-of",
                "json",
            ])
            .arg(path)
            .output(),
    )
    .await
    .map_err(|_| AppError::Invalid("source packet probe timed out".into()))??;
    if !output.status.success() || output.stdout.len() > 64 * 1024 {
        return Err(AppError::Invalid("source packet probe failed".into()));
    }
    let probe: SourceProbe = serde_json::from_slice(&output.stdout)?;
    let pts = probe
        .packets
        .first()
        .and_then(|packet| packet.pts)
        .ok_or_else(|| AppError::Invalid("source video has no packet PTS".into()))?;
    let time_base = probe
        .streams
        .first()
        .and_then(|stream| stream.time_base.as_deref())
        .and_then(|value| value.split_once('/'))
        .ok_or_else(|| AppError::Invalid("source video time base unavailable".into()))?;
    let numerator = time_base
        .0
        .parse::<i64>()
        .map_err(|_| AppError::Invalid("invalid source video time base".into()))?;
    let denominator = time_base
        .1
        .parse::<i64>()
        .map_err(|_| AppError::Invalid("invalid source video time base".into()))?;
    let observed = SourceTime::new(
        pts,
        TimeBase::new(numerator, denominator)
            .map_err(|error| AppError::Invalid(error.to_string()))?,
    );
    let expected = expected
        .source_time()
        .map_err(|error| AppError::Invalid(error.to_string()))?;
    if observed
        .compare(expected)
        .map_err(|error| AppError::Invalid(error.to_string()))?
        != std::cmp::Ordering::Equal
    {
        return Err(AppError::Invalid(
            "mapped video range must start at the first packet PTS".into(),
        ));
    }
    Ok(())
}

pub(super) async fn verify_origin(path: &Path, count: usize) -> AppResult<()> {
    let result = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new("ffprobe")
            .kill_on_drop(true)
            .args([
                "-v",
                "error",
                "-show_entries",
                "stream=codec_type,start_pts:format=start_time",
                "-of",
                "json",
            ])
            .arg(path)
            .output(),
    )
    .await
    .map_err(|_| AppError::Invalid("adapter probe timed out".into()))??;
    if !result.status.success() || result.stdout.len() > 64 * 1024 {
        return Err(AppError::Invalid("adapter cannot be probed".into()));
    }
    let probe: AdapterProbe = serde_json::from_slice(&result.stdout)?;
    if probe.streams.len() != count
        || probe
            .streams
            .first()
            .is_none_or(|stream| stream.codec_type != "video" || stream.start_pts != Some(0))
        || probe.format.start_time.as_deref() != Some("0.000000")
    {
        return Err(AppError::Invalid(
            "adapter is not a verified zero-origin single-video source".into(),
        ));
    }
    Ok(())
}
