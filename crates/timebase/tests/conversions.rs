//! Regression coverage for exact source, session, and output conversions.

#![allow(
    clippy::expect_used,
    reason = "test fixtures fail most clearly at the construction site"
)]

use longcut_timebase::{OutputRate, Rounding, SessionTime, SourceTime, TimeBase};

#[test]
fn maps_ntsc_source_to_session_and_output_exactly() {
    // Given
    let time_base = TimeBase::new(1001, 60_000).expect("valid time base");
    let output_rate = OutputRate::new(60_000, 1001).expect("valid output rate");
    let source = SourceTime::new(60_000, time_base);

    // When
    let session = source
        .to_session(Rounding::Exact)
        .expect("exact nanoseconds");
    let frame = output_rate
        .session_to_frame(session, Rounding::Exact)
        .expect("exact frame");

    // Then
    assert_eq!(session, SessionTime::new(1_001_000_000_000));
    assert_eq!(frame.ticks(), 60_000);
}

#[test]
fn maps_audio_ticks_and_roundtrips_session() {
    // Given
    let time_base = TimeBase::new(1, 48_000).expect("valid audio time base");
    let source = SourceTime::new(48_000, time_base);

    // When
    let session = source
        .to_session(Rounding::Exact)
        .expect("exact nanoseconds");
    let restored =
        SourceTime::from_session(session, time_base, Rounding::Exact).expect("exact source tick");

    // Then
    assert_eq!(session, SessionTime::new(1_000_000_000));
    assert_eq!(restored, source);
}

#[test]
fn preserves_ticks_beyond_javascript_safe_integer() {
    // Given
    let time_base = TimeBase::new(1, 1_000_000_000).expect("valid nanosecond base");
    let source = SourceTime::new(9_007_199_254_740_993, time_base);

    // When
    let session = source
        .to_session(Rounding::Exact)
        .expect("exact conversion");

    // Then
    assert_eq!(session.ticks(), 9_007_199_254_740_993);
}

#[test]
fn applies_explicit_rounding_to_negative_values() {
    // Given
    let time_base = TimeBase::new(1, 3_000_000_000).expect("valid time base");
    let source = SourceTime::new(-1, time_base);

    // When
    let floor = source.to_session(Rounding::Floor).expect("rounded down");
    let ceil = source.to_session(Rounding::Ceil).expect("rounded up");

    // Then
    assert_eq!(floor.ticks(), -1);
    assert_eq!(ceil.ticks(), 0);
}

#[test]
fn reports_inexact_and_overflow_without_panicking() {
    // Given
    let thirds = SourceTime::new(1, TimeBase::new(1, 3).expect("valid time base"));
    let huge = SourceTime::new(i64::MAX, TimeBase::new(1, 1).expect("valid time base"));

    // When
    let inexact = thirds.to_session(Rounding::Exact);
    let overflow = huge.to_session(Rounding::Floor);

    // Then
    assert!(inexact.is_err());
    assert!(overflow.is_err());
}
