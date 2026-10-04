#![cfg(unix)]

use longcut_session_contract::Conformance;
use longcut_session_media::{Check, Options, verify};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, time::Duration};
mod common;
use common::{TestResult, prepare};

fn scanner(root: &std::path::Path, body: &str) -> Result<PathBuf, std::io::Error> {
    let path = root.join("probe-wrapper");
    fs::write(
        &path,
        format!(
            "#!/bin/sh\ncase \" $* \" in\n*\" -show_packets \"*) {body} ;;\n*) exec ffprobe \"$@\" ;;\nesac\n"
        ),
    )?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    Ok(path)
}

#[tokio::test]
async fn timed_out_packet_scan_is_unverifiable_and_reaped() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let ffprobe = scanner(
        root.path(),
        "printf '%s\\n' $$ >> \"$0.pid\"; exec sleep 30",
    )?;
    // When
    let report = verify(
        root.path(),
        &Options {
            ffprobe,
            probe_timeout: Duration::from_millis(500),
            ..Options::default()
        },
    )
    .await;
    // Then
    assert_eq!(report.preflight_conformance, Conformance::Degraded);
    assert_eq!(
        report.time_map_media_bounds_verification,
        Check::Unverifiable
    );
    assert!(
        report
            .assets
            .iter()
            .all(|asset| asset.probe == Check::Passed
                && asset.time_map_bounds == Check::Unverifiable),
        "{report:?}"
    );
    let pids = fs::read_to_string(root.path().join("probe-wrapper.pid"))?;
    assert_eq!(pids.lines().count(), 2);
    for pid in pids.lines() {
        assert!(
            !std::process::Command::new("kill")
                .args(["-0", pid])
                .output()?
                .status
                .success()
        );
    }
    Ok(())
}

#[tokio::test]
async fn missing_packet_duration_cannot_pass_preflight() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let ffprobe = scanner(
        root.path(),
        "printf 'stream_index=0|pts=0|duration=N/A\\nstream_index=1|pts=0|duration=480000\\n'",
    )?;
    // When
    let report = verify(
        root.path(),
        &Options {
            ffprobe,
            ..Options::default()
        },
    )
    .await;
    // Then
    assert_eq!(report.preflight_conformance, Conformance::Degraded);
    assert_eq!(
        report.time_map_media_bounds_verification,
        Check::Unverifiable
    );
    assert!(
        report.assets[0]
            .issues
            .iter()
            .any(|issue| issue.code == "TIME_MAP_BOUNDS_UNVERIFIABLE")
    );
    Ok(())
}

#[tokio::test]
async fn oversized_packet_line_is_bounded_without_buffering_the_stream() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let ffprobe = scanner(root.path(), "exec head -c 4097 /dev/zero")?;
    // When
    let report = verify(
        root.path(),
        &Options {
            ffprobe,
            ..Options::default()
        },
    )
    .await;
    // Then
    assert_eq!(
        report.time_map_media_bounds_verification,
        Check::Unverifiable
    );
    assert!(
        report.assets[0]
            .issues
            .iter()
            .any(|issue| issue.code == "PACKET_SCAN_UNVERIFIABLE")
    );
    Ok(())
}
