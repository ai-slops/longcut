use crate::{
    error::{AppError, AppResult},
    model::{SourceRole, StreamMapping},
};
use longcut_session_contract::model::{
    Binding, Package, SessionRange, Source, SourceKind, StreamKind,
};
use longcut_timebase::Rounding;

pub(super) struct Plan {
    pub bindings: Vec<Binding>,
    pub mappings: Vec<StreamMapping>,
    pub session_range: SessionRange,
    pub frames: String,
    pub duration: String,
    pub role: SourceRole,
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
    let mut frames = 0_i128;
    let mut duration_ns = 0_i64;
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
        if segment.media_range.start.ticks.get() != 0 {
            return Err(unsupported(
                "nonzero source PTS origin is preserved but not yet editable in the normalized editor",
            ));
        }
        let (start, end) = segment.session_range.bounds();
        let span = end
            .checked_sub(start)
            .ok_or_else(|| unsupported("session range overflow"))?;
        let media_end = segment
            .media_range
            .end
            .source_time()
            .map_err(|error| unsupported(&error.to_string()))?;
        let end_ns = media_end
            .to_session(Rounding::Nearest)
            .map_err(|error| unsupported(&error.to_string()))?
            .ticks();
        if (i128::from(end_ns) - i128::from(span)).abs() > 1 {
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
            duration_ns = span;
            frames = i128::from(segment.media_range.end.ticks.get())
                .checked_mul(i128::from(segment.media_range.end.timebase_num))
                .and_then(|value| value.checked_mul(i128::from(frame_rate)))
                .ok_or_else(|| unsupported("frame count overflow"))?
                / i128::from(segment.media_range.end.timebase_den);
        }
        mappings.push(StreamMapping {
            asset_id: binding.asset_id.0.clone(),
            stream_index: binding.stream_index,
            media_start: segment.media_range.start.clone(),
            media_end: segment.media_range.end.clone(),
        });
    }
    let frames = u64::try_from(frames).map_err(|_| unsupported("invalid adapter frame count"))?;
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
        role,
    })
}
