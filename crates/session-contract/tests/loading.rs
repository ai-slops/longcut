use std::path::Path;

use longcut_session_contract::{Conformance, load};

#[test]
fn loader_exposes_accepted_metadata_without_promoting_media_status()
-> Result<(), Box<dyn std::error::Error>> {
    // Given
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/edit-session-packages/complete");
    // When
    let (package, report) = load(&root).map_err(|_| "fixture must load")?.into_parts();
    // Then
    assert_eq!(package.registry.assets.len(), 2);
    assert_eq!(report.metadata_conformance, Conformance::Conformant);
    assert_eq!(report.media_verification, "not_performed");
    Ok(())
}

#[test]
fn loader_never_exposes_rejected_metadata() -> Result<(), Box<dyn std::error::Error>> {
    // Given
    let root = tempfile::tempdir()?;
    std::fs::write(root.path().join("manifest.json"), b"{}")?;
    // When
    let result = load(root.path());
    // Then
    assert!(matches!(result, Err(report) if report.metadata_conformance == Conformance::Rejected));
    Ok(())
}
