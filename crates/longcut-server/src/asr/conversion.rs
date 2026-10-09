use crate::{
    error::{AppError, AppResult},
    transcript::Cue,
};

use super::whisper::WhisperOutput;

pub(super) const MAX_CUES: usize = 20_000;
pub(super) const MAX_CUE_TEXT_BYTES: usize = 16 * 1024;
pub(super) const MAX_JOB_TEXT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy)]
pub(super) struct CueWindow {
    pub start: u64,
    pub end: u64,
    pub frame_rate: u32,
}

pub(super) struct CueAccumulator {
    cues: Vec<Cue>,
    text_bytes: usize,
}

impl CueAccumulator {
    pub(super) const fn new() -> Self {
        Self {
            cues: Vec::new(),
            text_bytes: 0,
        }
    }

    pub(super) fn append(&mut self, output: WhisperOutput, window: CueWindow) -> AppResult<()> {
        for segment in output.transcription {
            let text = segment.text.trim();
            if text.is_empty() {
                continue;
            }
            if segment.offsets.to <= segment.offsets.from {
                return Err(AppError::Invalid(
                    "whisper returned a non-positive cue range".into(),
                ));
            }
            if text.len() > MAX_CUE_TEXT_BYTES {
                return Err(AppError::Invalid(
                    "whisper cue text exceeds 16384 UTF-8 bytes".into(),
                ));
            }
            if self.cues.len() >= MAX_CUES {
                return Err(AppError::Invalid("ASR output exceeds 20000 cues".into()));
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
            let local_start = millis_to_start_frame(segment.offsets.from, window.frame_rate)?;
            let local_end = millis_to_end_frame(segment.offsets.to, window.frame_rate)?;
            let start = window.start.saturating_add(local_start).min(window.end);
            let end = window.start.saturating_add(local_end).min(window.end);
            if end <= start {
                return Err(AppError::Invalid(
                    "whisper returned a cue outside the source chunk".into(),
                ));
            }
            self.cues.push(Cue {
                id: uuid::Uuid::new_v4().to_string(),
                start_frame: start.to_string(),
                end_frame: end.to_string(),
                text: text.into(),
            });
            self.text_bytes = next_text_bytes;
        }
        Ok(())
    }

    pub(super) fn into_cues(self) -> Vec<Cue> {
        self.cues
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
