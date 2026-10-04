use longcut_session_contract::model::Registry;
use longcut_session_media::{Check, DecodePolicy, Options, verify};
use std::fs;
mod common;
use common::{TestResult, prepare};

#[tokio::test]
async fn normal_b_frame_reordering_has_verified_gap_free_coverage() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let output = tokio::process::Command::new("ffmpeg")
        .args(["-v", "error", "-nostdin", "-i"])
        .arg(root.path().join("media/program.mp4"))
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "medium",
            "-bf",
            "3",
            "-threads",
            "1",
            "-c:a",
            "copy",
            "-map_metadata",
            "-1",
        ])
        .arg(root.path().join("media/reordered.mp4"))
        .output()
        .await?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = root.path().join("sources.json");
    let mut registry: Registry = serde_json::from_slice(&fs::read(&path)?)?;
    registry.assets[0].uri = "media/reordered.mp4".into();
    registry.assets[0].size_bytes = None;
    registry.assets[0].content_hash = None;
    fs::write(path, serde_json::to_vec(&registry)?)?;
    // When
    let report = verify(
        root.path(),
        &Options {
            decode: DecodePolicy::Full,
            ..Options::default()
        },
    )
    .await;
    // Then
    assert_eq!(report.assets[0].full_decode, Check::Passed, "{report:?}");
    assert_eq!(
        report.assets[0].packet_coverage,
        Check::Passed,
        "{report:?}"
    );
    assert!(!report.assets[0].packet_bounds[0].pts_ordered);
    Ok(())
}
