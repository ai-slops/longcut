use std::{collections::BTreeSet, path::Path, process::Stdio};

use serde::Deserialize;
use thiserror::Error;
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::{ChildStdout, Command},
    time::timeout,
};

use crate::Options;

#[derive(Debug, Deserialize)]
pub(crate) struct Probe {
    pub streams: Vec<Stream>,
    pub program_version: Version,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Version {
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Stream {
    pub index: u32,
    pub codec_type: String,
    pub codec_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub sample_rate: Option<String>,
    pub channels: Option<u32>,
    pub channel_layout: Option<String>,
    pub time_base: Option<String>,
}

#[derive(Debug, Error)]
pub(crate) enum ProbeError {
    #[error("ffprobe inspection was cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("ffprobe exceeded its timeout")]
    Timeout,
    #[error("ffprobe output exceeded its limit")]
    OutputLimit,
    #[error("ffprobe response was invalid: {0}")]
    Invalid(String),
    #[error("ffprobe rejected input: {0}")]
    Failed(String),
}

pub(crate) fn input_command(path: &Path, options: &Options) -> Command {
    let mut command = Command::new(&options.ffprobe);
    command
        .args([
            "-v",
            "error",
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
        .env_remove("FFREPORT")
        .env("AV_LOG_FORCE_NOCOLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    command
}

fn command(path: &Path, options: &Options) -> Command {
    let mut command = input_command(path, options);
    command.args([
        "-show_streams", "-show_program_version", "-show_entries",
        "stream=index,codec_type,codec_name,width,height,sample_rate,channels,channel_layout,time_base:program_version=version",
        "-of", "json",
    ]);
    command
}

pub(crate) async fn run<T, F>(
    mut command: Command,
    consume: impl FnOnce(ChildStdout) -> F,
    options: &Options,
) -> Result<T, ProbeError>
where
    F: std::future::Future<Output = Result<T, ProbeError>>,
{
    let mut child = command.spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| ProbeError::Invalid("stdout pipe missing".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| ProbeError::Invalid("stderr pipe missing".into()))?;
    let result = tokio::select! {
        biased;
        _ = options.cancellation.cancelled() => Err(ProbeError::Cancelled),
        result = timeout(options.probe_timeout, async {
            tokio::try_join!(consume(stdout), limited(stderr, 64 * 1024), async { child.wait().await.map_err(ProbeError::Io) })
        }) => match result { Ok(result) => result, Err(_) => Err(ProbeError::Timeout) },
    };
    let output = match result {
        Ok(output) => output,
        Err(error) => {
            child.kill().await?;
            return Err(error);
        }
    };
    if !output.2.success() || !output.1.is_empty() {
        return Err(ProbeError::Failed(
            String::from_utf8_lossy(&output.1)
                .chars()
                .take(2048)
                .collect(),
        ));
    }
    Ok(output.0)
}

pub(crate) async fn inspect(path: &Path, options: &Options) -> Result<Probe, ProbeError> {
    let output = run(
        command(path, options),
        |stdout| limited(stdout, 1024 * 1024),
        options,
    )
    .await?;
    let probe: Probe =
        serde_json::from_slice(&output).map_err(|error| ProbeError::Invalid(error.to_string()))?;
    let distinct: BTreeSet<_> = probe.streams.iter().map(|stream| stream.index).collect();
    if probe.streams.is_empty() || distinct.len() != probe.streams.len() {
        return Err(ProbeError::Invalid(
            "streams must be nonempty with unique indexes".into(),
        ));
    }
    Ok(probe)
}

async fn limited(reader: impl AsyncRead + Unpin, limit: u64) -> Result<Vec<u8>, ProbeError> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes).await?;
    if u64::try_from(bytes.len()).map_err(|_| ProbeError::OutputLimit)? > limit {
        return Err(ProbeError::OutputLimit);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_uses_local_protocols_explicit_input_and_bounded_fields() {
        let command = command(Path::new("/package/-source.mp4"), &Options::default());
        let args: Vec<_> = command.as_std().get_args().collect();
        assert!(
            args.windows(2)
                .any(|args| args == ["-protocol_whitelist", "file"])
        );
        assert!(
            args.windows(2)
                .any(|args| args == ["-i", "/package/-source.mp4"])
        );
        assert!(args.iter().any(|arg| *arg == "-format_whitelist"));
    }
}
