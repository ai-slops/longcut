use std::{path::Path, process::Stdio, time::Duration};

use longcut_session_contract::model::{
    AssetId, Binding, Integer, Rational, SourceKind, Stream, StreamKind,
};
use serde::Deserialize;
use tokio::process::Command;

use crate::error::{AppError, AppResult};

pub(super) struct MediaProbe {
    pub duration_ns: i64,
    pub streams: Vec<Stream>,
    pub bindings: Vec<Binding>,
    pub kind: SourceKind,
}

#[derive(Deserialize)]
struct Output {
    streams: Vec<ProbeStream>,
    format: ProbeFormat,
}

#[derive(Deserialize)]
struct ProbeFormat {
    duration: String,
}

#[derive(Deserialize)]
struct ProbeStream {
    index: u32,
    codec_type: String,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    channel_layout: Option<String>,
    time_base: Option<String>,
    tags: Option<Tags>,
    #[serde(default)]
    side_data_list: Vec<SideData>,
}

#[derive(Deserialize)]
struct Tags {
    rotate: Option<String>,
}

#[derive(Deserialize)]
struct SideData {
    rotation: Option<i32>,
}

pub(super) async fn inspect(path: &Path, asset_id: &AssetId) -> AppResult<MediaProbe> {
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        Command::new("ffprobe")
            .kill_on_drop(true)
            .args([
                "-v",
                "error",
                "-protocol_whitelist",
                "file,pipe",
                "-format_whitelist",
                "mov,matroska,webm,avi,wav,flac,mp3,ogg,aac,mpegts",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
            ])
            .arg(path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| AppError::Invalid("ordinary media probe timed out".into()))??;
    if !output.status.success() {
        return Err(AppError::Invalid(format!(
            "ordinary media probe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let probe: Output = serde_json::from_slice(&output.stdout)
        .map_err(|error| AppError::Invalid(format!("invalid ordinary media probe: {error}")))?;
    let duration_ns = duration_ns(&probe.format.duration)?;
    let mut streams = Vec::new();
    let mut bindings = Vec::new();
    let mut video = false;
    let mut audio = false;
    for value in probe.streams {
        let kind = match value.codec_type.as_str() {
            "video" => {
                video = true;
                StreamKind::Video
            }
            "audio" => {
                audio = true;
                StreamKind::Audio
            }
            _ => continue,
        };
        let codec = value
            .codec_name
            .filter(|name| !name.is_empty())
            .ok_or_else(|| AppError::Invalid("media stream codec is unavailable".into()))?;
        let orientation = value
            .side_data_list
            .iter()
            .find_map(|data| data.rotation)
            .or_else(|| value.tags.and_then(|tags| tags.rotate?.parse().ok()));
        streams.push(Stream {
            stream_index: value.index,
            kind,
            codec,
            width: value.width,
            height: value.height,
            frame_rate: value.r_frame_rate.as_deref().and_then(rational),
            sample_rate: value.sample_rate.and_then(|rate| rate.parse().ok()),
            channels: value.channels,
            channel_layout: value.channel_layout,
            time_base: value.time_base.as_deref().and_then(rational),
            orientation,
        });
        bindings.push(Binding {
            asset_id: asset_id.clone(),
            stream_index: value.index,
            channel_indices: None,
        });
    }
    let kind = match (video, audio) {
        (true, true) => SourceKind::Av,
        (true, false) => SourceKind::Video,
        (false, true) => SourceKind::Audio,
        (false, false) => {
            return Err(AppError::Invalid(
                "ordinary media has no audio or video stream".into(),
            ));
        }
    };
    Ok(MediaProbe {
        duration_ns,
        streams,
        bindings,
        kind,
    })
}

fn rational(value: &str) -> Option<Rational> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator = numerator.parse::<i64>().ok()?;
    let denominator = denominator.parse::<i64>().ok()?;
    (numerator > 0 && denominator > 0).then_some(Rational {
        numerator: Integer::new(numerator),
        denominator: Integer::new(denominator),
    })
}

fn duration_ns(value: &str) -> AppResult<i64> {
    let (seconds, fraction) = value.split_once('.').unwrap_or((value, ""));
    let seconds = seconds
        .parse::<i64>()
        .map_err(|_| AppError::Invalid("media duration is unavailable".into()))?;
    if seconds < 0 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AppError::Invalid("media duration is unavailable".into()));
    }
    let digits = fraction.get(..fraction.len().min(9)).unwrap_or(fraction);
    let fraction = if digits.is_empty() {
        0
    } else {
        digits
            .parse::<i64>()
            .map_err(|_| AppError::Invalid("media duration is unavailable".into()))?
            .checked_mul(10_i64.pow(u32::try_from(9 - digits.len()).map_err(|_| {
                AppError::Invalid("media duration precision is unsupported".into())
            })?))
            .ok_or_else(|| AppError::Invalid("media duration is too large".into()))?
    };
    let duration = seconds
        .checked_mul(1_000_000_000)
        .and_then(|whole| whole.checked_add(fraction))
        .ok_or_else(|| AppError::Invalid("media duration is too large".into()))?;
    if duration == 0 {
        return Err(AppError::Invalid("media has no positive duration".into()));
    }
    Ok(duration)
}
