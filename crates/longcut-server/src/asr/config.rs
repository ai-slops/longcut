use std::{env, path::PathBuf};

use crate::media;

use super::{AsrLanguage, AsrRuntimeStatus};

const DEFAULT_CHUNK_SECONDS: u32 = 600;
const DEFAULT_CONCURRENCY: usize = 1;

#[derive(Clone, Debug)]
pub struct AsrConfig {
    pub(crate) cli: Option<PathBuf>,
    pub(crate) model: Option<PathBuf>,
    pub(crate) ffmpeg: PathBuf,
    pub(crate) chunk_seconds: u32,
    pub(crate) max_concurrency: usize,
}

impl AsrConfig {
    pub fn from_env() -> Self {
        Self {
            cli: env::var_os("LONGCUT_WHISPER_CLI").map(PathBuf::from),
            model: env::var_os("LONGCUT_WHISPER_MODEL").map(PathBuf::from),
            ffmpeg: PathBuf::from("ffmpeg"),
            chunk_seconds: bounded_u32(
                "LONGCUT_ASR_CHUNK_SECONDS",
                DEFAULT_CHUNK_SECONDS,
                30,
                3600,
            ),
            max_concurrency: bounded_usize(
                "LONGCUT_ASR_MAX_CONCURRENCY",
                DEFAULT_CONCURRENCY,
                1,
                4,
            ),
        }
    }

    pub fn from_paths(cli: PathBuf, model: PathBuf) -> Self {
        Self {
            cli: Some(cli),
            model: Some(model),
            ffmpeg: PathBuf::from("ffmpeg"),
            chunk_seconds: DEFAULT_CHUNK_SECONDS,
            max_concurrency: DEFAULT_CONCURRENCY,
        }
    }

    #[doc(hidden)]
    pub fn with_ffmpeg(mut self, ffmpeg: PathBuf) -> Self {
        self.ffmpeg = ffmpeg;
        self
    }

    pub async fn status(&self) -> AsrRuntimeStatus {
        let cli_available = executable_file(self.cli.as_ref()).await;
        let model_available = regular_file(self.model.as_ref()).await;
        let ffmpeg_available = media::tool_available_path(&self.ffmpeg).await;
        let mut missing = Vec::new();
        if self.cli.is_none() {
            missing.push("LONGCUT_WHISPER_CLI");
        }
        if self.model.is_none() {
            missing.push("LONGCUT_WHISPER_MODEL");
        }
        AsrRuntimeStatus {
            configured: missing.is_empty(),
            available: missing.is_empty() && cli_available && model_available && ffmpeg_available,
            cli_available,
            model_available,
            ffmpeg_available,
            missing,
            engine: "whisper.cpp",
            model: self.model.as_ref().map(|path| path.display().to_string()),
            default_language: AsrLanguage::Ko,
            supported_languages: ["ko", "en", "auto"],
            chunk_seconds: self.chunk_seconds,
            max_concurrency: self.max_concurrency,
        }
    }

    pub(crate) fn runtime(&self) -> Option<(&std::path::Path, &std::path::Path)> {
        self.cli.as_deref().zip(self.model.as_deref())
    }
}

async fn regular_file(path: Option<&PathBuf>) -> bool {
    match path {
        Some(path) => tokio::fs::metadata(path)
            .await
            .map(|metadata| metadata.is_file())
            .unwrap_or(false),
        None => false,
    }
}

async fn executable_file(path: Option<&PathBuf>) -> bool {
    let Some(path) = path else {
        return false;
    };
    let Ok(metadata) = tokio::fs::metadata(path).await else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn bounded_u32(name: &str, fallback: u32, minimum: u32, maximum: u32) -> u32 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| (minimum..=maximum).contains(value))
        .unwrap_or(fallback)
}

fn bounded_usize(name: &str, fallback: usize, minimum: usize, maximum: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| (minimum..=maximum).contains(value))
        .unwrap_or(fallback)
}
