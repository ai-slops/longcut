use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use longcut_server::{AppState, app, model::*};
use std::{path::Path, time::Duration};
use tower::ServiceExt;

#[path = "../../session-media/tests/common/mod.rs"]
mod fixture;
use fixture::TestResult;

fn multipart(root: &Path) -> Result<Vec<u8>, std::io::Error> {
    let mut bytes = Vec::new();
    for name in [
        "manifest.json",
        "sources.json",
        "time-maps.json",
        "visual-composition.json",
        "audio-mix.json",
        "media-health.json",
        "media/program.mp4",
        "media/clean.mp4",
    ] {
        bytes.extend_from_slice(format!("--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
        bytes.extend(std::fs::read(root.join(name))?);
        bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(b"--boundary--\r\n");
    Ok(bytes)
}

#[tokio::test]
async fn package_upload_prepare_confirm_playback_and_undo_are_integrated() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let package = tempfile::tempdir()?;
    fixture::prepare(package.path()).await?;
    let state = AppState::initialize(root.path()).await?;
    let project = state
        .store
        .create(CreateProject {
            title: "package".into(),
            frame_rate: 30,
        })
        .await?;
    let router = app(state.clone());
    let response = router
        .clone()
        .oneshot(
            Request::post(format!("/api/v1/projects/{}/session-imports", project.id))
                .header("content-type", "multipart/form-data; boundary=boundary")
                .body(Body::from(multipart(package.path())?))?,
        )
        .await?;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let job: SessionImportJob =
        serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
    // When
    let ready = tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            let value = state.store.get_import(&project.id, &job.id).await?;
            if value.status != ImportStatus::Validating {
                break Ok::<_, longcut_server::error::AppError>(value);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await??;
    // Then
    assert_eq!(ready.status, ImportStatus::Prepared, "{ready:?}");
    let summary = ready.session.as_ref().ok_or("prepared session")?;
    assert!(
        summary
            .sources
            .iter()
            .any(|source| source.editor_source_id.is_some()),
        "{summary:?}"
    );
    assert_eq!(state.store.get(&project.id).await?.revision, "0");
    let uri = format!(
        "/api/v1/projects/{}/session-imports/{}/confirm",
        project.id, job.id
    );
    let denied = router
        .clone()
        .oneshot(
            Request::post(&uri)
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"base_revision":"0","allow_degraded":false}"#,
                ))?,
        )
        .await?;
    assert_eq!(denied.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let response = router
        .clone()
        .oneshot(
            Request::post(&uri)
                .header("content-type", "application/json")
                .body(Body::from(r#"{"base_revision":"0","allow_degraded":true}"#))?,
        )
        .await?;
    let status = response.status();
    let bytes = response.into_body().collect().await?.to_bytes();
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let imported: Project = serde_json::from_slice(&bytes)?;
    assert_eq!(imported.revision, "1");
    assert_eq!(imported.source_sessions.len(), 1);
    assert!(!imported.sources.is_empty());
    let source = &imported.sources[0];
    let media = router
        .clone()
        .oneshot(
            Request::get(source.proxy_url.as_ref().ok_or("ready proxy")?)
                .header("range", "bytes=0-31")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(media.status(), StatusCode::PARTIAL_CONTENT);
    let record = state.store.source_session(&project.id, &job.id).await?;
    assert_eq!(record.package.registry.sources.len(), 3);
    assert!(
        root.path()
            .join("source-sessions")
            .join(&job.id)
            .join("media/program.mp4")
            .is_file()
    );
    let undone = state
        .store
        .command(
            &project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: "1".into(),
                command: Command::Undo,
            },
        )
        .await?;
    assert!(undone.sources.is_empty() && undone.source_sessions.is_empty());
    let restored = state
        .store
        .command(
            &project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: "2".into(),
                command: Command::Redo,
            },
        )
        .await?;
    assert_eq!(restored.source_sessions.len(), 1);
    let reopened = AppState::initialize(root.path()).await?;
    assert_eq!(
        reopened.store.get(&project.id).await?.source_sessions.len(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn package_path_traversal_is_rejected_without_project_mutation() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let state = AppState::initialize(root.path()).await?;
    let project = state
        .store
        .create(CreateProject {
            title: "test".into(),
            frame_rate: 30,
        })
        .await?;
    let bytes = b"--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"../escape\"\r\n\r\nattack\r\n--boundary--\r\n";
    // When
    let response = app(state.clone())
        .oneshot(
            Request::post(format!("/api/v1/projects/{}/session-imports", project.id))
                .header("content-type", "multipart/form-data; boundary=boundary")
                .body(Body::from(bytes.as_slice()))?,
        )
        .await?;
    // Then
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(state.store.get(&project.id).await?.revision, "0");
    assert!(!root.path().join("escape").exists());
    Ok(())
}
