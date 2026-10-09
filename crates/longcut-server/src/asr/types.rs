use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrLanguage {
    #[default]
    Ko,
    En,
    Auto,
}

impl AsrLanguage {
    pub const fn cli_value(self) -> &'static str {
        match self {
            Self::Ko => "ko",
            Self::En => "en",
            Self::Auto => "auto",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartAsr {
    #[serde(default)]
    pub language: AsrLanguage,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrJobStatus {
    Queued,
    Running,
    Ready,
    Failed,
    Cancelled,
}

impl AsrJobStatus {
    pub const fn active(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }

    pub const fn db_value(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AsrProgress {
    pub completed_chunks: u32,
    pub total_chunks: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AsrJob {
    pub id: String,
    pub project_id: String,
    pub source_id: String,
    pub status: AsrJobStatus,
    pub progress: AsrProgress,
    pub language: AsrLanguage,
    pub engine: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
    pub transcript_id: Option<String>,
    pub error: Option<String>,
    #[serde(skip)]
    pub(crate) base_transcript_id: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AsrRuntimeStatus {
    pub configured: bool,
    pub available: bool,
    pub cli_available: bool,
    pub model_available: bool,
    pub ffmpeg_available: bool,
    pub missing: Vec<&'static str>,
    pub engine: &'static str,
    pub model: Option<String>,
    pub default_language: AsrLanguage,
    pub supported_languages: [&'static str; 3],
    pub chunk_seconds: u32,
    pub max_concurrency: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AsrTranscriptMetadata {
    pub engine: String,
    pub model: String,
    pub requested_language: AsrLanguage,
    pub result_language: Option<String>,
}

#[derive(Debug)]
pub(crate) struct SourceSnapshot {
    pub media_url: String,
    pub duration_frames: u64,
    pub frame_rate: u32,
}
