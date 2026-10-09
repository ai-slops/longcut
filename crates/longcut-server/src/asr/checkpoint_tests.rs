use super::*;
use crate::asr::whisper::{WhisperOffsets, WhisperResult, WhisperSegment};
use crate::transcript::Cue;

type WordSemantic = (String, String, String);
type CueSemantic = (String, String, String, Vec<WordSemantic>);
type TestResult = Result<(), Box<dyn std::error::Error>>;

fn output(from: u64, to: u64, text: &str) -> WhisperOutput {
    WhisperOutput {
        transcription: vec![WhisperSegment {
            offsets: WhisperOffsets { from, to },
            text: text.into(),
        }],
        result: WhisperResult {
            language: Some("en".into()),
        },
    }
}

fn semantics(cues: Vec<Cue>) -> Result<Vec<CueSemantic>, &'static str> {
    cues.into_iter()
        .map(|cue| {
            let words = cue
                .words
                .ok_or("checkpoint cue words")?
                .into_iter()
                .map(|word| (word.start_frame, word.end_frame, word.text))
                .collect();
            Ok((cue.start_frame, cue.end_frame, cue.text, words))
        })
        .collect()
}

#[test]
fn checkpoint_replay_matches_uninterrupted_accumulation_ignoring_ids() -> TestResult {
    // Given
    let layout = ChunkLayout {
        chunk_frames: 300,
        duration_frames: 600,
        frame_rate: 30,
    };
    let first = output(0, 1_000, " first");
    let second = output(2_000, 3_000, " second");
    let checkpoint = Checkpoint::from_output(0, layout.window(0)?.bounds(), &first)?;
    let mut uninterrupted = CueAccumulator::new();
    uninterrupted.append(first, layout.window(0)?)?;
    uninterrupted.append(output(2_000, 3_000, " second"), layout.window(1)?)?;
    // When
    let mut resumed = replay(vec![checkpoint], layout)?.accumulator;
    resumed.append(second, layout.window(1)?)?;
    // Then
    assert_eq!(
        semantics(resumed.into_cues()?)?,
        semantics(uninterrupted.into_cues()?)?
    );
    Ok(())
}

#[test]
fn replay_rejects_gapped_or_corrupted_checkpoints() -> TestResult {
    // Given
    let layout = ChunkLayout {
        chunk_frames: 300,
        duration_frames: 900,
        frame_rate: 30,
    };
    let bounds = layout.window(1)?.bounds();
    let gapped = Checkpoint::from_output(1, bounds, &output(2_000, 3_000, " gap"))?;
    let corrupted = Checkpoint {
        index: 0,
        payload: "{".into(),
        ..gapped.clone()
    };
    // When
    let gap_result = replay(vec![gapped], layout);
    let corruption_result = replay(vec![corrupted], layout);
    // Then
    assert!(gap_result.is_err());
    assert!(corruption_result.is_err());
    Ok(())
}
