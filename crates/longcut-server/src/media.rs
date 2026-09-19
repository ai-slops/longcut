use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use serde::Deserialize;
use tokio::{fs, process::Command, time::timeout};

use crate::{
    error::{AppError, AppResult},
    model::{MediaStatus, Source, SourceRole},
};

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
    if duration <= 0.0 {
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
    let original = artifacts.join(artifact_name(&source.media_url)?);
    let proxy_name = format!("{}-proxy.mp4", source.id);
    let proxy_temp = artifacts.join(format!(".{proxy_name}.tmp.mp4"));
    run_ffmpeg(
        Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(&original)
            .args([
                "-vf",
                "scale=1280:-2",
                "-c:v",
                "libx264",
                "-preset",
                "veryfast",
                "-crf",
                "25",
                "-c:a",
                "aac",
                "-b:a",
                "128k",
                "-movflags",
                "+faststart",
                "-y",
            ])
            .arg(&proxy_temp),
        "proxy",
    )
    .await?;
    fs::rename(&proxy_temp, artifacts.join(&proxy_name)).await?;
    let thumbnail_name = format!("{}-thumbnail.jpg", source.id);
    let thumbnail_temp = artifacts.join(format!(".{thumbnail_name}.tmp.jpg"));
    run_ffmpeg(
        Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-ss", "0", "-i"])
            .arg(&original)
            .args(["-frames:v", "1", "-vf", "scale=640:-2", "-y"])
            .arg(&thumbnail_temp),
        "thumbnail",
    )
    .await?;
    fs::rename(&thumbnail_temp, artifacts.join(&thumbnail_name)).await?;
    let waveform = if source.has_audio {
        extract_waveform(&original).await?
    } else {
        vec![]
    };
    let mut ready = source.clone();
    ready.proxy_url = Some(format!("/api/v1/media/{proxy_name}"));
    ready.thumbnail_url = Some(format!("/api/v1/media/{thumbnail_name}"));
    ready.waveform = waveform;
    ready.status = MediaStatus::Ready;
    Ok(ready)
}

async fn run_ffmpeg(command: &mut Command, label: &str) -> AppResult<()> {
    command.kill_on_drop(true);
    let output = timeout(
        Duration::from_secs(300),
        command.stderr(Stdio::piped()).output(),
    )
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("{label} timed out")))??;
    if !output.status.success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "{label} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

async fn extract_waveform(path: &Path) -> AppResult<Vec<f32>> {
    let output = timeout(
        Duration::from_secs(120),
        Command::new("ffmpeg")
            .kill_on_drop(true)
            .args(["-hide_banner", "-loglevel", "error", "-i"])
            .arg(path)
            .args(["-vn", "-ac", "1", "-ar", "8000", "-f", "s16le", "-"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("waveform timed out")))??;
    if !output.status.success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "waveform failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let (pairs, _) = output.stdout.as_chunks::<2>();
    let samples: Vec<f32> = pairs
        .iter()
        .map(|pair| f32::from(i16::from_le_bytes(*pair)).abs() / 32768.0)
        .collect();
    if samples.is_empty() {
        return Ok(vec![]);
    }
    let chunk = samples.len().div_ceil(512);
    Ok(samples
        .chunks(chunk)
        .map(|group| group.iter().copied().fold(0.0, f32::max))
        .collect())
}

pub async fn tool_available(name: &str) -> bool {
    Command::new(name)
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
