use std::fs;

use longcut_session_contract::{
    Conformance,
    model::{Integer, Registry, TimeMaps},
};
use longcut_session_media::{Options, verify};
mod common;
use common::{TestResult, prepare};

#[tokio::test]
async fn real_packet_envelopes_verify_declared_ranges() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    // When
    let report = verify(root.path(), &Options::default()).await;
    let value = serde_json::to_value(&report)?;
    // Then
    assert_eq!(
        report.preflight_conformance,
        Conformance::Conformant,
        "{report:?}"
    );
    assert_eq!(value["time_map_media_bounds_verification"], "passed");
    assert_eq!(value["time_map_bounds_method"], "packet_pts_envelope");
    assert_eq!(
        value["assets"][0]["time_map_segments"][0]["check"],
        "passed"
    );
    Ok(())
}

#[tokio::test]
async fn endpoint_half_a_nanosecond_beyond_media_is_rejected_without_rounding() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let path = root.path().join("time-maps.json");
    let mut maps: TimeMaps = serde_json::from_slice(&fs::read(&path)?)?;
    maps.segments[0].media_range.end.ticks = Integer::new(20_000_000_001);
    maps.segments[0].media_range.end.timebase_den = 2_000_000_000;
    fs::write(path, serde_json::to_vec(&maps)?)?;
    // When
    let report = verify(root.path(), &Options::default()).await;
    // Then
    assert_eq!(
        report.metadata.metadata_conformance,
        Conformance::Conformant
    );
    assert_eq!(report.preflight_conformance, Conformance::Degraded);
    assert!(
        report.assets[0]
            .issues
            .iter()
            .any(|issue| issue.code == "TIME_MAP_RANGE_OUT_OF_BOUNDS"),
        "{report:?}"
    );
    Ok(())
}

#[tokio::test]
async fn negative_audio_priming_pts_are_retained_as_packet_evidence() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let path = root.path().join("time-maps.json");
    let mut maps: TimeMaps = serde_json::from_slice(&fs::read(&path)?)?;
    maps.segments[1].media_range.start.ticks = Integer::new(-1024);
    fs::write(path, serde_json::to_vec(&maps)?)?;
    // When
    let report = verify(root.path(), &Options::default()).await;
    let value = serde_json::to_value(&report)?;
    // Then
    assert_eq!(
        report.preflight_conformance,
        Conformance::Conformant,
        "{report:?}"
    );
    assert_eq!(value["assets"][0]["packet_bounds"][1]["start_pts"], "-1024");
    assert_eq!(
        value["assets"][0]["time_map_segments"][1]["check"],
        "passed"
    );
    Ok(())
}

#[tokio::test]
async fn positive_start_pts_rejects_a_zero_based_source_range() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let output = tokio::process::Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-itsoffset", "2", "-i"])
        .arg(root.path().join("media/program.mp4"))
        .args(["-map", "0", "-c", "copy", "-avoid_negative_ts", "disabled"])
        .arg(root.path().join("media/shifted.mp4"))
        .output()
        .await?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = root.path().join("sources.json");
    let mut registry: Registry = serde_json::from_slice(&fs::read(&path)?)?;
    registry.assets[0].uri = "media/shifted.mp4".into();
    registry.assets[0].size_bytes = None;
    registry.assets[0].content_hash = None;
    fs::write(path, serde_json::to_vec(&registry)?)?;
    // When
    let report = verify(root.path(), &Options::default()).await;
    // Then
    assert_eq!(
        report.preflight_conformance,
        Conformance::Degraded,
        "{report:?}"
    );
    assert!(
        report.assets[0]
            .issues
            .iter()
            .any(|issue| issue.code == "TIME_MAP_RANGE_OUT_OF_BOUNDS")
    );
    Ok(())
}

#[tokio::test]
async fn absent_time_maps_remain_not_declared_and_do_not_scan_packets() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let path = root.path().join("manifest.json");
    let mut manifest: longcut_session_contract::model::Manifest =
        serde_json::from_slice(&fs::read(&path)?)?;
    manifest.time_maps_file = None;
    fs::write(path, serde_json::to_vec(&manifest)?)?;
    // When
    let report = verify(root.path(), &Options::default()).await;
    // Then
    assert_eq!(
        report.time_map_media_bounds_verification,
        longcut_session_media::Check::NotDeclared
    );
    assert!(
        report
            .assets
            .iter()
            .all(|asset| asset.packet_bounds.is_empty())
    );
    Ok(())
}
