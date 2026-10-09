use super::*;
use crate::asr::whisper::{WhisperOffsets, WhisperResult, WhisperSegment};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn output(segments: Vec<WhisperSegment>) -> WhisperOutput {
    WhisperOutput {
        transcription: segments,
        result: WhisperResult { language: None },
    }
}

fn segment(from: u64, to: u64, text: &str) -> WhisperSegment {
    WhisperSegment {
        offsets: WhisperOffsets { from, to },
        text: text.into(),
    }
}

const fn layout() -> ChunkLayout {
    ChunkLayout {
        chunk_frames: 30_000,
        duration_frames: 50_000,
        frame_rate: 1_000,
    }
}

fn word_count(cues: &[Cue]) -> usize {
    cues.iter()
        .flat_map(|cue| cue.words.iter().flatten())
        .count()
}

#[test]
fn unowned_model_overrun_is_discarded_before_inference_bounds_validation() -> TestResult {
    // Given: the exact real-model overrun from the first 32-second inference window.
    let mut cues = CueAccumulator::new();
    // When
    cues.append(
        output(vec![segment(31_480, 41_420, "해당입니다.")]),
        layout().window(0)?,
    )?;
    // Then
    assert!(cues.into_cues()?.is_empty());
    Ok(())
}

#[test]
fn owned_model_overrun_still_fails_instead_of_clipping_or_fabricating() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    // When: midpoint 29.5s is owned by chunk zero, but its end exceeds 32s inference.
    let result = cues.append(
        output(vec![segment(26_000, 33_000, "owned overrun")]),
        layout().window(0)?,
    );
    // Then
    assert!(result.is_err());
    assert!(cues.into_cues()?.is_empty());
    Ok(())
}

#[test]
fn overlapping_repeated_words_from_same_inference_are_both_preserved() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    // When
    cues.append(
        output(vec![
            segment(1_000, 2_000, "again"),
            segment(1_500, 2_500, "again"),
        ]),
        layout().window(0)?,
    )?;
    let cues = cues.into_cues()?;
    // Then
    assert_eq!(word_count(&cues), 2);
    Ok(())
}

#[test]
fn overlapping_duplicate_from_neighbor_inference_is_suppressed() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    // When
    cues.append(
        output(vec![segment(29_000, 30_100, " repeat ")]),
        layout().window(0)?,
    )?;
    cues.append(
        output(vec![segment(1_000, 2_100, "repeat")]),
        layout().window(1)?,
    )?;
    let cues = cues.into_cues()?;
    // Then
    assert_eq!(word_count(&cues), 1);
    Ok(())
}

#[test]
fn zero_duration_lexical_word_keeps_equal_frames_without_punctuation_label() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    let source = ChunkLayout {
        chunk_frames: 900,
        duration_frames: 1_500,
        frame_rate: 30,
    };
    // When
    cues.append(
        output(vec![
            segment(13_360, 13_360, "다림이"),
            segment(13_360, 15_840, "15mAh,"),
        ]),
        source.window(0)?,
    )?;
    let cues = cues.into_cues()?;
    // Then
    let words = cues[0].words.as_ref().ok_or("ASR words")?;
    assert_eq!(words[0].text, "다림이");
    assert_eq!(words[0].start_frame, words[0].end_frame);
    Ok(())
}

#[test]
fn zero_duration_point_inside_previous_chunk_interval_is_not_deduplicated() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    // When
    cues.append(
        output(vec![segment(29_000, 30_500, "repeat")]),
        layout().window(0)?,
    )?;
    cues.append(
        output(vec![segment(2_100, 2_100, "repeat")]),
        layout().window(1)?,
    )?;
    let cues = cues.into_cues()?;
    // Then
    assert_eq!(word_count(&cues), 2);
    Ok(())
}

#[test]
fn singleton_zero_duration_lexical_word_fails_instead_of_disappearing() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    cues.append(
        output(vec![segment(13_360, 13_360, "다림이")]),
        layout().window(0)?,
    )?;
    // When
    let result = cues.into_cues();
    // Then
    assert!(result.is_err());
    Ok(())
}

#[test]
fn trailing_zero_duration_group_after_full_cue_fails_instead_of_disappearing() -> TestResult {
    // Given
    let mut segments = (0..MAX_WORDS_PER_CUE)
        .map(|index| segment(index as u64, index as u64 + 1, "word"))
        .collect::<Vec<_>>();
    segments.push(segment(20, 20, "tail"));
    let mut cues = CueAccumulator::new();
    cues.append(output(segments), layout().window(0)?)?;
    // When
    let result = cues.into_cues();
    // Then
    assert!(result.is_err());
    Ok(())
}

#[test]
fn raw_context_candidates_are_bounded_even_when_none_are_retained() -> TestResult {
    // Given
    let candidates = (0..300_001)
        .map(|_| segment(31_000, 31_100, "context"))
        .collect();
    let mut cues = CueAccumulator::new();
    // When
    let result = cues.append(output(candidates), layout().window(0)?);
    // Then
    assert!(result.is_err());
    Ok(())
}
