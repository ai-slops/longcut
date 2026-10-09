use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde::Deserialize;
use tokio::{fs, process::Command, time::timeout};
use tokio_util::sync::CancellationToken;

use crate::{
    error::{AppError, AppResult},
    model::{MediaStatus, Source, SourceRole},
};

mod derivative_pipeline;
mod process;
mod waveform;

#[derive(Debug, Deserialize)]
struct Probe {
    streams: Vec<ProbeStream>,
    format: ProbeFormat,
}
#[derive(Debug, Deserialize)]
struct ProbeStream {
    codec_type: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
}
#[derive(Debug, Deserialize)]
struct ProbeFormat {
    duration: Option<String>,
}

pub async fn probe_source(
    path: &Path,
    name: &str,
    role: SourceRole,
    frame_rate: u32,
    artifact: &str,
) -> AppResult<Source> {
    let output = timeout(
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
    .map_err(|_| AppError::Invalid("ffprobe timed out".into()))??;
    if !output.status.success() {
        return Err(AppError::Invalid(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let probe: Probe = serde_json::from_slice(&output.stdout)
        .map_err(|error| AppError::Invalid(format!("invalid ffprobe output: {error}")))?;
    let video = probe
        .streams
        .iter()
        .find(|stream| stream.codec_type.as_deref() == Some("video"));
    let duration = probe
        .format
        .duration
        .as_deref()
        .unwrap_or("0")
        .parse::<f64>()
        .map_err(|_| AppError::Invalid("media duration unavailable".into()))?;
    if !duration.is_finite() || duration <= 0.0 {
        return Err(AppError::Invalid("media has no positive duration".into()));
    }
    let id = uuid::Uuid::new_v4().to_string();
    Ok(Source {
        id,
        name: name.into(),
        role,
        duration_frames: (duration * f64::from(frame_rate)).ceil().to_string(),
        width: video.and_then(|stream| stream.width).unwrap_or(0),
        height: video.and_then(|stream| stream.height).unwrap_or(0),
        has_audio: probe
            .streams
            .iter()
            .any(|stream| stream.codec_type.as_deref() == Some("audio")),
        media_url: format!("/api/v1/media/{artifact}"),
        proxy_url: None,
        thumbnail_url: None,
        waveform: vec![],
        status: MediaStatus::Processing,
        error: None,
    })
}

pub async fn generate_demo(artifacts: &Path) -> AppResult<(PathBuf, String)> {
    let artifact = format!("{}-synthetic-original.mp4", uuid::Uuid::new_v4());
    let final_path = artifacts.join(&artifact);
    let temp = artifacts.join(format!(".{artifact}.tmp.mp4"));
    let output = timeout(Duration::from_secs(90), Command::new("ffmpeg").kill_on_drop(true).args([
        "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i",
        "testsrc2=size=1920x1080:rate=30,drawtext=text='SYNTHETIC DEMO':fontcolor=white:fontsize=72:x=(w-text_w)/2:y=80",
        "-f", "lavfi", "-i", "sine=frequency=880:sample_rate=48000", "-t", "8", "-shortest",
        "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p", "-c:a", "aac", "-movflags", "+faststart", "-y",
    ]).arg(&temp).stderr(Stdio::piped()).output()).await.map_err(|_| AppError::Internal(anyhow::anyhow!("demo generation timed out")))??;
    if !output.status.success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "ffmpeg demo failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    fs::rename(&temp, &final_path).await?;
    Ok((final_path, artifact))
}

pub async fn derivatives(artifacts: &Path, source: &Source) -> AppResult<Source> {
    derivative_pipeline::generate(artifacts, source, &CancellationToken::new()).await
}

pub async fn derivatives_with_cancel(
    artifacts: &Path,
    source: &Source,
    cancel: &CancellationToken,
) -> AppResult<Source> {
    derivative_pipeline::generate(artifacts, source, cancel).await
}

pub async fn tool_available(name: &str) -> bool {
    tool_available_path(Path::new(name)).await
}

pub async fn tool_available_path(path: &Path) -> bool {
    Command::new(path)
        .kill_on_drop(true)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn artifact_name(url: &str) -> AppResult<&str> {
    url.strip_prefix("/api/v1/media/")
        .filter(|name| !name.is_empty() && !name.contains('/') && !name.contains(".."))
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("invalid persisted media URL")))
}
