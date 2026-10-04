#![cfg(unix)]

use longcut_session_contract::Conformance;
use longcut_session_media::{Options, verify};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::Duration,
};
mod common;
use common::{TestResult, prepare};

fn executable(root: &Path, script: &str) -> Result<PathBuf, std::io::Error> {
    let path = root.join("probe-stub");
    fs::write(&path, script)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    Ok(path)
}

#[tokio::test]
async fn timed_out_probe_is_killed_and_reaped() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let ffprobe = executable(
        root.path(),
        "#!/bin/sh\nprintf '%s\\n' $$ >> \"$0.pid\"\nexec sleep 30\n",
    )?;
    let options = Options {
        ffprobe,
        probe_timeout: Duration::from_millis(500),
        ..Options::default()
    };
    // When
    let report = verify(root.path(), &options).await;
    // Then
    assert_eq!(report.preflight_conformance, Conformance::Degraded);
    assert!(
        report.assets.iter().all(|asset| asset
            .issues
            .iter()
            .any(|issue| issue.code == "PROBE_TIMEOUT")),
        "{report:?}"
    );
    let pids = fs::read_to_string(root.path().join("probe-stub.pid"))?;
    assert_eq!(pids.lines().count(), 2);
    for pid in pids.lines() {
        assert!(
            !std::process::Command::new("kill")
                .args(["-0", pid])
                .output()?
                .status
                .success(),
            "timed-out process {pid} still exists"
        );
    }
    Ok(())
}

#[tokio::test]
async fn excessive_stdout_and_stderr_are_bounded() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    for script in [
        "#!/bin/sh\nexec head -c 1048577 /dev/zero\n",
        "#!/bin/sh\nexec head -c 65537 /dev/zero >&2\n",
    ] {
        let ffprobe = executable(root.path(), script)?;
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
        assert!(
            report.assets.iter().all(|asset| asset
                .issues
                .iter()
                .any(|issue| issue.code == "PROBE_OUTPUT_LIMIT")),
            "{report:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn missing_probe_tool_is_an_explicit_environment_failure() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    // When
    let report = verify(
        root.path(),
        &Options {
            ffprobe: root.path().join("absent-probe"),
            ..Options::default()
        },
    )
    .await;
    // Then
    assert_eq!(report.preflight_conformance, Conformance::Rejected);
    assert!(report.assets.iter().all(|asset| {
        asset
            .issues
            .iter()
            .any(|issue| issue.code == "PROBE_UNAVAILABLE")
    }));
    Ok(())
}

#[tokio::test]
async fn asset_deadline_also_waits_for_probe_reaping() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let ffprobe = executable(
        root.path(),
        "#!/bin/sh\nprintf '%s\\n' $$ >> \"$0.pid\"\nexec sleep 30\n",
    )?;
    let options = Options {
        ffprobe,
        asset_timeout: Duration::from_millis(500),
        ..Options::default()
    };
    // When
    let report = verify(root.path(), &options).await;
    // Then
    assert!(
        report.assets.iter().all(|asset| asset
            .issues
            .iter()
            .any(|issue| issue.code == "ASSET_TIMEOUT")),
        "{report:?}"
    );
    let pids = fs::read_to_string(root.path().join("probe-stub.pid"))?;
    assert_eq!(pids.lines().count(), 2);
    for pid in pids.lines() {
        assert!(
            !std::process::Command::new("kill")
                .args(["-0", pid])
                .output()?
                .status
                .success(),
            "asset-timed-out process {pid} still exists"
        );
    }
    Ok(())
}
