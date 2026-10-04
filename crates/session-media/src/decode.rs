use std::{io::ErrorKind, path::Path, process::Stdio, time::Duration};

use thiserror::Error;
use tokio::{io::AsyncReadExt, process::Command, time::timeout};

use crate::Options;

#[derive(Debug, Error)]
pub(crate) enum DecodeError {
    #[error("FFmpeg decode was cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("FFmpeg decode exceeded its timeout")]
    Timeout,
    #[error("FFmpeg diagnostic output exceeded its limit")]
    OutputLimit,
    #[error("FFmpeg rejected or could not decode the input: {0}")]
    Failed(String),
}

impl DecodeError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Cancelled => "ASSET_CANCELLED",
            Self::Io(error) if error.kind() == ErrorKind::NotFound => "DECODE_UNAVAILABLE",
            Self::Io(_) => "DECODE_IO_ERROR",
            Self::Timeout => "DECODE_TIMEOUT",
            Self::OutputLimit => "DECODE_OUTPUT_LIMIT",
            Self::Failed(_) => "DECODE_FAILED",
        }
    }
}

pub(crate) async fn inspect(
    path: &Path,
    duration: Duration,
    options: &Options,
) -> Result<(), DecodeError> {
    let mut command = Command::new(&options.ffmpeg);
    command
        .args([
            "-v",
            "error",
            "-xerror",
            "-nostdin",
            "-protocol_whitelist",
            "file",
            "-format_whitelist",
            "mov,matroska,webm,avi,wav,flac,mp3,ogg,aac,mpegts",
            "-probesize",
            "10000000",
            "-analyzeduration",
            "10000000",
            "-threads",
            "1",
            "-max_alloc",
            "67108864",
            "-i",
        ])
        .arg(path)
        .args(["-map", "0:v?", "-map", "0:a?", "-f", "null", "-"])
        .env_remove("FFREPORT")
        .env("AV_LOG_FORCE_NOCOLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn()?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| DecodeError::Io(std::io::Error::other("FFmpeg stderr pipe missing")))?;
    let work = async {
        let mut bytes = Vec::new();
        let mut bounded_stderr = stderr.take(64 * 1024 + 1);
        let read = async {
            bounded_stderr.read_to_end(&mut bytes).await?;
            if bytes.len() > 64 * 1024 {
                return Err(DecodeError::OutputLimit);
            }
            Ok::<(), DecodeError>(())
        };
        let (_, status) =
            tokio::try_join!(read, async { child.wait().await.map_err(DecodeError::Io) })?;
        if status.success() {
            Ok(())
        } else {
            Err(DecodeError::Failed(
                String::from_utf8_lossy(&bytes).chars().take(2048).collect(),
            ))
        }
    };
    let result = tokio::select! {
        biased;
        _ = options.cancellation.cancelled() => Err(DecodeError::Cancelled),
        result = timeout(duration, work) => match result {
            Ok(result) => result,
            Err(_) => Err(DecodeError::Timeout),
        },
    };
    if result.is_err() {
        match child.kill().await {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::InvalidInput => {}
            Err(error) => return Err(DecodeError::Io(error)),
        }
    }
    result
}
