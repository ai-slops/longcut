//! Regression coverage for piecewise stream mapping invariants and gaps.

#![allow(
    clippy::expect_used,
    reason = "test fixtures fail most clearly at the construction site"
)]

use longcut_timebase::{
    MapError, MediaPoint, Rounding, Segment, SegmentSpec, SessionTime, SourceTime, StreamKey,
    TimeBase, TimeMap,
};

fn audio(ticks: i64) -> SourceTime {
    SourceTime::new(ticks, TimeBase::new(1, 48_000).expect("valid audio base"))
}

fn key(asset: &str, stream: &str) -> StreamKey {
    StreamKey::new(asset, stream).expect("non-empty key")
}

fn segment(stream: StreamKey, media: (SourceTime, SourceTime), session: (i64, i64)) -> Segment {
    Segment::new(SegmentSpec {
        stream,
        media_start: media.0,
        media_end: media.1,
        session_start: SessionTime::new(session.0),
        session_end: SessionTime::new(session.1),
    })
    .expect("valid segment")
}

#[test]
fn maps_drift_with_different_point_and_endpoint_bases() {
    // Given
    let stream = key("asset-a", "audio-0");
    let endpoint = TimeBase::new(1, 96_000).expect("valid endpoint base");
    let map = TimeMap::new(vec![segment(
        stream.clone(),
        (audio(0), SourceTime::new(96_000, endpoint)),
        (0, 1_001_000_000),
    )])
    .expect("valid map");
    let point = MediaPoint::new(stream, audio(24_000));

    // When
    let mapped = map
        .source_to_session(&point, Rounding::Exact)
        .expect("mapping succeeds");

    // Then
    assert_eq!(mapped, Some(SessionTime::new(500_500_000)));
}

#[test]
fn maps_negative_origin_and_inverse_roundtrips() {
    // Given
    let stream = key("asset-a", "audio-0");
    let map = TimeMap::new(vec![segment(
        stream.clone(),
        (audio(-48_000), audio(0)),
        (10_000_000_000, 11_000_000_000),
    )])
    .expect("valid map");
    let source = MediaPoint::new(stream.clone(), audio(-24_000));

    // When
    let session = map
        .source_to_session(&source, Rounding::Exact)
        .expect("mapping succeeds")
        .expect("inside segment");
    let restored = map
        .session_to_source(&stream, session, Rounding::Exact)
        .expect("inverse succeeds");

    // Then
    assert_eq!(restored, Some(audio(-24_000)));
}

#[test]
fn half_open_discontinuities_return_gaps_and_select_next_start() {
    // Given
    let stream = key("asset-a", "audio-0");
    let map = TimeMap::new(vec![
        segment(
            stream.clone(),
            (audio(0), audio(48_000)),
            (0, 1_000_000_000),
        ),
        segment(
            stream.clone(),
            (audio(96_000), audio(144_000)),
            (2_000_000_000, 3_000_000_000),
        ),
    ])
    .expect("valid discontinuous map");

    // When
    let media_gap = map
        .source_to_session(
            &MediaPoint::new(stream.clone(), audio(48_000)),
            Rounding::Exact,
        )
        .expect("gap lookup succeeds");
    let session_gap = map
        .session_to_source(&stream, SessionTime::new(1_500_000_000), Rounding::Exact)
        .expect("gap lookup succeeds");
    let next = map
        .source_to_session(&MediaPoint::new(stream, audio(96_000)), Rounding::Exact)
        .expect("boundary lookup succeeds");

    // Then
    assert_eq!(media_gap, None);
    assert_eq!(session_gap, None);
    assert_eq!(next, Some(SessionTime::new(2_000_000_000)));
}

#[test]
fn rejects_invalid_overlapping_and_reversed_stream_mappings() {
    // Given
    let stream = key("asset-a", "audio-0");
    let invalid = Segment::new(SegmentSpec {
        stream: stream.clone(),
        media_start: audio(1),
        media_end: audio(0),
        session_start: SessionTime::new(0),
        session_end: SessionTime::new(1),
    });
    let overlapping = TimeMap::new(vec![
        segment(
            stream.clone(),
            (audio(0), audio(48_000)),
            (0, 1_000_000_000),
        ),
        segment(
            stream.clone(),
            (audio(24_000), audio(72_000)),
            (2_000_000_000, 3_000_000_000),
        ),
    ]);
    let reversed = TimeMap::new(vec![
        segment(
            stream.clone(),
            (audio(0), audio(48_000)),
            (2_000_000_000, 3_000_000_000),
        ),
        segment(stream, (audio(96_000), audio(144_000)), (0, 1_000_000_000)),
    ]);

    // When / Then
    assert!(matches!(invalid, Err(MapError::InvalidMediaRange)));
    assert!(matches!(overlapping, Err(MapError::MediaOverlap { .. })));
    assert!(matches!(reversed, Err(MapError::ReversedOrder { .. })));
}

#[test]
fn permits_distinct_streams_to_share_session_ranges() {
    // Given
    let first = key("asset-a", "audio-0");
    let second = key("asset-a", "video-0");

    // When
    let map = TimeMap::new(vec![
        segment(first, (audio(0), audio(48_000)), (0, 1_000_000_000)),
        segment(second, (audio(0), audio(48_000)), (0, 1_000_000_000)),
    ]);

    // Then
    assert!(map.is_ok());
}
