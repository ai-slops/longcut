use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use longcut_server::{AppState, app, model::*};
use longcut_session_contract::Conformance;
use std::path::Path;
use tower::ServiceExt;

type TestResult = Result<(), Box<dyn std::error::Error>>;

struct PreparedFixture {
    data: tempfile::TempDir,
    state: AppState,
    project: Project,
    job: SessionImportJob,
}

async fn setup_prepared() -> Result<PreparedFixture, Box<dyn std::error::Error>> {
    let data = tempfile::tempdir()?;
    let state = AppState::initialize(data.path()).await?;
    let project = state
        .store
        .create(CreateProject {
            title: "fault test".into(),
            frame_rate: 30,
        })
        .await?;
    let job = state.store.create_import(&project.id).await?;
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/edit-session-packages/complete");
    let (package, _) = longcut_session_contract::load(&fixture)
        .map_err(|_| "fixture metadata")?
        .into_parts();
    let prepared = PreparedImport {
        record: SourceSessionRecord {
            summary: SourceSessionSummary {
                id: job.id.clone(),
                session_id: package.manifest.session_id.clone(),
                title: "Fault session".into(),
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
        artifacts: vec!["session-report.json".into()],
    };
    state.store.prepare_import(&job, &prepared).await?;
    let staging = data.path().join("session-imports").join(&job.id);
    tokio::fs::create_dir_all(staging.join("package/media")).await?;
    tokio::fs::create_dir_all(staging.join("work")).await?;
    tokio::fs::write(staging.join("package/media/program.mp4"), b"original-media").await?;
    tokio::fs::write(staging.join("work/session-report.json"), b"report").await?;
    Ok(PreparedFixture {
        data,
        state,
        project,
        job,
    })
}

#[tokio::test]
async fn stale_confirmation_leaves_prepared_job_and_files_untouched() -> TestResult {
    // Given
    let fixture = setup_prepared().await?;
    fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: "0".into(),
                command: Command::SetBrief {
                    brief: "concurrent edit".into(),
                },
            },
        )
        .await?;
    let staging = fixture
        .data
        .path()
        .join("session-imports")
        .join(&fixture.job.id);
    let uri = format!(
        "/api/v1/projects/{}/session-imports/{}/confirm",
        fixture.project.id, fixture.job.id
    );

    // When
    let response = app(fixture.state.clone())
        .oneshot(
            Request::post(uri)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"base_revision":"0","allow_degraded":false}"#,
                ))?,
        )
        .await?;

    // Then
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        fixture
            .state
            .store
            .get_import(&fixture.project.id, &fixture.job.id)
            .await?
            .status,
        ImportStatus::Prepared
    );
    assert_eq!(
        fixture.state.store.get(&fixture.project.id).await?.revision,
        "1"
    );
    assert!(staging.join("package/media/program.mp4").is_file());
    assert!(staging.join("work/session-report.json").is_file());
    assert!(
        !fixture
            .data
            .path()
            .join("source-sessions")
            .join(&fixture.job.id)
            .exists()
    );
    assert!(
        !fixture
            .data
            .path()
            .join("artifacts/session-report.json")
            .exists()
    );
    Ok(())
}

#[tokio::test]
async fn original_asset_access_is_revoked_when_imported_session_is_undone() -> TestResult {
    // Given
    let fixture = setup_prepared().await?;
    let router = app(fixture.state.clone());
    let confirm_uri = format!(
        "/api/v1/projects/{}/session-imports/{}/confirm",
        fixture.project.id, fixture.job.id
    );
    let confirmed = router
        .clone()
        .oneshot(
            Request::post(confirm_uri)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"base_revision":"0","allow_degraded":false}"#,
                ))?,
        )
        .await?;
    assert_eq!(confirmed.status(), StatusCode::OK);
    let asset_uri = format!(
        "/api/v1/projects/{}/source-sessions/{}/assets/program-file",
        fixture.project.id, fixture.job.id
    );
    let active = router
        .clone()
        .oneshot(Request::get(&asset_uri).body(Body::empty())?)
        .await?;
    assert_eq!(active.status(), StatusCode::OK);
    assert_eq!(
        active.into_body().collect().await?.to_bytes(),
        "original-media"
    );

    // When
    fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: "1".into(),
                command: Command::Undo,
            },
        )
        .await?;
    let inactive = router
        .oneshot(Request::get(asset_uri).body(Body::empty())?)
        .await?;

    // Then
    assert_eq!(inactive.status(), StatusCode::NOT_FOUND);
    assert!(
        fixture
            .data
            .path()
            .join("source-sessions")
            .join(&fixture.job.id)
            .join("media/program.mp4")
            .is_file()
    );
    Ok(())
}
