use super::{Context, intervals::track_coverage, unit, valid_range};
use crate::{Report, model::*};

pub(super) fn validate_tracks(context: &Context<'_>, report: &mut Report) {
    visual(context, report);
    audio(context, report);
}

fn contribution(context: &Context<'_>, source_id: &SourceId, kind: StreamKind) -> bool {
    let program = context
        .package
        .manifest
        .program_reference_source_id
        .as_ref()
        .and_then(|id| context.index.sources.get(id).copied());
    context
        .index
        .sources
        .get(source_id)
        .is_some_and(|source| context.index.independent_kind(source, program, kind))
}

fn visual(context: &Context<'_>, report: &mut Report) {
    let Some(track) = &context.package.visual_composition else {
        report.warning(
            "VISUAL_COMPOSITION_MISSING",
            "visual-composition.json",
            "no visual composition track provided",
        );
        return;
    };
    let before = report.diagnostics.len();
    let mut ranges = Vec::new();
    let mut has_sources = false;
    let mut fully_known = true;
    for (position, interval) in track.intervals.iter().enumerate() {
        fully_known &= matches!(interval.coverage, Some(Coverage::Complete));
        let path = format!("visual-composition.json/intervals/{position}");
        if !valid_range(&interval.session_range, context.duration)
            || !unit(interval.confidence)
            || interval.canvas.width == 0
            || interval.canvas.height == 0
        {
            report.error(
                "INVALID_INTERVAL",
                &path,
                "invalid range, confidence or canvas dimensions",
            );
        }
        ranges.push(interval.session_range.bounds());
        for (offset, value) in interval.contributions.iter().enumerate() {
            let path = format!("{path}/contributions/{offset}");
            if !contribution(context, &value.source_id, StreamKind::Video) {
                report.error(
                    "INVALID_VISUAL_SOURCE",
                    &path,
                    "visual contribution must resolve to independent video, not Program reference",
                );
            }
            if value.opacity.is_some_and(|opacity| !unit(opacity))
                || value
                    .normalized_rect
                    .as_ref()
                    .is_some_and(|rect| !valid_rect(rect))
                || value.crop.as_ref().is_some_and(|crop| !valid_crop(crop))
            {
                report.error("INVALID_GEOMETRY", &path, "geometry and opacity must be finite normalized values with positive visible area");
            }
            has_sources = true;
        }
    }
    match track_coverage(&mut ranges, context.duration, "visual-composition.json") {
        Err(message) => report.error("TRACK_OVERLAP", "visual-composition.json", message),
        Ok(false) => report.warning(
            "VISUAL_COVERAGE_GAP",
            "visual-composition.json",
            "declare unknown intervals explicitly with partial coverage",
        ),
        Ok(true) => {}
    }
    report.capabilities.semantic_visual_reconstruction = has_sources
        && fully_known
        && before == report.diagnostics.len()
        && report.capabilities.session_time_map;
}

fn audio(context: &Context<'_>, report: &mut Report) {
    let Some(track) = &context.package.audio_mix else {
        report.warning(
            "AUDIO_MIX_MISSING",
            "audio-mix.json",
            "no audio mix track provided",
        );
        return;
    };
    let before = report.diagnostics.len();
    let mut ranges = Vec::new();
    let mut has_sources = false;
    let mut fully_known = true;
    for (position, interval) in track.intervals.iter().enumerate() {
        fully_known &= matches!(interval.coverage, Some(Coverage::Complete));
        let path = format!("audio-mix.json/intervals/{position}");
        if !valid_range(&interval.session_range, context.duration) || !unit(interval.confidence) {
            report.error("INVALID_INTERVAL", &path, "invalid range or confidence");
        }
        ranges.push(interval.session_range.bounds());
        for (offset, value) in interval.contributions.iter().enumerate() {
            let path = format!("{path}/contributions/{offset}");
            if !contribution(context, &value.source_id, StreamKind::Audio) {
                report.error(
                    "INVALID_AUDIO_SOURCE",
                    &path,
                    "mix contribution must resolve to independent audio, not Program reference",
                );
            }
            if value.gain_db.is_some_and(|gain| !gain.is_finite())
                || value
                    .pan
                    .is_some_and(|pan| !pan.is_finite() || !(-1.0..=1.0).contains(&pan))
                || value.confidence.is_some_and(|confidence| !unit(confidence))
            {
                report.error("INVALID_MIX", &path, "invalid gain, pan or confidence");
            }
            has_sources = true;
        }
    }
    match track_coverage(&mut ranges, context.duration, "audio-mix.json") {
        Err(message) => report.error("TRACK_OVERLAP", "audio-mix.json", message),
        Ok(false) => report.warning(
            "AUDIO_COVERAGE_GAP",
            "audio-mix.json",
            "declare unknown intervals explicitly with partial coverage",
        ),
        Ok(true) => {}
    }
    report.capabilities.semantic_audio_reconstruction = has_sources
        && fully_known
        && before == report.diagnostics.len()
        && report.capabilities.session_time_map;
}

fn valid_rect(rect: &Rect) -> bool {
    [rect.x, rect.y, rect.width, rect.height]
        .into_iter()
        .all(unit)
        && rect.width > 0.0
        && rect.height > 0.0
        && rect.x + rect.width <= 1.0
        && rect.y + rect.height <= 1.0
}

fn valid_crop(crop: &Crop) -> bool {
    [crop.top, crop.right, crop.bottom, crop.left]
        .into_iter()
        .all(unit)
        && crop.top + crop.bottom < 1.0
        && crop.left + crop.right < 1.0
}
