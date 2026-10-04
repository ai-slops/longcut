use serde_json::Value;
use std::{fs, process::Command};
mod common;
use common::{TestResult, prepare};

#[tokio::test]
async fn cli_inspects_real_unicode_path_and_missing_asset() -> TestResult {
    // Given
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("한글 package");
    fs::create_dir(&root)?;
    prepare(&root).await?;
    let binary = env!("CARGO_BIN_EXE_session-media-inspect");
    // When / Then
    let output = Command::new(binary).arg(&root).output()?;
    assert_eq!(output.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["preflight_conformance"], "conformant");
    assert_eq!(report["assets"][0]["checksum"], "passed");
    assert_eq!(report["full_decode_verification"], "not_performed");
    fs::remove_file(root.join("media/program.mp4"))?;
    let output = Command::new(binary).arg(&root).output()?;
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["assets"][0]["issues"][0]["code"], "MEDIA_MISSING");
    Ok(())
}

#[tokio::test]
async fn cli_full_decode_option_runs_decoder_and_reports_result() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    prepare(root.path()).await?;
    let binary = env!("CARGO_BIN_EXE_session-media-inspect");

    // When
    let output = Command::new(binary)
        .args(["--full-decode".as_ref(), root.path().as_os_str()])
        .output()?;

    // Then
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["full_decode_verification"], "passed");
    assert_eq!(report["assets"][0]["full_decode"], "passed");
    assert_eq!(report["alignment_verification"], "not_performed");
    Ok(())
}

#[test]
fn cli_help_and_invalid_path_have_explicit_exit_status() -> TestResult {
    // Given
    let binary = env!("CARGO_BIN_EXE_session-media-inspect");
    let directory = tempfile::tempdir()?;
    // When / Then
    assert!(
        Command::new(binary)
            .arg("--help")
            .output()?
            .status
            .success()
    );
    assert_eq!(
        Command::new(binary)
            .arg(directory.path().join("absent"))
            .output()?
            .status
            .code(),
        Some(2)
    );
    assert_eq!(Command::new(binary).output()?.status.code(), Some(2));
    Ok(())
}
