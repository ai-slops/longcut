use crate::{
    error::{AppError, AppResult},
    model::{SourceRole, StreamMapping},
};
use longcut_session_contract::model::{
    Binding, MediaTime, Package, SessionRange, Source, SourceKind, StreamKind,
};
use longcut_timebase::{Rounding, SourceTime};
use std::cmp::Ordering;

#[cfg(test)]
mod tests;

pub(super) struct Plan {
    pub bindings: Vec<Binding>,
    pub mappings: Vec<StreamMapping>,
    pub session_range: SessionRange,
    pub frames: String,
    pub duration: String,
    pub input_offset: String,
    pub role: SourceRole,
}

struct MediaSpan {
    nanoseconds: i64,
    frames: u64,
}

fn media_span(start: &MediaTime, end: &MediaTime, frame_rate: u32) -> AppResult<MediaSpan> {
    let unsupported = |reason: &str| AppError::Invalid(reason.into());
    let start_numerator = i128::from(start.ticks.get())
        .checked_mul(i128::from(start.timebase_num))
        .ok_or_else(|| unsupported("media range overflow"))?;
    let end_numerator = i128::from(end.ticks.get())
        .checked_mul(i128::from(end.timebase_num))
        .ok_or_else(|| unsupported("media range overflow"))?;
    let numerator = end_numerator
        .checked_mul(i128::from(start.timebase_den))
        .and_then(|value| {
            start_numerator
                .checked_mul(i128::from(end.timebase_den))
                .and_then(|start_value| value.checked_sub(start_value))
        })
        .ok_or_else(|| unsupported("media range overflow"))?;
    let denominator = i128::from(end.timebase_den)
        .checked_mul(i128::from(start.timebase_den))
        .ok_or_else(|| unsupported("media range overflow"))?;
    if numerator <= 0 || denominator <= 0 {
        return Err(unsupported("invalid media range"));
    }
    let micros_numerator = numerator
        .checked_mul(1_000_000)
        .ok_or_else(|| unsupported("media duration overflow"))?;
    if micros_numerator % denominator != 0 {
        return Err(unsupported(
            "media duration cannot be adapted at FFmpeg microsecond precision",
        ));
    }
    let nanos_numerator = numerator
        .checked_mul(1_000_000_000)
        .ok_or_else(|| unsupported("media duration overflow"))?;
    let quotient = nanos_numerator / denominator;
    let remainder = nanos_numerator % denominator;
    let rounded_nanos = quotient
        .checked_add(i128::from(remainder * 2 >= denominator))
        .ok_or_else(|| unsupported("media duration overflow"))?;
    let nanoseconds =
        i64::try_from(rounded_nanos).map_err(|_| unsupported("media duration overflow"))?;
    let frame_count = numerator
        .checked_mul(i128::from(frame_rate))
        .and_then(|value| value.checked_add(denominator - 1))
        .ok_or_else(|| unsupported("frame count overflow"))?
        / denominator;
    let frames =
        u64::try_from(frame_count).map_err(|_| unsupported("invalid adapter frame count"))?;
    Ok(MediaSpan {
        nanoseconds,
        frames,
    })
}

