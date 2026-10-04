use longcut_server::{AppState, model::*};
use longcut_session_contract::Conformance;
use std::path::Path;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn prepared(job: &SessionImportJob, artifacts: Vec<String>) -> TestResultValue<PreparedImport> {
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/edit-session-packages/complete");
    let (package, _) = longcut_session_contract::load(&fixture)
        .map_err(|_| "fixture metadata")?
        .into_parts();
    Ok(PreparedImport {
        record: SourceSessionRecord {
            summary: SourceSessionSummary {
                id: job.id.clone(),
                session_id: package.manifest.session_id.clone(),
                title: "Recovered session".into(),
                duration_ns: "10000000000".into(),
                conformance: Conformance::Conformant,
                imported_at: job.created_at.clone(),
                assets: vec![],
                sources: vec![],
                warnings: vec![],
                verification_url: None,
            },
            package,
            mappings: vec![],
        },
        sources: vec![],
        artifacts,
    })
}

type TestResultValue<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::test]
async fn interrupted_publishing_is_failed_and_all_partial_files_are_removed_on_restart()
-> TestResult {
    // Given
    let data = tempfile::tempdir()?;
    let state = AppState::initialize(data.path()).await?;
    let project = state
        .store
        .create(CreateProject {
            title: "recovery".into(),
            frame_rate: 30,
        })
        .await?;
    let job = state.store.create_import(&project.id).await?;
    let prepared = prepared(&job, vec!["partially-published.bin".into()])?;
    state.store.prepare_import(&job, &prepared).await?;
    state
        .store
        .transition_import(&project.id, &job.id, ImportStatus::Publishing, None)
        .await?;
    let published_package = data.path().join("source-sessions").join(&job.id);
    let staging = data.path().join("session-imports").join(&job.id);
    tokio::fs::create_dir_all(&published_package).await?;
    tokio::fs::create_dir_all(staging.join("work")).await?;
    tokio::fs::write(published_package.join("manifest.json"), b"partial").await?;
    tokio::fs::write(staging.join("work/unpublished.bin"), b"partial").await?;
    tokio::fs::write(
        data.path().join("artifacts/partially-published.bin"),
        b"partial",
    )
    .await?;
    drop(state);

    // When
    let reopened = AppState::initialize(data.path()).await?;

    // Then
    let recovered = reopened.store.get_import(&project.id, &job.id).await?;
    assert_eq!(recovered.status, ImportStatus::Failed);
    assert!(
        recovered
            .error
            .as_deref()
            .is_some_and(|error| error.contains("interrupted"))
    );
    assert!(!published_package.exists());
    assert!(!staging.exists());
    assert!(
        !data
            .path()
            .join("artifacts/partially-published.bin")
            .exists()
    );
    assert_eq!(reopened.store.get(&project.id).await?.revision, "0");
    Ok(())
}

#[tokio::test]
async fn prepared_import_and_staging_files_survive_restart_for_later_confirmation() -> TestResult {
    // Given
    let data = tempfile::tempdir()?;
    let state = AppState::initialize(data.path()).await?;
    let project = state
        .store
        .create(CreateProject {
            title: "prepared".into(),
            frame_rate: 30,
        })
        .await?;
    let job = state.store.create_import(&project.id).await?;
    let prepared = prepared(&job, vec!["ready.bin".into()])?;
    state.store.prepare_import(&job, &prepared).await?;
    let staging = data.path().join("session-imports").join(&job.id);
    tokio::fs::create_dir_all(staging.join("package")).await?;
    tokio::fs::create_dir_all(staging.join("work")).await?;
    tokio::fs::write(staging.join("package/manifest.json"), b"ready").await?;
    tokio::fs::write(staging.join("work/ready.bin"), b"ready").await?;
    drop(state);

    // When
    let reopened = AppState::initialize(data.path()).await?;

    // Then
    assert_eq!(
        reopened
            .store
            .get_import(&project.id, &job.id)
            .await?
            .status,
        ImportStatus::Prepared
    );
    assert_eq!(
        reopened
            .store
            .prepared_import(&project.id, &job.id)
            .await?
            .artifacts,
        vec!["ready.bin"]
    );
    assert!(staging.join("package/manifest.json").is_file());
    assert!(staging.join("work/ready.bin").is_file());
    Ok(())
}
