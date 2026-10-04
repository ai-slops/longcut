#![cfg(unix)]

use longcut_session_contract::Conformance;
use longcut_session_media::{Options, verify};
use std::{fs, os::unix::fs::PermissionsExt, process::Command, time::Duration};
mod common;
use common::{TestResult, prepare};

#[tokio::test]
async fn cancellation_waits_for_running_probe_to_be_reaped() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let ffprobe = root.path().join("probe-stub");
    let ready = root.path().join("probe-stub.ready");
    assert!(Command::new("mkfifo").arg(&ready).status()?.success());
    fs::write(
        &ffprobe,
        "#!/bin/sh\nprintf '%s' $$ > \"$0.pid\"\nprintf ready > \"$0.ready\"\nexec sleep 30\n",
    )?;
    fs::set_permissions(&ffprobe, fs::Permissions::from_mode(0o700))?;
    let options = Options {
        ffprobe,
        ..Options::default()
    };
    // When
    let (report, notified) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(verify(root.path(), &options), async {
            let signal = tokio::fs::read(&ready).await;
            options.cancellation.cancel();
            signal
        })
    })
    .await?;
    // Then
    assert_eq!(notified?, b"ready");
    assert!(report.cancelled);
    assert_eq!(report.preflight_conformance, Conformance::Degraded);
    let pid = fs::read_to_string(root.path().join("probe-stub.pid"))?;
    assert!(
        !Command::new("kill")
            .args(["-0", &pid])
            .output()?
            .status
            .success()
    );
    Ok(())
}
