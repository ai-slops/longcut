use longcut_session_contract::model::{Integer, MediaTime};

use super::media_span;

#[test]
fn fractional_output_frame_uses_exact_ceiling() -> Result<(), Box<dyn std::error::Error>> {
    // Given
    let start = MediaTime {
        ticks: Integer::new(0),
        timebase_num: 1,
        timebase_den: 20,
    };
    let end = MediaTime {
        ticks: Integer::new(21),
        timebase_num: 1,
        timebase_den: 20,
    };

    // When
    let span = media_span(&start, &end, 30)?;

    // Then
    assert_eq!(span.frames, 32);
    assert_eq!(span.nanoseconds, 1_050_000_000);
    Ok(())
}
