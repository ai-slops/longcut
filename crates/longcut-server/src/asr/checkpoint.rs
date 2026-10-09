use crate::error::{AppError, AppResult};
use sha2::{Digest, Sha256};

use super::{
    conversion::{ChunkLayout, CueAccumulator, merge_language},
    whisper::WhisperOutput,
};

pub(crate) const MAX_CHECKPOINT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_JOB_CHECKPOINT_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(crate) struct Checkpoint {
    pub index: u32,
    pub inference_start: u64,
    pub inference_end: u64,
    pub owner_start: u64,
    pub owner_end: u64,
    pub payload: String,
}

pub(super) struct Replay {
    pub accumulator: CueAccumulator,
    pub result_language: Option<String>,
    pub next_index: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckpointStamp {
    pub count: u32,
    pub total_bytes: usize,
    pub fingerprint: String,
}

pub(super) struct ReplayBuilder {
    accumulator: CueAccumulator,
    result_language: Option<String>,
    stamp: CheckpointStamp,
}

impl Checkpoint {
    pub(super) fn from_output(
        index: u32,
        bounds: (u64, u64, u64, u64),
        output: &WhisperOutput,
    ) -> AppResult<Self> {
        let payload = serde_json::to_string(output)?;
        if payload.len() > MAX_CHECKPOINT_BYTES {
            return Err(AppError::Invalid(
                "ASR checkpoint exceeds 16777216 bytes".into(),
            ));
        }
        Ok(Self {
            index,
            inference_start: bounds.0,
            inference_end: bounds.1,
            owner_start: bounds.2,
            owner_end: bounds.3,
            payload,
        })
    }
}

impl ReplayBuilder {
    pub(super) const fn new() -> Self {
        Self {
            accumulator: CueAccumulator::new(),
            result_language: None,
            stamp: CheckpointStamp {
                count: 0,
                total_bytes: 0,
                fingerprint: String::new(),
            },
        }
    }

    pub(super) fn append(&mut self, checkpoint: Checkpoint, layout: ChunkLayout) -> AppResult<()> {
        if checkpoint.index != self.stamp.count {
            return Err(AppError::Conflict(
                "ASR checkpoints are not contiguous".into(),
            ));
        }
        let window = layout.window(checkpoint.index)?;
        if window.bounds()
            != (
                checkpoint.inference_start,
                checkpoint.inference_end,
                checkpoint.owner_start,
                checkpoint.owner_end,
            )
        {
            return Err(AppError::Conflict("ASR checkpoint window changed".into()));
        }
        if checkpoint.payload.len() > MAX_CHECKPOINT_BYTES {
            return Err(AppError::Conflict(
                "ASR checkpoint exceeds byte limit".into(),
            ));
        }
        self.stamp.total_bytes = self
            .stamp
            .total_bytes
            .checked_add(checkpoint.payload.len())
            .ok_or_else(|| AppError::Conflict("ASR checkpoint byte count overflow".into()))?;
        if self.stamp.total_bytes > MAX_JOB_CHECKPOINT_BYTES {
            return Err(AppError::Conflict(
                "ASR checkpoints exceed job byte limit".into(),
            ));
        }
        let output: WhisperOutput = serde_json::from_str(&checkpoint.payload)
            .map_err(|_| AppError::Conflict("ASR checkpoint payload is invalid".into()))?;
        self.result_language =
            merge_language(self.result_language.take(), output.result.language.clone());
        self.accumulator.append(output, window)?;
        self.stamp.fingerprint = extend_fingerprint(&self.stamp.fingerprint, &checkpoint);
        self.stamp.count = self
            .stamp
            .count
            .checked_add(1)
            .ok_or_else(|| AppError::Conflict("ASR checkpoint count overflow".into()))?;
        Ok(())
    }

    pub(super) fn finish(self) -> (Replay, CheckpointStamp) {
        (
            Replay {
                accumulator: self.accumulator,
                result_language: self.result_language,
                next_index: self.stamp.count,
            },
            self.stamp,
        )
    }
}

pub(crate) fn extend_fingerprint(previous: &str, checkpoint: &Checkpoint) -> String {
    let mut digest = Sha256::new();
    digest.update(previous.len().to_be_bytes());
    digest.update(previous.as_bytes());
    digest.update(checkpoint.index.to_be_bytes());
    for bound in [
        checkpoint.inference_start,
        checkpoint.inference_end,
        checkpoint.owner_start,
        checkpoint.owner_end,
    ] {
        digest.update(bound.to_be_bytes());
    }
    digest.update(checkpoint.payload.len().to_be_bytes());
    digest.update(checkpoint.payload.as_bytes());
    format!("{:x}", digest.finalize())
}

#[cfg(test)]
pub(super) fn replay(checkpoints: Vec<Checkpoint>, layout: ChunkLayout) -> AppResult<Replay> {
    let mut builder = ReplayBuilder::new();
    for checkpoint in checkpoints {
        builder.append(checkpoint, layout)?;
    }
    Ok(builder.finish().0)
}

#[cfg(test)]
#[path = "checkpoint_tests.rs"]
mod tests;
