mod plan;

use longcut_session_contract::model::Source as PackageSource;
use serde::Deserialize;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};
use tokio_util::sync::CancellationToken;

use crate::{
    error::{AppError, AppResult},
    media,
    model::{EditorSourceMapping, Source},
};

pub(super) async fn build(
    context: &super::prepare::Context<'_>,
    source: &PackageSource,
) -> AppResult<(Source, EditorSourceMapping)> {
    let plan = plan::build(context.package, source, context.frame_rate)?;
    let name = format!("{}-session-source.mp4", uuid::Uuid::new_v4());
    let path = context.work.join(&name);
    let mut command = Command::new("ffmpeg");
    command.args(["-v", "error", "-nostdin", "-copyts"]);
    for binding in &plan.bindings {
        let asset = context
            .package
            .registry
            .assets
            .iter()
            .find(|asset| asset.asset_id == binding.asset_id)
            .ok_or(AppError::NotFound)?;
        command
            .args([
                "-protocol_whitelist",
                "file",
                "-format_whitelist",
                "mov,matroska,webm,avi,wav,flac,mp3,ogg,aac,mpegts",
                "-i",
            ])
            .arg(context.root.join(&asset.uri));
    }
    for (index, binding) in plan.bindings.iter().enumerate() {
        command.args(["-map", &format!("{index}:{}", binding.stream_index)]);
    }
    command
        .args([
            "-t",
            &plan.duration,
            "-c",
            "copy",
            "-avoid_negative_ts",
            "disabled",
            "-map_metadata",
            "-1",
            "-movflags",
            "+faststart",
            "-y",
        ])
        .arg(&path);
    run(command, context.token).await?;
    verify_origin(&path, plan.bindings.len()).await?;
    let mut editor = media::probe_source(
        &path,
        &source.display_name,
        plan.role,
        context.frame_rate,
        &name,
    )
    .await?;
    editor.duration_frames = plan.frames;
    editor = media::derivatives_with_cancel(context.work, &editor, context.token).await?;
    let mapping = EditorSourceMapping {
        editor_source_id: editor.id.clone(),
        package_source_id: source.source_id.0.clone(),
        bindings: plan.mappings,
        session_range: plan.session_range,
    };
    Ok((editor, mapping))
}

async fn run(mut command: Command, token: &CancellationToken) -> AppResult<()> {
    let mut child = command
        .kill_on_drop(true)
        .env_remove("FFREPORT")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("adapter stderr unavailable")))?;
    let work = async {
        let mut bytes = Vec::new();
        let read = async {
            stderr.take(64 * 1024 + 1).read_to_end(&mut bytes).await?;
            if bytes.len() > 64 * 1024 {
                return Err(std::io::Error::other("adapter output exceeded limit"));
            }
            Ok::<(), std::io::Error>(())
        };
        let (_, status) = tokio::try_join!(read, child.wait())?;
        if !status.success() {
            return Err(AppError::Invalid(format!(
                "lossless editor adapter unavailable: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }
        Ok::<(), AppError>(())
    };
    let result = tokio::select! {
        _ = token.cancelled() => Err(AppError::Conflict("import cancelled".into())),
        result = tokio::time::timeout(Duration::from_secs(1800), work) => match result { Ok(result) => result, Err(_) => Err(AppError::Invalid("adapter timed out".into())) },
    };
    if result.is_err() {
        child.kill().await?;
    }
    result
}

#[derive(Deserialize)]
struct Probe {
    streams: Vec<Stream>,
    format: Format,
}
#[derive(Deserialize)]
struct Stream {
    codec_type: String,
    start_pts: Option<i64>,
}
#[derive(Deserialize)]
struct Format {
    start_time: Option<String>,
}

async fn verify_origin(path: &Path, count: usize) -> AppResult<()> {
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
    if !result.status.success() {
        return Err(AppError::Invalid("adapter cannot be probed".into()));
    }
    let probe: Probe = serde_json::from_slice(&result.stdout)?;
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
