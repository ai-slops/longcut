use std::{io::ErrorKind, path::Path};

use longcut_session_contract::model::{Asset, TimeMaps};

use crate::{
    AssetVerification, Check, DecodePolicy, Options, bounds, decode, files, packets,
    probe::{self, ProbeError},
    streams,
};

pub(crate) struct Verifier<'a> {
    pub root: &'a Path,
    pub options: &'a Options,
}

impl Verifier<'_> {
    pub(crate) async fn verify(&self, asset: &Asset, maps: Option<&TimeMaps>) -> AssetVerification {
        let root = self.root;
        let options = self.options;
        let mut report = AssetVerification::new(asset);
        let segments: Vec<_> = maps
            .into_iter()
            .flat_map(|maps| maps.segments.iter().enumerate())
            .filter(|(_, segment)| segment.media_ref.asset_id == asset.asset_id)
            .collect();
        if segments.is_empty() {
            report.time_map_bounds = Check::NotDeclared;
            report.packet_coverage = Check::NotDeclared;
        }
        let Some(deadline) = tokio::time::Instant::now().checked_add(options.asset_timeout) else {
            report.issue(
                "INVALID_TIMEOUT",
                "asset timeout exceeds the supported clock range",
            );
            return report;
        };
        let expected_hash = match asset.content_hash.as_deref() {
            None => None,
            Some(value) => match value.strip_prefix("sha256:") {
                Some(hash)
                    if hash.len() == 64 && hash.bytes().all(|byte| byte.is_ascii_hexdigit()) =>
                {
                    Some(hash)
                }
                _ => {
                    report.checksum = Check::Failed;
                    report.issue(
                        "UNSUPPORTED_HASH",
                        "content_hash must use sha256:<64 hex digits>",
                    );
                    return report;
                }
            },
        };
        let integrity = match files::bounded(
            files::integrity(root, &asset.uri),
            deadline,
            &options.cancellation,
        )
        .await
        {
            Ok(integrity) => integrity,
            Err(error) => {
                report.file = Check::Failed;
                report.issue(error.code(), error.to_string());
                return report;
            }
        };
        report.file = Check::Passed;
        report.actual_size_bytes = Some(integrity.size.to_string());
        report.actual_sha256 = Some(integrity.sha256.clone());
        report.size = match asset.size_bytes {
            None => Check::NotDeclared,
            Some(size) if u64::try_from(size.get()).ok() == Some(integrity.size) => Check::Passed,
            Some(_) => {
                report.issue(
                    "SIZE_MISMATCH",
                    "actual byte count differs from declared size",
                );
                Check::Failed
            }
        };
        report.checksum = match expected_hash {
            None => Check::NotDeclared,
            Some(expected) if expected.eq_ignore_ascii_case(&integrity.sha256) => Check::Passed,
            Some(_) => {
                report.issue(
                    "CHECKSUM_MISMATCH",
                    "actual SHA-256 differs from declared checksum",
                );
                Check::Failed
            }
        };
        if !report.issues.is_empty() {
            return report;
        }
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        let probe_options = Options {
            probe_timeout: options.probe_timeout.min(remaining),
            ..options.clone()
        };
        match probe::inspect(&integrity.path, &probe_options).await {
            Ok(actual) => {
                report.probe = Check::Passed;
                report.ffprobe_version = Some(actual.program_version.version.clone());
                streams::compare(asset, &actual, &mut report);
                if report.stream_metadata == Check::Passed && !segments.is_empty() {
                    let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                    let scan_options = Options {
                        probe_timeout: options.probe_timeout.min(remaining),
                        ..options.clone()
                    };
                    match packets::inspect(&integrity.path, &actual, &scan_options).await {
                        Ok(scan) => {
                            let result = bounds::compare(&segments, &actual, &scan);
                            report.time_map_bounds = result.check;
                            report.packet_bounds = result.packets;
                            report.packet_coverage = crate::report::aggregate_bounds(
                                segments.iter().map(|(_, segment)| {
                                    report
                                        .packet_bounds
                                        .iter()
                                        .find(|packet| {
                                            packet.stream_index == segment.media_ref.stream_index
                                        })
                                        .map_or(Check::Unverifiable, |packet| packet.coverage)
                                }),
                            );
                            for packet in &report.packet_bounds {
                                match packet.coverage {
                                    Check::Failed => report.issues.push(crate::Issue {
                                        code: "PACKET_COVERAGE_GAP",
                                        message: format!("stream {} has {} ordered packet-timestamp gap(s)", packet.stream_index, packet.gap_count),
                                    }),
                                    Check::Unverifiable => report.issues.push(crate::Issue {
                                        code: "PACKET_COVERAGE_UNVERIFIABLE",
                                        message: format!("stream {} lacks complete timing within the bounded reorder window", packet.stream_index),
                                    }),
                                    Check::Passed | Check::NotPerformed | Check::NotDeclared => {}
                                }
                            }
                            report.time_map_segments = result.segments;
                            report.issues.extend(result.issues);
                        }
                        Err(error) => {
                            report.time_map_bounds = Check::Unverifiable;
                            report.packet_coverage = Check::Unverifiable;
                            report.issue("PACKET_SCAN_UNVERIFIABLE", error.to_string());
                        }
                    }
                }
            }
            Err(error) => {
                report.probe = Check::Failed;
                let code = match &error {
                    ProbeError::Timeout if remaining < options.probe_timeout => "ASSET_TIMEOUT",
                    ProbeError::Timeout => "PROBE_TIMEOUT",
                    ProbeError::Cancelled => "ASSET_CANCELLED",
                    ProbeError::OutputLimit => "PROBE_OUTPUT_LIMIT",
                    ProbeError::Invalid(_) => "PROBE_INVALID_RESPONSE",
                    ProbeError::Failed(_) => "PROBE_FAILED",
                    ProbeError::Io(error) if error.kind() == ErrorKind::NotFound => {
                        "PROBE_UNAVAILABLE"
                    }
                    ProbeError::Io(_) => "PROBE_IO_ERROR",
                };
                report.issue(code, error.to_string());
            }
        }
        if matches!(options.decode, DecodePolicy::Full) && report.probe == Check::Passed {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            match decode::inspect(&integrity.path, remaining, options).await {
                Ok(()) => report.full_decode = Check::Passed,
                Err(error) => {
                    report.full_decode = match &error {
                        decode::DecodeError::Cancelled => Check::NotPerformed,
                        decode::DecodeError::Timeout | decode::DecodeError::OutputLimit => {
                            Check::Unverifiable
                        }
                        decode::DecodeError::Io(_) | decode::DecodeError::Failed(_) => {
                            Check::Failed
                        }
                    };
                    report.issue(error.code(), error.to_string());
                }
            }
        }
        if let Err(error) = files::bounded(
            files::unchanged(&integrity),
            deadline,
            &options.cancellation,
        )
        .await
        {
            report.file = Check::Failed;
            if !segments.is_empty() {
                report.time_map_bounds = Check::Unverifiable;
                report.packet_coverage = Check::Unverifiable;
                for segment in &mut report.time_map_segments {
                    segment.check = Check::Unverifiable;
                }
            }
            report.issue(error.code(), error.to_string());
        }
        report
    }
}
