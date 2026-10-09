use std::{path::Path, time::Duration};

use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::error::AppResult;

use super::{AsrConfig, process};

const FFMPEG_TIMEOUT: Duration = Duration::from_secs(900);

pub(super) struct AudioChunk<'a> {
    pub source: &'a Path,
    pub output: &'a Path,
    pub start_frame: u64,
    pub end_frame: u64,
    pub frame_rate: u32,
}

pub(super) async fn extract_chunk(
    config: &AsrConfig,
    chunk: AudioChunk<'_>,
    cancellation: &CancellationToken,
) -> AppResult<()> {
    let mut command = Command::new(&config.ffmpeg);
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-protocol_whitelist",
            "file,pipe",
            "-format_whitelist",
            "mov,matroska,webm,avi,wav,flac,mp3,ogg,aac,mpegts",
            "-ss",
            &seconds(chunk.start_frame, chunk.frame_rate),
            "-i",
        ])
        .arg(chunk.source)
        .args([
            "-t",
            &seconds(chunk.end_frame - chunk.start_frame, chunk.frame_rate),
            "-map",
            "0:a:0",
            "-vn",
            "-ac",
            "1",
            "-ar",
            "16000",
            "-c:a",
            "pcm_s16le",
            "-threads",
            "1",
            "-y",
        ])
        .arg(chunk.output);
    process::run(
        &mut command,
        process::ProcessSpec {
            label: "ASR audio extraction",
            limit: FFMPEG_TIMEOUT,
            cancellation,
        },
    )
    .await
}

fn seconds(frames: u64, frame_rate: u32) -> String {
    let whole = frames / u64::from(frame_rate);
    let remainder = frames % u64::from(frame_rate);
    let nanos = remainder * 1_000_000_000 / u64::from(frame_rate);
    format!("{whole}.{nanos:09}")
}
