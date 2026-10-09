use super::*;
use crate::asr::whisper::{WhisperOffsets, WhisperResult, WhisperSegment};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn output(segments: Vec<WhisperSegment>) -> WhisperOutput {
    WhisperOutput {
        transcription: segments,
        result: WhisperResult { language: None },
    }
}

fn segment(from: u64, to: u64, text: impl Into<String>) -> WhisperSegment {
    WhisperSegment {
        offsets: WhisperOffsets { from, to },
        text: text.into(),
    }
}

const fn layout(chunk_frames: u64, duration_frames: u64, frame_rate: u32) -> ChunkLayout {
    ChunkLayout {
        chunk_frames,
        duration_frames,
        frame_rate,
    }
}

#[test]
fn inference_windows_extend_two_seconds_at_source_edges_and_middle() -> TestResult {
    // Given
    let chunks = layout(300, 750, 30);
    // When
    let windows = [chunks.window(0)?, chunks.window(1)?, chunks.window(2)?];
    // Then
    assert_eq!(windows[0].bounds(), (0, 360, 0, 300));
    assert_eq!(windows[1].bounds(), (240, 660, 300, 600));
    assert_eq!(windows[2].bounds(), (540, 750, 600, 750));
    Ok(())
}

#[test]
fn midpoint_on_owner_boundary_belongs_to_right_chunk_once() -> TestResult {
    // Given
    let chunks = layout(300, 600, 30);
    let mut cues = CueAccumulator::new();
    // When
    cues.append(
        output(vec![segment(9_000, 11_000, " boundary")]),
        chunks.window(0)?,
    )?;
    cues.append(
        output(vec![segment(1_000, 3_000, " boundary")]),
        chunks.window(1)?,
    )?;
    let cues = cues.into_cues()?;
    // Then
    assert_eq!(cues.len(), 1);
    assert_eq!(cues[0].words.as_ref().map(Vec::len), Some(1));
    assert_eq!(cues[0].start_frame, "270");
    assert_eq!(cues[0].end_frame, "330");
    Ok(())
}

#[test]
fn neighboring_overlap_candidates_with_normalized_text_are_reconciled() -> TestResult {
    // Given
    let chunks = layout(10_000, 20_000, 1_000);
    let mut cues = CueAccumulator::new();
    // When
    cues.append(
        output(vec![segment(9_700, 10_100, "  repeat  ")]),
        chunks.window(0)?,
    )?;
    cues.append(
        output(vec![segment(1_900, 2_300, "repeat")]),
        chunks.window(1)?,
    )?;
    let cues = cues.into_cues()?;
    // Then
    assert_eq!(
        cues.iter()
            .flat_map(|cue| cue.words.iter().flatten())
            .count(),
        1
    );
    Ok(())
}

#[test]
fn repeated_spoken_word_at_distinct_times_is_not_globally_merged() -> TestResult {
    // Given
    let chunks = layout(10_000, 20_000, 1_000);
    let mut cues = CueAccumulator::new();
    // When
    cues.append(
        output(vec![
            segment(1_000, 1_400, "again"),
            segment(2_000, 2_400, "again"),
        ]),
        chunks.window(0)?,
    )?;
    let cues = cues.into_cues()?;
    // Then
    assert_eq!(
        cues.iter()
            .flat_map(|cue| cue.words.iter().flatten())
            .count(),
        2
    );
    Ok(())
}

#[test]
fn inference_offset_rounding_translates_from_inference_start() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    let window = layout(300, 600, 30).window(1)?;
    // When
    cues.append(output(vec![segment(2_001, 3_001, "bounded")]), window)?;
    let cues = cues.into_cues()?;
    // Then
    assert_eq!(cues[0].start_frame, "300");
    assert_eq!(cues[0].end_frame, "331");
    let words = cues[0].words.as_ref().ok_or("ASR words")?;
    assert_eq!(words[0].start_frame, "300");
    Ok(())
}

#[test]
fn zero_duration_punctuation_is_preserved_without_fabricated_duration() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    // When
    cues.append(
        output(vec![segment(0, 900, "word"), segment(900, 900, ".")]),
        layout(30, 30, 30).window(0)?,
    )?;
    let cues = cues.into_cues()?;
    // Then
    let words = cues[0].words.as_ref().ok_or("ASR words")?;
    assert_eq!(cues[0].text, "word.");
    assert_eq!(
        (words[1].start_frame.as_str(), words[1].end_frame.as_str()),
        ("27", "27")
    );
    Ok(())
}

#[test]
fn malformed_or_out_of_inference_offsets_are_rejected() -> TestResult {
    // Given
    let window = layout(30, 30, 30).window(0)?;
    // When
    let reversed = CueAccumulator::new().append(output(vec![segment(1_000, 500, "bad")]), window);
    let outside = CueAccumulator::new().append(output(vec![segment(0, 1_001, "bad")]), window);
    // Then
    assert!(reversed.is_err());
    assert!(outside.is_err());
    Ok(())
}

#[test]
fn one_hundred_thousand_words_aggregate_below_cue_limit() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    let words = (0..MAX_WORDS)
        .map(|index| segment(index as u64, index as u64 + 1, "x"))
        .collect();
    // When
    cues.append(
        output(words),
        layout(MAX_WORDS as u64, MAX_WORDS as u64, 1_000).window(0)?,
    )?;
    let cues = cues.into_cues()?;
    // Then
    assert_eq!(cues.len(), MAX_WORDS.div_ceil(MAX_WORDS_PER_CUE));
    assert!(cues.len() < MAX_CUES);
    assert!(cues.iter().all(|cue| {
        cue.words
            .as_ref()
            .is_some_and(|words| words.len() <= MAX_WORDS_PER_CUE)
    }));
    Ok(())
}

#[test]
fn append_rejects_word_and_aggregate_text_limits_before_retention() -> TestResult {
    // Given
    let mut oversized = CueAccumulator::new();
    let word = "가".repeat((MAX_CUE_TEXT_BYTES / 3) + 1);
    let mut aggregate = CueAccumulator {
        words: Vec::new(),
        text_bytes: MAX_JOB_TEXT_BYTES,
        previous_chunk_start: 0,
        processed_candidates: 0,
    };
    let window = layout(30, 30, 30).window(0)?;
    // When
    let word_result = oversized.append(output(vec![segment(0, 1_000, word)]), window);
    let aggregate_result = aggregate.append(output(vec![segment(0, 1_000, "x")]), window);
    // Then
    assert!(word_result.is_err());
    assert!(aggregate_result.is_err());
    Ok(())
}

#[test]
fn readable_cues_split_before_aggregate_text_exceeds_cue_limit() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    let text = "x".repeat(10_000);
    // When
    cues.append(
        output(vec![segment(0, 1, text.clone()), segment(1, 2, text)]),
        layout(2, 2, 1_000).window(0)?,
    )?;
    let cues = cues.into_cues()?;
    // Then
    assert_eq!(cues.len(), 2);
    assert!(cues.iter().all(|cue| cue.text.len() <= MAX_CUE_TEXT_BYTES));
    Ok(())
}

#[test]
fn silence_does_not_fabricate_cues() -> TestResult {
    // Given
    let mut cues = CueAccumulator::new();
    // When
    cues.append(output(Vec::new()), layout(30, 30, 30).window(0)?)?;
    // Then
    assert!(cues.into_cues()?.is_empty());
    Ok(())
}