pub(super) fn build(package: &Package, source: &Source, frame_rate: u32) -> AppResult<Plan> {
    let unsupported = |reason: &str| AppError::Invalid(reason.into());
    if source.kind == SourceKind::Audio {
        return Err(unsupported(
            "audio-only source is preserved; standalone stem editing is not yet available",
        ));
    }
    let track = package
        .time_maps
        .as_ref()
        .ok_or_else(|| unsupported("explicit time maps are needed for editor adaptation"))?;
    let mut video = Vec::new();
    let mut audio = Vec::new();
    for binding in &source.media_bindings {
        if binding.channel_indices.is_some() {
            return Err(unsupported("channel-subset adapters are not yet available"));
        }
        let stream = package
            .registry
            .assets
            .iter()
            .find(|asset| asset.asset_id == binding.asset_id)
            .and_then(|asset| {
                asset
                    .streams
                    .iter()
                    .find(|stream| stream.stream_index == binding.stream_index)
            })
            .ok_or(AppError::NotFound)?;
        match stream.kind {
            StreamKind::Video => video.push(binding.clone()),
            StreamKind::Audio => audio.push(binding.clone()),
        }
    }
    if video.len() != 1 || audio.len() > 1 {
        return Err(unsupported(
            "multi-segment or multi-video sources require a timeline adapter",
        ));
    }
    video.extend(audio);
    let mut mappings = Vec::new();
    let mut session: Option<SessionRange> = None;
    let mut frames = 0_u64;
    let mut duration_ns = 0_i64;
    let mut media_origin: Option<SourceTime> = None;
    for binding in &video {
        let segments: Vec<_> = track
            .segments
            .iter()
            .filter(|segment| {
                segment.media_ref.asset_id == binding.asset_id
                    && segment.media_ref.stream_index == binding.stream_index
            })
            .collect();
        if segments.len() != 1 {
            return Err(unsupported(
                "discontinuous or split stream mappings require a timeline adapter",
            ));
        }
        let segment = segments[0];
        let (start, end) = segment.session_range.bounds();
        let span = end
            .checked_sub(start)
            .ok_or_else(|| unsupported("session range overflow"))?;
        let origin = segment
            .media_range
            .start
            .source_time()
            .map_err(|error| unsupported(&error.to_string()))?;
        if let Some(existing) = media_origin {
            if existing
                .compare(origin)
                .map_err(|error| unsupported(&error.to_string()))?
                != Ordering::Equal
            {
                return Err(unsupported("AV bindings have different media origins"));
            }
        } else {
            media_origin = Some(origin);
        }
        let media_span = media_span(
            &segment.media_range.start,
            &segment.media_range.end,
            frame_rate,
        )?;
        if (i128::from(media_span.nanoseconds) - i128::from(span)).abs() > 1 {
            return Err(unsupported(
                "clock-drift correction is preserved but requires a retiming adapter",
            ));
        }
        if let Some(existing) = &session {
            if existing.bounds() != segment.session_range.bounds() {
                return Err(unsupported("AV bindings have different session intervals"));
            }
        } else {
            session = Some(segment.session_range.clone());
            duration_ns = media_span.nanoseconds;
            frames = media_span.frames;
        }
        mappings.push(StreamMapping {
            asset_id: binding.asset_id.0.clone(),
            stream_index: binding.stream_index,
            media_start: segment.media_range.start.clone(),
            media_end: segment.media_range.end.clone(),
        });
    }
    if frames == 0 {
        return Err(unsupported(
            "source interval is shorter than one output frame",
        ));
    }
    let role = if package.manifest.program_reference_source_id.as_ref() == Some(&source.source_id) {
        SourceRole::Program
    } else if source.semantic_role.contains("camera") || source.semantic_role.contains("reaction") {
        SourceRole::Camera
    } else {
        SourceRole::Primary
    };
    let origin_ns = media_origin
        .ok_or_else(|| unsupported("source has no mapping"))?
        .to_session(Rounding::Exact)
        .map_err(|_| unsupported("media origin cannot be normalized without rounding"))?
        .ticks();
    let shifted_ns = origin_ns
        .checked_neg()
        .ok_or_else(|| unsupported("media origin overflow"))?;
    if shifted_ns % 1_000 != 0 {
        return Err(unsupported(
            "media origin cannot be normalized at FFmpeg microsecond precision",
        ));
    }
    let offset_sign = if shifted_ns < 0 { "-" } else { "" };
    let offset_magnitude = i128::from(shifted_ns).abs();
    Ok(Plan {
        bindings: video,
        mappings,
        session_range: session.ok_or_else(|| unsupported("source has no mapping"))?,
        frames: frames.to_string(),
        duration: format!(
            "{}.{:09}",
            duration_ns / 1_000_000_000,
            duration_ns % 1_000_000_000
        ),
        input_offset: format!(
            "{offset_sign}{}.{:06}",
            offset_magnitude / 1_000_000_000,
            offset_magnitude % 1_000_000_000 / 1_000
        ),
        role,
    })
}
