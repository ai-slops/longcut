//! Runs exact conversion and piecewise map scenarios through the public API.

use longcut_timebase::{
    MapError, MediaPoint, Rounding, Segment, SegmentSpec, SessionTime, SourceTime, StreamKey,
    TimeBase, TimeError, TimeMap,
};
use thiserror::Error;

#[derive(Debug, Error)]
enum ExampleError {
    #[error(transparent)]
    Map(#[from] MapError),
    #[error(transparent)]
    Time(#[from] TimeError),
    #[error("expected mapped point was a gap")]
    UnexpectedGap,
}

fn main() -> Result<(), ExampleError> {
    let audio_base = TimeBase::new(1, 48_000)?;
    let stream = StreamKey::new("asset-a", "audio-0")?;
    let first = Segment::new(SegmentSpec {
        stream: stream.clone(),
        media_start: SourceTime::new(-48_000, audio_base),
        media_end: SourceTime::new(0, audio_base),
        session_start: SessionTime::new(0),
        session_end: SessionTime::new(1_001_000_000),
    })?;
    let second = Segment::new(SegmentSpec {
        stream: stream.clone(),
        media_start: SourceTime::new(48_000, audio_base),
        media_end: SourceTime::new(96_000, audio_base),
        session_start: SessionTime::new(2_000_000_000),
        session_end: SessionTime::new(3_000_000_000),
    })?;
    let map = TimeMap::new(vec![first, second])?;

    let source = MediaPoint::new(stream.clone(), SourceTime::new(-24_000, audio_base));
    let session = map
        .source_to_session(&source, Rounding::Exact)?
        .ok_or(ExampleError::UnexpectedGap)?;
    let restored = map
        .session_to_source(&stream, session, Rounding::Exact)?
        .ok_or(ExampleError::UnexpectedGap)?;
    let gap = map.source_to_session(
        &MediaPoint::new(stream.clone(), SourceTime::new(24_000, audio_base)),
        Rounding::Exact,
    )?;
    let invalid = Segment::new(SegmentSpec {
        stream,
        media_start: SourceTime::new(1, audio_base),
        media_end: SourceTime::new(0, audio_base),
        session_start: SessionTime::new(0),
        session_end: SessionTime::new(1),
    });
    let large = SourceTime::new(9_007_199_254_740_993, TimeBase::new(1, 1_000_000_000)?)
        .to_session(Rounding::Exact)?;

    println!("mapped_session_ns={}", session.ticks());
    println!("inverse_source_ticks={}", restored.ticks());
    println!("gap={gap:?}");
    println!("invalid={invalid:?}");
    println!("large_exact_ns={}", large.ticks());
    Ok(())
}
