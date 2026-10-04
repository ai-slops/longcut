mod health;
mod intervals;
mod limits;
mod registry;
mod tracks;

use crate::{Conformance, Report, model::*};

pub fn validate(package: &Package) -> Report {
    let mut report = Report::default();
    if !limits::within_budget(package) {
        report.error(
            "VALIDATION_LIMIT_EXCEEDED",
            "package",
            "metadata exceeds the bounded validation work budget",
        );
        return report;
    }
    let manifest = &package.manifest;
    let patch = manifest.version.strip_prefix("0.1.");
    if !patch.is_some_and(|value| {
        value
            .parse::<u64>()
            .is_ok_and(|number| number.to_string() == value)
    }) {
        report.error(
            "UNSUPPORTED_VERSION",
            "manifest.json/version",
            "only 0.1.x is supported",
        );
    }
    if manifest.session_id.trim().is_empty() {
        report.error(
            "EMPTY_ID",
            "manifest.json/session_id",
            "session ID is required",
        );
    }
    for path in [
        Some(&manifest.sources_file),
        manifest.time_maps_file.as_ref(),
        manifest.visual_composition_file.as_ref(),
        manifest.audio_mix_file.as_ref(),
        manifest.media_health_file.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if !crate::loader::safe_relative(path) {
            report.error(
                "UNSAFE_PATH",
                "manifest.json",
                "metadata paths must be local relative paths",
            );
        }
    }
    let duration = match manifest.duration.nanoseconds() {
        Ok(value) if value > 0 => value,
        _ => {
            report.error(
                "INVALID_DURATION",
                "manifest.json/duration",
                "duration must be positive exact i64 nanoseconds",
            );
            return report.finish();
        }
    };
    let mut extensions = vec![
        (&manifest.extensions, "manifest.json/extensions"),
        (&package.registry.extensions, "sources.json/extensions"),
    ];
    if let Some(track) = &package.time_maps {
        extensions.push((&track.extensions, "time-maps.json/extensions"));
    }
    if let Some(track) = &package.visual_composition {
        extensions.push((&track.extensions, "visual-composition.json/extensions"));
    }
    if let Some(track) = &package.audio_mix {
        extensions.push((&track.extensions, "audio-mix.json/extensions"));
    }
    if let Some(track) = &package.media_health {
        extensions.push((&track.extensions, "media-health.json/extensions"));
    }
    for (values, path) in extensions {
        for name in values.keys() {
            if !name.contains('.')
                || name.split('.').any(|part| {
                    part.is_empty()
                        || !part
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                })
            {
                report.error(
                    "EXTENSION_NAMESPACE",
                    path,
                    "extension keys require a dotted namespace",
                );
            }
        }
    }
    let index = registry::index(package, &mut report);
    if report.metadata_conformance == Conformance::Rejected {
        return report.finish();
    }
    let context = Context {
        package,
        index,
        duration,
    };
    let ranges = intervals::time_maps(&context, &mut report);
    tracks::validate_tracks(&context, &mut report);
    health::validate_health(&context, &ranges, &mut report);
    report.finish()
}

struct Context<'a> {
    package: &'a Package,
    index: registry::Index<'a>,
    duration: i64,
}

fn unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn valid_range(range: &SessionRange, duration: i64) -> bool {
    let (start, end) = range.bounds();
    start >= 0 && start < end && end <= duration
}
