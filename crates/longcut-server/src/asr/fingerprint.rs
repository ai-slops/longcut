use std::{path::Path, time::Duration};

use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, BufReader};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};

use super::{AsrConfig, AsrLanguage, ResumeSnapshot, SourceSnapshot, conversion::OVERLAP_SECONDS};

const HASH_BUFFER_BYTES: usize = 64 * 1024;
const HASH_TIMEOUT: Duration = Duration::from_secs(1800);
const CONVERSION_POLICY_VERSION: u32 = 1;
const ARGUMENT_POLICY_VERSION: u32 = 1;

pub(super) async fn capture(
    config: &AsrConfig,
    source_path: &Path,
    source: &SourceSnapshot,
    language: AsrLanguage,
    cancellation: &CancellationToken,
) -> AppResult<ResumeSnapshot> {
    let (cli, model) = config
        .runtime()
        .ok_or_else(|| AppError::Conflict("ASR runtime is not configured".into()))?;
    let source_sha256 = hash_file(source_path, cancellation).await?;
    let model_sha256 = hash_file(model, cancellation).await?;
    let cli_sha256 = hash_file(cli, cancellation).await?;
    Ok(ResumeSnapshot {
        source_sha256,
        model_sha256,
        cli_sha256,
        source_media_url: source.media_url.clone(),
        model_path: model.to_string_lossy().into_owned(),
        cli_path: cli.to_string_lossy().into_owned(),
        duration_frames: source.duration_frames,
        frame_rate: source.frame_rate,
        language,
        chunk_seconds: config.chunk_seconds,
        overlap_seconds: u32::try_from(OVERLAP_SECONDS)
            .map_err(|_| AppError::Invalid("ASR overlap metadata overflow".into()))?,
        conversion_policy_version: CONVERSION_POLICY_VERSION,
        argument_policy_version: ARGUMENT_POLICY_VERSION,
    })
}

async fn hash_file(path: &Path, cancellation: &CancellationToken) -> AppResult<String> {
    let operation = async {
        let file = tokio::fs::File::open(path).await?;
        let mut reader = BufReader::with_capacity(HASH_BUFFER_BYTES, file);
        let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
        let mut digest = Sha256::new();
        loop {
            let read = tokio::select! {
                biased;
                () = cancellation.cancelled() => {
                    return Err(AppError::Conflict("ASR fingerprinting cancelled".into()));
                }
                read = reader.read(&mut buffer) => read?,
            };
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
        Ok(format!("{:x}", digest.finalize()))
    };
    tokio::time::timeout(HASH_TIMEOUT, operation)
        .await
        .map_err(|_| AppError::Conflict("ASR fingerprinting timed out".into()))?
}
