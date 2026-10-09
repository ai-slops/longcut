use std::process::Stdio;

use longcut_session_contract::model::{Integer, MediaTime};
use tokio::process::Command;

use super::{AttemptWorkspace, verify_source_start};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn real_zero_pts_subrange_is_rejected_before_remux() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let media = root.path().join("twenty-seconds.mp4");
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x90:rate=30",
            "-t",
            "20",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-threads",
            "1",
            "-an",
            "-y",
        ])
        .arg(&media)
        .stderr(Stdio::piped())
        .output()
        .await?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let subrange_start = MediaTime {
        ticks: Integer::new(60),
        timebase_num: 1,
        timebase_den: 30,
    };

    // When
    let error = verify_source_start(&media, 0, &subrange_start)
        .await
        .expect_err("subrange must not be shifted without cropping");

    // Then
    assert_eq!(
        error.to_string(),
        "invalid input: mapped video range must start at the first packet PTS"
    );
    Ok(())
}

#[tokio::test]
async fn failed_attempt_removes_all_owned_artifacts() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let attempt = AttemptWorkspace::create(root.path()).await?;
    tokio::fs::write(attempt.path().join("remux.mp4"), b"partial").await?;
    tokio::fs::write(attempt.path().join("proxy.mp4"), b"partial").await?;

    // When
    attempt.cleanup().await?;

    // Then
    assert_eq!(std::fs::read_dir(root.path())?.count(), 0);
    Ok(())
}
