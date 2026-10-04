use std::{process::Stdio, time::Duration};

use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{Child, ChildStdout, Command},
    sync::{Semaphore, SemaphorePermit},
    task::JoinHandle,
    time::sleep,
};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

const STDERR_LIMIT: usize = 64 * 1024;
const MAX_FFMPEG_PROCESSES: usize = 2;
static FFMPEG_SLOTS: Semaphore = Semaphore::const_new(MAX_FFMPEG_PROCESSES);

pub(super) struct RunningChild {
    child: Child,
    stderr: JoinHandle<std::io::Result<Vec<u8>>>,
    _permit: SemaphorePermit<'static>,
}

impl RunningChild {
    pub(super) fn take_stdout(&mut self) -> AppResult<ChildStdout> {
        self.child
            .stdout
            .take()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("ffmpeg stdout was not configured")))
    }

    pub(super) async fn wait(
        mut self,
        label: &str,
        limit: Duration,
        cancel: &CancellationToken,
    ) -> AppResult<()> {
        let outcome = tokio::select! {
            biased;
            () = cancel.cancelled() => {
                terminate_and_reap(&mut self.child).await?;
                Err(AppError::Internal(anyhow::anyhow!("{label} cancelled")))
            }
            () = sleep(limit) => {
                terminate_and_reap(&mut self.child).await?;
                Err(AppError::Internal(anyhow::anyhow!("{label} timed out")))
            }
            status = self.child.wait() => status.map_err(AppError::from),
        };
        let stderr = self.stderr.await.map_err(|error| {
            AppError::Internal(anyhow::anyhow!("stderr reader failed: {error}"))
        })??;
        let status = outcome?;
        if !status.success() {
            return Err(AppError::Internal(anyhow::anyhow!(
                "{label} failed: {}",
                String::from_utf8_lossy(&stderr)
            )));
        }
        Ok(())
    }
}

pub(super) async fn spawn(
    command: &mut Command,
    cancel: &CancellationToken,
) -> AppResult<RunningChild> {
    let permit = tokio::select! {
        biased;
        () = cancel.cancelled() => return Err(AppError::Conflict("media job cancelled".into())),
        permit = FFMPEG_SLOTS.acquire() => permit.map_err(|error| AppError::Internal(anyhow::anyhow!("ffmpeg limiter closed: {error}")))?,
    };
    let mut child = command
        .kill_on_drop(true)
        .env_remove("FFREPORT")
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("ffmpeg stderr was not configured")))?;
    Ok(RunningChild {
        child,
        stderr: tokio::spawn(read_bounded(stderr)),
        _permit: permit,
    })
}

pub(super) async fn run(
    command: &mut Command,
    label: &str,
    limit: Duration,
    cancel: &CancellationToken,
) -> AppResult<()> {
    spawn(command, cancel)
        .await?
        .wait(label, limit, cancel)
        .await
}

async fn terminate_and_reap(child: &mut Child) -> AppResult<()> {
    match child.start_kill() {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::InvalidInput => {}
        Err(error) => return Err(error.into()),
    }
    child.wait().await?;
    Ok(())
}

async fn read_bounded(mut reader: impl AsyncRead + Unpin) -> std::io::Result<Vec<u8>> {
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
