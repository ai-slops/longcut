#![cfg(unix)]

use longcut_session_contract::Conformance;
use longcut_session_media::{Check, DecodePolicy, Options, verify};
use std::{fs, os::unix::fs::PermissionsExt, process::Command, time::Duration};

mod common;
use common::{TestResult, prepare};

fn remove_integrity_declarations(root: &std::path::Path, asset_index: usize) -> TestResult {
    let path = root.join("sources.json");
    let mut registry: longcut_session_contract::model::Registry =
        serde_json::from_slice(&fs::read(&path)?)?;
    registry.assets[asset_index].size_bytes = None;
    registry.assets[asset_index].content_hash = None;
    fs::write(path, serde_json::to_vec_pretty(&registry)?)?;
    Ok(())
}

#[tokio::test]
async fn full_decode_passes_for_real_valid_media() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let options = Options {
        decode: DecodePolicy::Full,
        ..Options::default()
    };

    // When
    let report = verify(root.path(), &options).await;

    // Then
    assert_eq!(
        report.preflight_conformance,
        Conformance::Conformant,
        "{report:?}"
    );
    assert_eq!(report.full_decode_verification, "passed");
    assert!(
        report
            .assets
            .iter()
            .all(|asset| asset.full_decode == Check::Passed)
    );
    Ok(())
}

#[tokio::test]
async fn decoder_failure_is_reported_without_claiming_decode_success() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let ffmpeg = root.path().join("ffmpeg-stub");
    fs::write(
        &ffmpeg,
        "#!/bin/sh\nprintf 'synthetic decoder failure' >&2\nexit 1\n",
    )?;
    fs::set_permissions(&ffmpeg, fs::Permissions::from_mode(0o700))?;
    let options = Options {
        ffmpeg,
        decode: DecodePolicy::Full,
        ..Options::default()
    };

    // When
    let report = verify(root.path(), &options).await;

    // Then
    assert_eq!(report.preflight_conformance, Conformance::Degraded);
    assert_eq!(report.full_decode_verification, "failed");
    assert!(report.assets.iter().all(|asset| {
        asset.full_decode == Check::Failed
            && asset
                .issues
                .iter()
                .any(|issue| issue.code == "DECODE_FAILED")
    }));
    Ok(())
}

#[tokio::test]
async fn full_decode_detects_corrupt_encoded_payload_that_headers_can_describe() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    remove_integrity_declarations(root.path(), 0)?;
    let media = root.path().join("media/program.mp4");
    let mut bytes = fs::read(&media)?;
    let atom_kind = bytes
        .windows(4)
        .position(|window| window == b"mdat")
        .ok_or("generated MP4 has no mdat payload")?;
    let atom_start = atom_kind
        .checked_sub(4)
        .ok_or("generated MP4 has an invalid mdat atom")?;
    let atom_size = usize::try_from(u32::from_be_bytes(bytes[atom_start..atom_kind].try_into()?))?;
    let payload = atom_kind + 4;
    let end = atom_start
        .checked_add(atom_size)
        .filter(|end| *end <= bytes.len())
        .ok_or("generated MP4 has an invalid mdat size")?;
    if payload >= end {
        return Err("generated MP4 mdat payload is unexpectedly short".into());
    }
    let corruption_start = payload + (end - payload) / 3;
    let corruption_end = corruption_start.saturating_add(8192).min(end);
    bytes[corruption_start..corruption_end].fill(0);
    fs::write(media, bytes)?;
    let options = Options {
        decode: DecodePolicy::Full,
        ..Options::default()
    };

    // When
    let report = verify(root.path(), &options).await;

    // Then
    assert_eq!(report.assets[0].probe, Check::Passed, "{report:?}");
    assert_eq!(report.assets[0].full_decode, Check::Failed, "{report:?}");
    assert!(
        report.assets[0]
            .issues
            .iter()
            .any(|issue| issue.code == "DECODE_FAILED")
    );
    assert_eq!(report.assets[1].full_decode, Check::Passed);
    Ok(())
}

#[tokio::test]
async fn cancellation_waits_for_running_decoder_to_be_reaped() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let ffmpeg = root.path().join("ffmpeg-stub");
    let ready = root.path().join("ffmpeg-stub.ready");
    assert!(Command::new("mkfifo").arg(&ready).status()?.success());
    fs::write(
        &ffmpeg,
        "#!/bin/sh\nprintf '%s' $$ > \"$0.pid\"\nprintf ready > \"$0.ready\"\nexec sleep 30\n",
    )?;
    fs::set_permissions(&ffmpeg, fs::Permissions::from_mode(0o700))?;
    let options = Options {
        ffmpeg,
        decode: DecodePolicy::Full,
        ..Options::default()
    };

    // When
    let (report, notified) = tokio::time::timeout(Duration::from_secs(10), async {
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
    assert_eq!(report.full_decode_verification, "not_performed");
    let pid = fs::read_to_string(root.path().join("ffmpeg-stub.pid"))?;
    assert!(
        !Command::new("kill")
            .args(["-0", &pid])
            .output()?
            .status
            .success()
    );
    Ok(())
}
