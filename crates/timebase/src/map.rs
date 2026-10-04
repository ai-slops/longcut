use std::cmp::Ordering;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{MapError, Rounding, SessionTime, SourceTime};

/// Stable asset and stream identity used to isolate mappings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct StreamKey {
    asset: String,
    stream: String,
}

impl<'de> Deserialize<'de> for StreamKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            asset: String,
            stream: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        Self::new(&wire.asset, &wire.stream).map_err(serde::de::Error::custom)
    }
}

impl StreamKey {
    /// Constructs a non-empty asset and stream key.
    pub fn new(asset: &str, stream: &str) -> Result<Self, MapError> {
        if asset.is_empty() || stream.is_empty() {
            return Err(MapError::EmptyStreamKey);
        }
        Ok(Self {
            asset: asset.to_owned(),
            stream: stream.to_owned(),
        })
    }

    /// Returns the asset identifier.
    pub fn asset(&self) -> &str {
        &self.asset
    }

    /// Returns the stream identifier.
    pub fn stream(&self) -> &str {
        &self.stream
    }
}

/// Source lookup point including its stream identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPoint {
    stream: StreamKey,
    time: SourceTime,
}

impl MediaPoint {
    /// Constructs a media lookup point.
    pub const fn new(stream: StreamKey, time: SourceTime) -> Self {
        Self { stream, time }
    }
}

/// Validated input fields for one linear mapping segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentSpec {
    /// Asset and stream identity.
    pub stream: StreamKey,
    /// Inclusive source start.
    pub media_start: SourceTime,
    /// Exclusive source end.
    pub media_end: SourceTime,
    /// Inclusive session start in nanoseconds.
    pub session_start: SessionTime,
    /// Exclusive session end in nanoseconds.
    pub session_end: SessionTime,
}

/// A validated half-open linear mapping with explicit endpoint drift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Segment(SegmentSpec);

impl Segment {
    /// Validates and constructs a half-open segment.
    pub fn new(spec: SegmentSpec) -> Result<Self, MapError> {
        if spec
            .media_start
            .seconds()?
            .compare(spec.media_end.seconds()?)?
            != Ordering::Less
        {
            return Err(MapError::InvalidMediaRange);
        }
        if spec.session_start >= spec.session_end {
            return Err(MapError::InvalidSessionRange);
        }
        Ok(Self(spec))
    }

    /// Returns the validated segment fields.
    pub const fn spec(&self) -> &SegmentSpec {
        &self.0
    }

    fn contains_source(&self, point: SourceTime) -> Result<bool, MapError> {
        let value = point.seconds()?;
        Ok(
            value.compare(self.0.media_start.seconds()?)? != Ordering::Less
                && value.compare(self.0.media_end.seconds()?)? == Ordering::Less,
        )
    }

    fn contains_session(&self, time: SessionTime) -> bool {
        time >= self.0.session_start && time < self.0.session_end
    }

    fn source_to_session(
        &self,
        source: SourceTime,
        rounding: Rounding,
    ) -> Result<SessionTime, MapError> {
        let media_start = self.0.media_start.seconds()?;
        let media_duration = self.0.media_end.seconds()?.checked_sub(media_start)?;
        let progress = source
            .seconds()?
            .checked_sub(media_start)?
            .checked_div(media_duration)?;
        let start = self.0.session_start.ratio();
        let duration = self.0.session_end.ratio().checked_sub(start)?;
        let ticks = start
            .checked_add(progress.checked_mul(duration)?)?
            .round_i64(rounding)?;
        Ok(SessionTime::new(ticks))
    }

    fn session_to_source(
        &self,
        session: SessionTime,
        rounding: Rounding,
    ) -> Result<SourceTime, MapError> {
        let start = self.0.session_start.ratio();
        let session_offset = session.ratio().checked_sub(start)?;
        let session_duration = self.0.session_end.ratio().checked_sub(start)?;
        let progress = session_offset.checked_div(session_duration)?;
        let media_start = self.0.media_start.seconds()?;
        let media_duration = self.0.media_end.seconds()?.checked_sub(media_start)?;
        let seconds = media_start.checked_add(progress.checked_mul(media_duration)?)?;
        let time_base = self.0.media_start.time_base();
        let ticks = seconds
            .checked_div(time_base.ratio()?)?
            .round_i64(rounding)?;
        Ok(SourceTime::new(ticks, time_base))
    }
}

impl<'de> Deserialize<'de> for Segment {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(SegmentSpec::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

/// Validated piecewise mappings isolated by asset and stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TimeMap {
    segments: Vec<Segment>,
}

impl TimeMap {
    /// Validates segment ordering and overlap constraints.
    pub fn new(segments: Vec<Segment>) -> Result<Self, MapError> {
        for (index, left) in segments.iter().enumerate() {
            for right in segments.iter().skip(index + 1) {
                if left.0.stream == right.0.stream {
                    validate_pair(left, right)?;
                }
            }
        }
        Ok(Self { segments })
    }

    /// Maps a source point, returning `None` for explicit gaps and end boundaries.
    pub fn source_to_session(
        &self,
        point: &MediaPoint,
        rounding: Rounding,
    ) -> Result<Option<SessionTime>, MapError> {
        for segment in &self.segments {
            if segment.0.stream == point.stream && segment.contains_source(point.time)? {
                return segment.source_to_session(point.time, rounding).map(Some);
            }
        }
        Ok(None)
    }

    /// Maps a session point back to source, returning `None` for gaps and end boundaries.
    pub fn session_to_source(
        &self,
        stream: &StreamKey,
        session: SessionTime,
        rounding: Rounding,
    ) -> Result<Option<SourceTime>, MapError> {
        for segment in &self.segments {
            if &segment.0.stream == stream && segment.contains_session(session) {
                return segment.session_to_source(session, rounding).map(Some);
            }
        }
        Ok(None)
    }
}

impl<'de> Deserialize<'de> for TimeMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            segments: Vec<Segment>,
        }

        Self::new(Wire::deserialize(deserializer)?.segments).map_err(serde::de::Error::custom)
    }
}

fn validate_pair(left: &Segment, right: &Segment) -> Result<(), MapError> {
    let left_before_media = left
        .0
        .media_end
        .seconds()?
        .compare(right.0.media_start.seconds()?)?
        != Ordering::Greater;
    let right_before_media = right
        .0
        .media_end
        .seconds()?
        .compare(left.0.media_start.seconds()?)?
        != Ordering::Greater;
    if !left_before_media && !right_before_media {
        return Err(MapError::MediaOverlap {
            asset: left.0.stream.asset.clone(),
            stream: left.0.stream.stream.clone(),
        });
    }
    let left_before_session = left.0.session_end <= right.0.session_start;
    let right_before_session = right.0.session_end <= left.0.session_start;
    if !left_before_session && !right_before_session {
        return Err(MapError::SessionOverlap {
            asset: left.0.stream.asset.clone(),
            stream: left.0.stream.stream.clone(),
        });
    }
    if left_before_media != left_before_session {
        return Err(MapError::ReversedOrder {
            asset: left.0.stream.asset.clone(),
            stream: left.0.stream.stream.clone(),
        });
    }
    Ok(())
}
