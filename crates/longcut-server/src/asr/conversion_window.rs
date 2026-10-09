use crate::error::{AppError, AppResult};

pub(crate) const OVERLAP_SECONDS: u64 = 2;

#[derive(Clone, Copy)]
pub(crate) struct ChunkLayout {
    pub(in crate::asr) chunk_frames: u64,
    pub(in crate::asr) duration_frames: u64,
    pub(in crate::asr) frame_rate: u32,
}

impl ChunkLayout {
    pub(crate) fn window(self, index: u32) -> AppResult<CueWindow> {
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
pub(crate) struct CueWindow {
    pub(super) inference_start: u64,
    pub(super) inference_end: u64,
    pub(super) owner_start: u64,
    pub(super) owner_end: u64,
    pub(super) frame_rate: u32,
}

impl CueWindow {
    pub(crate) const fn bounds(self) -> (u64, u64, u64, u64) {
        (
            self.inference_start,
            self.inference_end,
            self.owner_start,
            self.owner_end,
        )
    }
}

pub(super) fn owned_by(start: u64, end: u64, window: CueWindow) -> AppResult<bool> {
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

pub(super) fn millis_to_start_frame(milliseconds: u64, frame_rate: u32) -> AppResult<u64> {
    milliseconds
        .checked_mul(u64::from(frame_rate))
        .map(|value| value / 1000)
        .ok_or_else(|| AppError::Invalid("whisper offset overflow".into()))
}

pub(super) fn millis_to_end_frame(milliseconds: u64, frame_rate: u32) -> AppResult<u64> {
    milliseconds
        .checked_mul(u64::from(frame_rate))
        .and_then(|value| value.checked_add(999))
        .map(|value| value / 1000)
        .ok_or_else(|| AppError::Invalid("whisper offset overflow".into()))
}
