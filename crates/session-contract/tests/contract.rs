use longcut_session_contract::{Conformance, model::*, validate};
mod common;
use common::fixture;

#[test]
fn complete_metadata_is_conformant_but_not_media_verified() -> Result<(), serde_json::Error> {
    // Given
    let package = fixture()?;
    // When
    let report = validate(&package);
    // Then
    assert_eq!(
        report.metadata_conformance,
        Conformance::Conformant,
        "{:?}",
        report.diagnostics
    );
    assert_eq!(report.media_verification, "not_performed");
    assert!(report.capabilities.semantic_visual_reconstruction);
    assert!(report.capabilities.semantic_audio_reconstruction);
    Ok(())
}

#[test]
fn missing_mix_degrades_without_disabling_visual_metadata() -> Result<(), serde_json::Error> {
    // Given
    let mut package = fixture()?;
    package.audio_mix = None;
    // When
    let report = validate(&package);
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Degraded);
    assert!(report.capabilities.semantic_visual_reconstruction);
    assert!(!report.capabilities.semantic_audio_reconstruction);
    Ok(())
}

#[test]
fn unresolved_binding_rejects_and_clears_capabilities() -> Result<(), serde_json::Error> {
    // Given
    let mut package = fixture()?;
    package.registry.sources[0].media_bindings[0].asset_id = AssetId("absent".into());
    // When
    let report = validate(&package);
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Rejected);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|item| item.code == "UNKNOWN_STREAM")
    );
    assert!(!report.capabilities.program_reference);
    Ok(())
}
