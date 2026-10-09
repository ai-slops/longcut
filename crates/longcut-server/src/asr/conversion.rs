use crate::{
    error::{AppError, AppResult},
    transcript::{Cue, Word},
};

use super::whisper::WhisperOutput;

#[path = "conversion_index.rs"]
mod index;
use index::PreviousChunkIndex;

pub(super) const OVERLAP_SECONDS: u64 = 2;
pub(super) const MAX_CUES: usize = 20_000;
pub(super) const MAX_WORDS: usize = 100_000;
pub(super) const MAX_PROCESSED_WORD_CANDIDATES: usize = 300_000;
pub(super) const MAX_WORDS_PER_CUE: usize = 12;
pub(super) const MAX_CUE_TEXT_BYTES: usize = 16 * 1024;
pub(super) const MAX_JOB_TEXT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) struct ChunkLayout {
    pub chunk_frames: u64,
    pub duration_frames: u64,
    pub frame_rate: u32,
}

impl ChunkLayout {
    pub(super) fn window(self, index: u32) -> AppResult<CueWindow> {
        let owner_start = u64::from(index)
            .checked_mul(self.chunk_frames)
            .ok_or_else(|| AppError::Invalid("ASR chunk position overflow".into()))?;
        let owner_end = owner_start
            .saturating_add(self.chunk_frames)
            .min(self.duration_frames);
        let overlap = OVERLAP_SECONDS
            .checked_mul(u64::from(self.frame_rate))
            .ok_or_else(|| AppError::Invalid("ASR overlap size overflow".into()))?;
        Ok(CueWindow {
            inference_start: owner_start.saturating_sub(overlap),
            inference_end: owner_end.saturating_add(overlap).min(self.duration_frames),
            owner_start,
            owner_end,
            frame_rate: self.frame_rate,
        })
    }
}

#[derive(Clone, Copy)]
pub(super) struct CueWindow {
    inference_start: u64,
    inference_end: u64,
    owner_start: u64,
    owner_end: u64,
    frame_rate: u32,
}

impl CueWindow {
    pub(super) const fn bounds(self) -> (u64, u64, u64, u64) {
        (
            self.inference_start,
            self.inference_end,
            self.owner_start,
            self.owner_end,
        )
    }
}

struct TimedWord {
    wire: Word,
    start: u64,
    end: u64,
    normalized: String,
}

pub(super) struct CueAccumulator {
    words: Vec<TimedWord>,
    text_bytes: usize,
    previous_chunk_start: usize,
    processed_candidates: usize,
}

impl CueAccumulator {
    pub(super) const fn new() -> Self {
        Self {
            words: Vec::new(),
            text_bytes: 0,
            previous_chunk_start: 0,
            processed_candidates: 0,
        }
    }

    pub(super) fn append(&mut self, output: WhisperOutput, window: CueWindow) -> AppResult<()> {
        let previous_chunk = self.previous_chunk_start..self.words.len();
        let previous_index = PreviousChunkIndex::new(
            self.words[previous_chunk.clone()]
                .iter()
                .map(|word| (word.normalized.as_str(), word.start, word.end)),
        );
        for segment in output.transcription {
            let text = segment.text.trim();
            if text.is_empty() {
                continue;
            }
            self.processed_candidates = self
                .processed_candidates
                .checked_add(1)
                .ok_or_else(|| AppError::Invalid("ASR word candidate count overflow".into()))?;
            if self.processed_candidates > MAX_PROCESSED_WORD_CANDIDATES {
                return Err(AppError::Invalid(
                    "ASR output exceeds 300000 word candidates".into(),
                ));
            }
            if segment.offsets.to < segment.offsets.from {
                return Err(AppError::Invalid(
                    "whisper returned a reversed word range".into(),
                ));
            }
            if text.len() > MAX_CUE_TEXT_BYTES {
                return Err(AppError::Invalid(
                    "whisper word text exceeds 16384 UTF-8 bytes".into(),
                ));
            }
            let local_start = millis_to_start_frame(segment.offsets.from, window.frame_rate)?;
            let local_end = if segment.offsets.to == segment.offsets.from {
                local_start
            } else {
                millis_to_end_frame(segment.offsets.to, window.frame_rate)?
            };
            let start = window
                .inference_start
                .checked_add(local_start)
                .ok_or_else(|| AppError::Invalid("whisper word offset overflow".into()))?;
            let end = window
                .inference_start
                .checked_add(local_end)
                .ok_or_else(|| AppError::Invalid("whisper word offset overflow".into()))?;
            if !owned_by(start, end, window)? {
                continue;
            }
            let inference_frames = window.inference_end - window.inference_start;
            if local_start > inference_frames || local_end > inference_frames {
                return Err(AppError::Invalid(
                    "whisper returned an owned word outside the inference window".into(),
                ));
            }
            let normalized = normalize(text);
            if previous_index.overlaps(&normalized, start, end) {
                continue;
            }
            if self.words.len() >= MAX_WORDS {
                return Err(AppError::Invalid("ASR output exceeds 100000 words".into()));
            }
            let next_text_bytes = self
                .text_bytes
                .checked_add(text.len())
                .ok_or_else(|| AppError::Invalid("ASR text byte count overflow".into()))?;
            if next_text_bytes > MAX_JOB_TEXT_BYTES {
                return Err(AppError::Invalid(
                    "ASR output exceeds 33554432 UTF-8 text bytes".into(),
                ));
            }
            self.words.push(TimedWord {
                wire: Word {
                    id: uuid::Uuid::new_v4().to_string(),
                    start_frame: start.to_string(),
                    end_frame: end.to_string(),
                    text: text.into(),
                },
                start,
                end,
                normalized,
            });
            self.text_bytes = next_text_bytes;
        }
        self.previous_chunk_start = previous_chunk.end;
        Ok(())
    }

