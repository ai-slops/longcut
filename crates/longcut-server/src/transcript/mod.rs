mod parse;
mod store;

use crate::{AppState, error::AppResult};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};

use crate::asr::AsrTranscriptMetadata;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cue {
    pub id: String,
    pub start_frame: String,
    pub end_frame: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranscriptPage {
    pub id: String,
    pub source_id: String,
    pub filename: String,
    pub created_at: String,
    pub frame_rate: u32,
    pub cue_count: usize,
    pub origin: TranscriptOrigin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asr: Option<AsrTranscriptMetadata>,
    pub cues: Vec<Cue>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TranscriptOrigin {
    SubtitleImport,
    LocalAsr,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportTranscript {
    pub filename: String,
    pub text: String,
}

#[derive(Deserialize, Default)]
pub struct Search {
    pub query: Option<String>,
    pub cursor: Option<String>,
}

pub async fn import(
    State(state): State<AppState>,
    Path((project, source)): Path<(String, String)>,
    Json(input): Json<ImportTranscript>,
) -> AppResult<Json<TranscriptPage>> {
    state
        .store
        .import_transcript(&project, &source, input)
        .await
        .map(Json)
}

pub async fn get(
    State(state): State<AppState>,
    Path((project, source)): Path<(String, String)>,
    Query(search): Query<Search>,
) -> AppResult<Json<Option<TranscriptPage>>> {
    state
        .store
        .transcript(&project, &source, search)
        .await
        .map(Json)
}
