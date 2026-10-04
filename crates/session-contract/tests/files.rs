use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use longcut_session_contract::{Conformance, inspect, schema::schemas};
use serde::Deserialize;
use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/edit-session-packages")
}

fn copy_complete(root: &Path) -> std::io::Result<()> {
    for name in [
        "manifest",
        "sources",
        "time-maps",
        "visual-composition",
        "audio-mix",
        "media-health",
    ] {
        fs::copy(
            fixtures().join("complete").join(format!("{name}.json")),
            root.join(format!("{name}.json")),
        )?;
    }
    Ok(())
}

#[derive(Deserialize)]
struct Case {
    name: String,
    file: String,
    pointer: String,
    value: Value,
    expected: String,
    code: Option<String>,
}

#[test]
fn fixture_mutations_distinguish_conformance_boundaries() -> TestResult {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../fixtures/edit-session-packages/cases.json"
    ))?;
    for case in cases {
        // Given
        let directory = tempfile::tempdir()?;
        copy_complete(directory.path())?;
        let file = directory.path().join(&case.file);
        let mut value: Value = serde_json::from_slice(&fs::read(&file)?)?;
        *value
            .pointer_mut(&case.pointer)
            .ok_or("invalid fixture pointer")? = case.value;
        fs::write(file, serde_json::to_vec(&value)?)?;
        // When
        let report = inspect(directory.path());
        // Then
        assert_eq!(
            serde_json::to_value(report.metadata_conformance)?,
            case.expected,
            "{}: {:?}",
            case.name,
            report.diagnostics
        );
        if let Some(code) = case.code {
            assert!(
                report.diagnostics.iter().any(|item| item.code == code),
                "{}: {:?}",
                case.name,
                report.diagnostics
            );
        }
    }
    Ok(())
}

#[test]
fn schema_generator_validates_all_six_complete_files() -> TestResult {
    // Given
    let generated = schemas();
    assert_eq!(generated.len(), 6);
    for (name, schema) in generated {
        let document: Value = serde_json::from_slice(&fs::read(
            fixtures().join("complete").join(format!("{name}.json")),
        )?)?;
        // When
        let validator = jsonschema::validator_for(&serde_json::to_value(schema)?)?;
        // Then
        assert!(validator.is_valid(&document), "schema rejected {name}");
        let mut extra = document;
        extra["producer_private_state"] = Value::Bool(true);
        assert!(
            !validator.is_valid(&extra),
            "schema allowed an unnamespaced core field"
        );
    }
    Ok(())
}

#[test]
fn published_schemas_match_generated_contract() -> TestResult {
    // Given
    let published: Value = serde_json::from_str(include_str!(
        "../../../schemas/edit-session-package/schema-bundle.json"
    ))?;
    // When
    let generated = serde_json::to_value(schemas())?;
    // Then
    assert_eq!(generated, published);
    Ok(())
}

#[test]
fn missing_optional_file_degrades_but_malformed_file_rejects() -> TestResult {
    // Given
    let directory = tempfile::tempdir()?;
    copy_complete(directory.path())?;
    fs::remove_file(directory.path().join("audio-mix.json"))?;
    // When / Then
    assert_eq!(
        inspect(directory.path()).metadata_conformance,
        Conformance::Degraded
    );
    fs::write(directory.path().join("audio-mix.json"), b"{bad-json")?;
    assert_eq!(
        inspect(directory.path()).metadata_conformance,
        Conformance::Rejected
    );
    Ok(())
}

#[test]
fn program_only_fixture_remains_openable_without_reconstruction() {
    // Given / When
    let report = inspect(&fixtures().join("program-only"));
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Degraded);
    assert!(report.capabilities.program_reference);
    assert!(!report.capabilities.semantic_visual_reconstruction);
    assert!(!report.capabilities.semantic_audio_reconstruction);
}

#[test]
fn cli_exposes_real_reports_help_and_schemas() -> TestResult {
    // Given
    let executable = env!("CARGO_BIN_EXE_session-inspect");
    // When / Then
    let output = Command::new(executable)
        .args(["inspect"])
        .arg(fixtures().join("complete"))
        .output()?;
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["metadata_conformance"], "conformant");
    assert_eq!(report["media_verification"], "not_performed");
    assert_eq!(
        Command::new(executable)
            .arg("inspect")
            .arg(fixtures().join("program-only"))
            .output()?
            .status
            .code(),
        Some(1)
    );
    assert_eq!(
        Command::new(executable)
            .arg("inspect")
            .arg(fixtures().join("absent"))
            .output()?
            .status
            .code(),
        Some(2)
    );
    assert!(
        Command::new(executable)
            .arg("--help")
            .output()?
            .status
            .success()
    );
    let output = Command::new(executable).arg("schema").output()?;
    assert!(output.status.success());
    let schemas: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        schemas
            .as_object()
            .ok_or("schema bundle object required")?
            .len(),
        6
    );
    Ok(())
}

#[test]
fn oversized_metadata_is_rejected_before_parsing() -> TestResult {
    // Given
    let directory = tempfile::tempdir()?;
    fs::File::create(directory.path().join("manifest.json"))?.set_len(4 * 1024 * 1024 + 1)?;
    // When
    let report = inspect(directory.path());
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Rejected);
    assert!(
        report
            .diagnostics
            .iter()
            .any(|item| item.code == "INVALID_MANIFEST")
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinked_metadata_is_rejected() -> TestResult {
    // Given
    let directory = tempfile::tempdir()?;
    std::os::unix::fs::symlink(
        fixtures().join("complete/manifest.json"),
        directory.path().join("manifest.json"),
    )?;
    // When
    let report = inspect(directory.path());
    // Then
    assert_eq!(report.metadata_conformance, Conformance::Rejected);
    Ok(())
}
