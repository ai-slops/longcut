use longcut_session_contract::Conformance;
use longcut_session_media::{Options, verify};
use std::{fs, path::Path};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[tokio::test]
async fn missing_actual_media_degrades_valid_metadata() -> TestResult {
    // Given
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/edit-session-packages/complete");
    // When
    let report = verify(&root, &Options::default()).await;
    // Then
    assert_eq!(
        report.metadata.metadata_conformance,
        Conformance::Conformant
    );
    assert_eq!(report.preflight_conformance, Conformance::Degraded);
    assert_eq!(report.assets.len(), 2);
    assert!(report.assets.iter().all(|asset| {
        asset
            .issues
            .iter()
            .any(|issue| issue.code == "MEDIA_MISSING")
    }));
    Ok(())
}

#[tokio::test]
async fn rejected_metadata_never_reaches_media_tools() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    fs::write(root.path().join("manifest.json"), b"{}")?;
    let options = Options {
        ffprobe: "/does-not-exist".into(),
        ..Options::default()
    };
    // When
    let report = verify(root.path(), &options).await;
    // Then
    assert_eq!(report.preflight_conformance, Conformance::Rejected);
    assert!(report.assets.is_empty());
    Ok(())
}
