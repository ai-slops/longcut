//! Read-only file integrity, packet continuity diagnostics, and optional full decode.

mod assets;
mod bounds;
mod decode;
mod files;
mod packets;
mod probe;
mod report;
mod streams;

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

pub use report::{AssetVerification, Check, Issue, PacketBounds, SegmentBounds, Verification};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DecodePolicy {
    #[default]
    Skip,
    Full,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub ffprobe: PathBuf,
    pub ffmpeg: PathBuf,
    pub decode: DecodePolicy,
    pub probe_timeout: Duration,
    pub asset_timeout: Duration,
    pub cancellation: tokio_util::sync::CancellationToken,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            ffprobe: "ffprobe".into(),
            ffmpeg: "ffmpeg".into(),
            decode: DecodePolicy::Skip,
            probe_timeout: Duration::from_secs(30),
            asset_timeout: Duration::from_secs(300),
            cancellation: tokio_util::sync::CancellationToken::new(),
        }
    }
}

/// Cancel through `Options::cancellation` and await completion to guarantee child reaping.
/// Dropping this future instead provides only the process runner's kill-on-drop fallback.
pub async fn verify(root: &Path, options: &Options) -> Verification {
    use longcut_session_contract::{Conformance, Diagnostic, Report, Severity};
    let root = root.to_path_buf();
    let load_root = root.clone();
    let loaded =
        tokio::task::spawn_blocking(move || longcut_session_contract::load(&load_root)).await;
    let (package, metadata) = match loaded {
        Ok(Ok(loaded)) => loaded.into_parts(),
        Ok(Err(report)) => return Verification::new(report),
        Err(error) => {
            let report = Report {
                metadata_conformance: Conformance::Rejected,
                diagnostics: vec![Diagnostic {
                    severity: Severity::Error,
                    code: "METADATA_WORKER_FAILED",
                    path: "package".into(),
                    message: error.to_string(),
                }],
                ..Report::default()
            };
            return Verification::new(report);
        }
    };
    let mut result = Verification::new(metadata);
    let verifier = assets::Verifier {
        root: &root,
        options,
    };
    for asset in &package.registry.assets {
        if options.cancellation.is_cancelled() {
            break;
        }
        let report = verifier.verify(asset, package.time_maps.as_ref()).await;
        if report.issues.iter().any(|issue| {
            matches!(
                issue.code,
                "UNSAFE_MEDIA_PATH"
                    | "UNSUPPORTED_HASH"
                    | "PROBE_UNAVAILABLE"
                    | "DECODE_UNAVAILABLE"
                    | "INVALID_TIMEOUT"
            )
        }) {
            result.preflight_conformance = Conformance::Rejected;
        } else if !report.issues.is_empty() && result.preflight_conformance != Conformance::Rejected
        {
            result.preflight_conformance = Conformance::Degraded;
        }
        result.assets.push(report);
    }
    result.cancelled = options.cancellation.is_cancelled();
    result.full_decode_verification = match options.decode {
        DecodePolicy::Skip => "not_performed",
        DecodePolicy::Full => {
            let checks: Vec<_> = result
                .assets
                .iter()
                .map(|asset| asset.full_decode)
                .collect();
            if checks.contains(&Check::Failed) {
                "failed"
            } else if checks.contains(&Check::Unverifiable) {
                "unverifiable"
            } else if checks.len() < package.registry.assets.len()
                || checks.contains(&Check::NotPerformed)
            {
                "not_performed"
            } else {
                "passed"
            }
        }
    };
    result.time_map_media_bounds_verification =
        report::aggregate_bounds(result.assets.iter().map(|asset| asset.time_map_bounds));
    result.continuous_packet_coverage_verification =
        report::aggregate_bounds(result.assets.iter().map(|asset| asset.packet_coverage));
    if result.assets.len() < package.registry.assets.len()
        && package
            .time_maps
            .as_ref()
            .is_some_and(|maps| !maps.segments.is_empty())
    {
        result.time_map_media_bounds_verification = report::aggregate_bounds(
            [
                result.time_map_media_bounds_verification,
                Check::NotPerformed,
            ]
            .into_iter(),
        );
        result.continuous_packet_coverage_verification = report::aggregate_bounds(
            [
                result.continuous_packet_coverage_verification,
                Check::NotPerformed,
            ]
            .into_iter(),
        );
    }
    if result.cancelled && result.preflight_conformance == Conformance::Conformant {
        result.preflight_conformance = Conformance::Degraded;
    }
    result
}
