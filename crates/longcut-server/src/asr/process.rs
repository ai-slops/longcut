use std::{process::Stdio, time::Duration};

use tokio::{io::AsyncReadExt, process::Command};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

const STDERR_LIMIT: usize = 64 * 1024;

pub(super) struct ProcessSpec<'a> {
    pub label: &'a str,
    pub limit: Duration,
    pub cancellation: &'a CancellationToken,
}

pub(super) async fn run(command: &mut Command, spec: ProcessSpec<'_>) -> AppResult<()> {
    let mut child = command
        .kill_on_drop(true)
        .env_remove("FFREPORT")
        .env("AV_LOG_FORCE_NOCOLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("subprocess stderr unavailable")))?;
    let stderr_task = tokio::spawn(read_bounded(stderr));
    let outcome = tokio::select! {
        biased;
        () = spec.cancellation.cancelled() => {
            terminate_and_reap(&mut child).await?;
            Err(AppError::Conflict(format!("{} cancelled", spec.label)))
        }
        () = tokio::time::sleep(spec.limit) => {
            terminate_and_reap(&mut child).await?;
            Err(AppError::Internal(anyhow::anyhow!("{} timed out", spec.label)))
        }
        status = child.wait() => status.map_err(AppError::from),
    };
    let stderr = stderr_task
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!("stderr reader failed: {error}")))??;
    let status = outcome?;
    if !status.success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "{} failed: {}",
            spec.label,
            String::from_utf8_lossy(&stderr)
        )));
    }
    Ok(())
}

async fn terminate_and_reap(child: &mut tokio::process::Child) -> AppResult<()> {
    match child.start_kill() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
        Err(error) => return Err(error.into()),
    }
    child.wait().await?;
    Ok(())
}

async fn read_bounded(mut reader: impl tokio::io::AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
    let mut kept = Vec::with_capacity(STDERR_LIMIT);
    let mut chunk = [0_u8; 8192];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            return Ok(kept);
        }
        let overflow = kept.len().saturating_add(read).saturating_sub(STDERR_LIMIT);
        if overflow > 0 {
            kept.drain(..overflow);
        }
        kept.extend_from_slice(&chunk[..read]);
    }
}