    pub(super) fn into_cues(mut self) -> AppResult<Vec<Cue>> {
        self.words.sort_by_key(|word| (word.start, word.end));
        let mut cues = Vec::with_capacity(self.words.len().div_ceil(MAX_WORDS_PER_CUE));
        let mut cue_text_bytes = 0_usize;
        let mut start = 0_usize;
        while start < self.words.len() {
            let mut end = start;
            let mut text_bytes = 0_usize;
            while end < self.words.len() && end - start < MAX_WORDS_PER_CUE {
                let separator_bytes =
                    usize::from(end > start && needs_space(&self.words[end].wire.text));
                let next_bytes = text_bytes
                    .checked_add(separator_bytes)
                    .and_then(|value| value.checked_add(self.words[end].wire.text.len()))
                    .ok_or_else(|| AppError::Invalid("ASR cue text byte count overflow".into()))?;
                if end > start && next_bytes > MAX_CUE_TEXT_BYTES {
                    break;
                }
                text_bytes = next_bytes;
                end += 1;
            }
            let words = &self.words[start..end];
            let cue = aggregate(words).ok_or_else(|| {
                AppError::Invalid("zero-duration ASR words cannot form a cue".into())
            })?;
            cue_text_bytes = cue_text_bytes
                .checked_add(cue.text.len())
                .ok_or_else(|| AppError::Invalid("ASR cue text byte count overflow".into()))?;
            if cue_text_bytes > MAX_JOB_TEXT_BYTES {
                return Err(AppError::Invalid(
                    "ASR output exceeds 33554432 UTF-8 cue text bytes".into(),
                ));
            }
            if cues.len() >= MAX_CUES {
                return Err(AppError::Invalid("ASR output exceeds 20000 cues".into()));
            }
            cues.push(cue);
            start = end;
        }
        Ok(cues)
    }
}

pub(super) fn merge_language(current: Option<String>, next: Option<String>) -> Option<String> {
    match (current, next) {
        (None, next) => next,
        (current, None) => current,
        (Some(current), Some(next)) if current == next => Some(current),
        (Some(_), Some(_)) => Some("mixed".into()),
    }
}

fn owned_by(start: u64, end: u64, window: CueWindow) -> AppResult<bool> {
    let midpoint_twice = start
        .checked_add(end)
        .ok_or_else(|| AppError::Invalid("whisper word midpoint overflow".into()))?;
    let owner_start_twice = window
        .owner_start
        .checked_mul(2)
        .ok_or_else(|| AppError::Invalid("ASR ownership range overflow".into()))?;
    let owner_end_twice = window
        .owner_end
        .checked_mul(2)
        .ok_or_else(|| AppError::Invalid("ASR ownership range overflow".into()))?;
    Ok(owner_start_twice <= midpoint_twice && midpoint_twice < owner_end_twice)
}

fn aggregate(words: &[TimedWord]) -> Option<Cue> {
    let start = words.iter().map(|word| word.start).min()?;
    let end = words.iter().map(|word| word.end).max()?;
    if end <= start {
        return None;
    }
    let mut text = String::new();
    for word in words {
        if !text.is_empty() && needs_space(&word.wire.text) {
            text.push(' ');
        }
        text.push_str(&word.wire.text);
    }
    Some(Cue {
        id: uuid::Uuid::new_v4().to_string(),
        start_frame: start.to_string(),
        end_frame: end.to_string(),
        text,
        words: Some(words.iter().map(|word| word.wire.clone()).collect()),
    })
}

fn needs_space(text: &str) -> bool {
    !text.chars().next().is_some_and(|character| {
        matches!(
            character,
            '.' | ',' | '!' | '?' | ':' | ';' | ')' | ']' | '}'
        )
    })
}

fn normalize(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn millis_to_start_frame(milliseconds: u64, frame_rate: u32) -> AppResult<u64> {
    milliseconds
        .checked_mul(u64::from(frame_rate))
        .map(|value| value / 1000)
        .ok_or_else(|| AppError::Invalid("whisper offset overflow".into()))
}

fn millis_to_end_frame(milliseconds: u64, frame_rate: u32) -> AppResult<u64> {
    milliseconds
        .checked_mul(u64::from(frame_rate))
        .and_then(|value| value.checked_add(999))
        .map(|value| value / 1000)
        .ok_or_else(|| AppError::Invalid("whisper offset overflow".into()))
}

#[cfg(test)]
#[path = "conversion_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "conversion_boundary_tests.rs"]
mod boundary_tests;
