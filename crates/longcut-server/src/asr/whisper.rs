use std::{path::Path, time::Duration};

use serde::Deserialize;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

use super::{AsrConfig, AsrLanguage, process};

const WHISPER_TIMEOUT: Duration = Duration::from_secs(1800);
const MAX_JSON_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub(super) struct WhisperOutput {
    pub transcription: Vec<WhisperSegment>,
    pub result: WhisperResult,
}

#[derive(Debug, Deserialize)]
pub(super) struct WhisperSegment {
    pub offsets: WhisperOffsets,
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct WhisperOffsets {
    pub from: u64,
    pub to: u64,
}

#[derive(Debug, Deserialize)]
pub(super) struct WhisperResult {
    pub language: Option<String>,
}

pub(super) struct WhisperRequest<'a> {
    pub wav: &'a Path,
    pub prefix: &'a Path,
    pub language: AsrLanguage,
}

pub(super) async fn transcribe(
    config: &AsrConfig,
    request: WhisperRequest<'_>,
    cancellation: &CancellationToken,
) -> AppResult<WhisperOutput> {
    let (cli, model) = config
        .runtime()
        .ok_or_else(|| AppError::Conflict("ASR runtime is not configured".into()))?;
    let mut command = Command::new(cli);
    command
        .arg("-m")
        .arg(model)
        .arg("-f")
        .arg(request.wav)
        .arg("-l")
        .arg(request.language.cli_value())
        .args(["-t", "4", "-ng", "-ml", "1", "-sow", "-oj", "-of"])
        .arg(request.prefix)
        .arg("-np");
    process::run(
        &mut command,
        process::ProcessSpec {
            label: "whisper.cpp",
            limit: WHISPER_TIMEOUT,
            cancellation,
        },
    )
    .await?;
    let json_path = request.prefix.with_extension("json");
    let metadata = tokio::fs::metadata(&json_path).await?;
    if metadata.len() > MAX_JSON_BYTES {
        return Err(AppError::Invalid(
            "whisper JSON output exceeds limit".into(),
        ));
    }
    let bytes = tokio::fs::read(&json_path).await?;
    serde_json::from_slice(&bytes)
        .map_err(|error| AppError::Invalid(format!("invalid whisper JSON output: {error}")))
}
