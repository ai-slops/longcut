//! Regression coverage for affine rounding and full-width session ranges.

use longcut_timebase::{
    MediaPoint, Rounding, Segment, SegmentSpec, SessionTime, SourceTime, StreamKey, TimeBase,
    TimeMap,
};

#[test]
fn nearest_rounds_final_negative_coordinate() -> Result<(), Box<dyn std::error::Error>> {
    // Given: source tick 1 maps to -0.5 ns, not to a positive offset in isolation.
    let base = TimeBase::new(1, 1)?;
    let stream = StreamKey::new("asset", "0")?;
    let map = TimeMap::new(vec![Segment::new(SegmentSpec {
        stream: stream.clone(),
        media_start: SourceTime::new(0, base),
        media_end: SourceTime::new(4, base),
        session_start: SessionTime::new(-1),
        session_end: SessionTime::new(1),
    })?])?;
    // When
    let result = map.source_to_session(
        &MediaPoint::new(stream, SourceTime::new(1, base)),
        Rounding::Nearest,
    )?;
    // Then: ties round away from zero in the destination coordinate space.
    assert_eq!(result, Some(SessionTime::new(-1)));
    Ok(())
}

#[test]
fn maps_full_width_session_without_intermediate_i64_overflow()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: both endpoints fit i64, but their positive difference does not.
    let base = TimeBase::new(1, 1_000_000_000)?;
    let stream = StreamKey::new("asset", "0")?;
    let map = TimeMap::new(vec![Segment::new(SegmentSpec {
        stream: stream.clone(),
        media_start: SourceTime::new(i64::MIN, base),
        media_end: SourceTime::new(i64::MAX, base),
        session_start: SessionTime::new(i64::MIN),
        session_end: SessionTime::new(i64::MAX),
    })?])?;
    // When / Then: the full-width identity map works in both directions.
    for ticks in [i64::MIN, -1, 0, 1, i64::MAX - 1] {
        assert_eq!(
            map.source_to_session(
                &MediaPoint::new(stream.clone(), SourceTime::new(ticks, base)),
                Rounding::Exact,
            )?,
            Some(SessionTime::new(ticks))
        );
        assert_eq!(
            map.session_to_source(&stream, SessionTime::new(ticks), Rounding::Exact)?,
            Some(SourceTime::new(ticks, base))
        );
    }
    Ok(())
}

#[test]
fn rational_components_survive_javascript_json_boundary() -> Result<(), Box<dyn std::error::Error>>
{
    // Given
    let base = TimeBase::new(9_007_199_254_740_993, 1)?;
    // When
    let json = serde_json::to_value(base)?;
    // Then: every i64 component is a string, not an unsafe JavaScript number.
    assert_eq!(json["numerator"], "9007199254740993");
    assert_eq!(json["denominator"], "1");
    assert_eq!(serde_json::from_value::<TimeBase>(json)?, base);
    Ok(())
}
