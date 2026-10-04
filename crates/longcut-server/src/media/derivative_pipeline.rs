use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::{fs, process::Command};
use tokio_util::sync::CancellationToken;

use crate::{
    error::AppResult,
    model::{MediaStatus, Source},
};

use super::{artifact_name, process, waveform};

const DERIVATIVE_TIMEOUT: Duration = Duration::from_secs(300);

struct ArtifactPaths {
    proxy_name: String,
    proxy_temp: PathBuf,
    proxy_final: PathBuf,
    thumbnail_name: String,
    thumbnail_temp: PathBuf,
    thumbnail_final: PathBuf,
}

impl ArtifactPaths {
    fn new(artifacts: &Path, source_id: &str) -> Self {
        let proxy_name = format!("{source_id}-proxy.mp4");
        let thumbnail_name = format!("{source_id}-thumbnail.jpg");
        Self {
            proxy_temp: artifacts.join(format!(".{proxy_name}.tmp.mp4")),
            proxy_final: artifacts.join(&proxy_name),
            thumbnail_temp: artifacts.join(format!(".{thumbnail_name}.tmp.jpg")),
            thumbnail_final: artifacts.join(&thumbnail_name),
            proxy_name,
            thumbnail_name,
        }
    }

    async fn clean_temporary(&self) {
        for path in [&self.proxy_temp, &self.thumbnail_temp] {
            match fs::remove_file(path).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => {}
            }
        }
    }
}

pub(super) async fn generate(
    artifacts: &Path,
    source: &Source,
    cancel: &CancellationToken,
) -> AppResult<Source> {
    let original = artifacts.join(artifact_name(&source.media_url)?);
    let paths = ArtifactPaths::new(artifacts, &source.id);
    paths.clean_temporary().await;
    let has_video = source.width > 0 && source.height > 0;
    let generated = async {
        create_proxy(&original, &paths.proxy_temp, has_video, cancel).await?;
        if has_video {
            create_thumbnail(&original, &paths.thumbnail_temp, cancel).await?;
        }
        let peaks = if source.has_audio {
            waveform::extract(&original, cancel).await?
        } else {
            Vec::new()
        };
        Ok::<Vec<f32>, crate::error::AppError>(peaks)
    }
    .await;
    let peaks = match generated {
        Ok(peaks) => peaks,
        Err(error) => {
            paths.clean_temporary().await;
            return Err(error);
        }
    };
    if let Err(error) = publish(&paths, has_video).await {
        paths.clean_temporary().await;
        return Err(error);
    }
    let mut ready = source.clone();
    ready.proxy_url = Some(format!("/api/v1/media/{}", paths.proxy_name));
    ready.thumbnail_url = has_video.then(|| format!("/api/v1/media/{}", paths.thumbnail_name));
    ready.waveform = peaks;
    ready.status = MediaStatus::Ready;
    Ok(ready)
}

async fn create_proxy(
    original: &Path,
    output: &Path,
    has_video: bool,
    cancel: &CancellationToken,
) -> AppResult<()> {
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-protocol_whitelist",
            "file,pipe",
            "-i",
        ])
        .arg(original);
    if has_video {
        command.args([
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-vf",
            "scale=1280:-2",
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "25",
            "-threads",
            "1",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-movflags",
            "+faststart",
            "-y",
        ]);
    } else {
        command.args([
            "-map",
            "0:a:0",
            "-vn",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
            "-threads",
            "1",
            "-movflags",
            "+faststart",
            "-y",
        ]);
    }
    command.arg(output);
    process::run(&mut command, "proxy", DERIVATIVE_TIMEOUT, cancel).await
}

async fn create_thumbnail(
    original: &Path,
    output: &Path,
    cancel: &CancellationToken,
) -> AppResult<()> {
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-protocol_whitelist",
            "file,pipe",
            "-ss",
            "0",
            "-i",
        ])
        .arg(original)
        .args([
            "-map",
            "0:v:0",
            "-frames:v",
            "1",
            "-vf",
            "scale=640:-2",
            "-threads",
            "1",
            "-y",
        ])
        .arg(output);
    process::run(&mut command, "thumbnail", DERIVATIVE_TIMEOUT, cancel).await
}

async fn publish(paths: &ArtifactPaths, has_video: bool) -> AppResult<()> {
    fs::rename(&paths.proxy_temp, &paths.proxy_final).await?;
    if has_video && let Err(error) = fs::rename(&paths.thumbnail_temp, &paths.thumbnail_final).await
    {
        let _ = fs::remove_file(&paths.proxy_final).await;
        return Err(error.into());
    }
    Ok(())
}
