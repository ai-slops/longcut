use super::*;
use crate::asr::whisper::{WhisperOffsets, WhisperResult, WhisperSegment};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn output(segments: Vec<WhisperSegment>) -> WhisperOutput {
    WhisperOutput {
        transcription: segments,
        result: WhisperResult { language: None },
    }
}

fn segment(from: u64, to: u64, text: String) -> WhisperSegment {
    WhisperSegment {
        offsets: WhisperOffsets { from, to },
        text,
    }
}

const fn window(start: u64, end: u64) -> CueWindow {
    CueWindow {
        start,
        end,
        frame_rate: 30,
    }
}

#[test]
fn append_rejects_cue_over_manual_contract_limit_before_push() {
    // Given
    let mut cues = CueAccumulator::new();
    let oversized = "가".repeat((MAX_CUE_TEXT_BYTES / 3) + 1);
    // When
    let result = cues.append(output(vec![segment(0, 1000, oversized)]), window(0, 30));
    // Then
    assert!(result.is_err());
    assert!(cues.into_cues().is_empty());
}

#[test]
fn append_rejects_aggregate_text_across_chunks_before_retention() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    let full_cue = "x".repeat(MAX_CUE_TEXT_BYTES);
    let first = output(
        (0..MAX_JOB_TEXT_BYTES / MAX_CUE_TEXT_BYTES)
            .map(|_| segment(0, 1000, full_cue.clone()))
            .collect(),
    );
    cues.append(first, window(0, 30))?;
    // When
    let result = cues.append(output(vec![segment(0, 1000, "x".into())]), window(30, 60));
    // Then
    assert!(result.is_err());
    assert_eq!(
        cues.into_cues().len(),
        MAX_JOB_TEXT_BYTES / MAX_CUE_TEXT_BYTES
    );
    Ok(())
}

#[test]
fn append_checks_job_cue_limit_before_each_push() {
    // Given
    let mut cues = CueAccumulator {
        cues: (0..MAX_CUES)
            .map(|index| Cue {
                id: index.to_string(),
                start_frame: "0".into(),
                end_frame: "1".into(),
                text: "x".into(),
            })
            .collect(),
        text_bytes: MAX_CUES,
    };
    // When
    let result = cues.append(
        output(vec![segment(0, 1000, "overflow".into())]),
        window(0, 30),
    );
    // Then
    assert!(result.is_err());
    assert_eq!(cues.into_cues().len(), MAX_CUES);
}

#[test]
fn append_offsets_chunk_coordinates_and_rounds_half_open_frames() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    // When
    cues.append(
        output(vec![segment(1, 1001, "bounded".into())]),
        window(300, 360),
    )?;
    // Then
    let cues = cues.into_cues();
    assert_eq!(cues[0].start_frame, "300");
    assert_eq!(cues[0].end_frame, "331");
    Ok(())
}

#[test]
fn append_rejects_reversed_timestamp_without_retention() {
    // Given
    let mut cues = CueAccumulator::new();
    // When
    let result = cues.append(
        output(vec![segment(1000, 500, "invalid".into())]),
        window(0, 60),
    );
    // Then
    assert!(result.is_err());
    assert!(cues.into_cues().is_empty());
}

#[test]
fn append_accepts_empty_silence_output_without_fabricating_cues() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    // When
    cues.append(output(Vec::new()), window(0, 30))?;
    // Then
    assert!(cues.into_cues().is_empty());
    Ok(())
}
