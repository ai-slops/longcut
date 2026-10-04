use std::fs;

use longcut_session_contract::{Conformance, model::Integer};
use longcut_session_media::{Check, Options, verify};
mod common;
use common::{TestResult, prepare};

fn mutate(
    root: &std::path::Path,
    change: impl FnOnce(&mut longcut_session_contract::model::Registry),
) -> TestResult {
    let mut registry = serde_json::from_slice(&fs::read(root.join("sources.json"))?)?;
    change(&mut registry);
    fs::write(root.join("sources.json"), serde_json::to_vec(&registry)?)?;
    Ok(())
}

#[tokio::test]
async fn real_media_matches_size_checksum_and_stream_declarations() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let original = fs::read(root.path().join("media/program.mp4"))?;
    // When
    let report = verify(root.path(), &Options::default()).await;
    // Then
    assert_eq!(
        report.preflight_conformance,
        Conformance::Conformant,
        "{report:?}"
    );
    assert!(report.assets.iter().all(|asset| asset.file == Check::Passed
        && asset.size == Check::Passed
        && asset.checksum == Check::Passed
        && asset.probe == Check::Passed
        && asset.stream_metadata == Check::Passed));
    assert_eq!(report.full_decode_verification, "not_performed");
    assert_eq!(report.alignment_verification, "not_performed");
    assert_eq!(fs::read(root.path().join("media/program.mp4"))?, original);
    Ok(())
}

#[tokio::test]
async fn mismatched_size_or_hash_stops_before_probing() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    mutate(root.path(), |registry| {
        registry.assets[0].size_bytes = Some(Integer::new(1));
        registry.assets[1].content_hash = Some(format!("sha256:{}", "0".repeat(64)));
    })?;
    // When
    let report = verify(
        root.path(),
        &Options {
            ffprobe: "/absent-tool".into(),
            ..Options::default()
        },
    )
    .await;
    // Then
    assert_eq!(report.preflight_conformance, Conformance::Degraded);
    assert!(
        report.assets[0]
            .issues
            .iter()
            .any(|issue| issue.code == "SIZE_MISMATCH")
    );
    assert!(
        report.assets[1]
            .issues
            .iter()
            .any(|issue| issue.code == "CHECKSUM_MISMATCH")
    );
    assert!(
        report
            .assets
            .iter()
            .all(|asset| asset.probe == Check::NotPerformed)
    );
    Ok(())
}

#[tokio::test]
async fn corrupt_bytes_fail_probe_when_size_and_hash_are_not_declared() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    mutate(root.path(), |registry| {
        registry.assets[0].size_bytes = None;
        registry.assets[0].content_hash = None;
    })?;
    fs::write(
        root.path().join("media/program.mp4"),
        b"this is not a video",
    )?;
    // When
    let report = verify(root.path(), &Options::default()).await;
    // Then
    assert_eq!(report.preflight_conformance, Conformance::Degraded);
    assert!(
        report.assets[0]
            .issues
            .iter()
            .any(|issue| issue.code == "PROBE_FAILED")
    );
    assert_eq!(report.assets[1].probe, Check::Passed);
    Ok(())
}

#[tokio::test]
async fn actual_stream_metadata_mismatch_is_not_treated_as_healthy() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    mutate(root.path(), |registry| {
        registry.assets[0].streams[0].width = Some(1920)
    })?;
    // When
    let report = verify(root.path(), &Options::default()).await;
    // Then
    assert_eq!(report.preflight_conformance, Conformance::Degraded);
    assert!(
        report.assets[0]
            .issues
            .iter()
            .any(|issue| issue.code == "STREAM_METADATA_MISMATCH")
    );
    Ok(())
}

#[cfg(unix)]
#[tokio::test]
async fn media_symlink_is_rejected_without_reading_target() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let original = root.path().join("media/program.mp4");
    let elsewhere = root.path().join("external.mp4");
    fs::rename(&original, &elsewhere)?;
    std::os::unix::fs::symlink(&elsewhere, &original)?;
    // When
    let report = verify(root.path(), &Options::default()).await;
    // Then
    assert_eq!(report.preflight_conformance, Conformance::Rejected);
    assert!(
        report.assets[0]
            .issues
            .iter()
            .any(|issue| issue.code == "UNSAFE_MEDIA_PATH")
    );
    assert_eq!(report.assets[0].actual_sha256, None);
    Ok(())
}

#[tokio::test]
async fn unsupported_hash_algorithm_rejects_instead_of_silently_skipping() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    mutate(root.path(), |registry| {
        registry.assets[0].content_hash = Some("md5:ignored".into())
    })?;
    // When
    let report = verify(root.path(), &Options::default()).await;
    // Then
    assert_eq!(report.preflight_conformance, Conformance::Rejected);
    assert!(
        report.assets[0]
            .issues
            .iter()
            .any(|issue| issue.code == "UNSUPPORTED_HASH")
    );
    Ok(())
}

#[tokio::test]
async fn playlist_containers_cannot_reference_secondary_inputs() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    mutate(root.path(), |registry| {
        registry.assets[0].size_bytes = None;
        registry.assets[0].content_hash = None;
    })?;
    fs::write(
        root.path().join("media/program.mp4"),
        b"ffconcat version 1.0\nfile 'clean.mp4'\n",
    )?;
    // When
    let report = verify(root.path(), &Options::default()).await;
    // Then
    assert!(
        report.assets[0]
            .issues
            .iter()
            .any(|issue| issue.code == "PROBE_FAILED"),
        "{report:?}"
    );
    Ok(())
}
