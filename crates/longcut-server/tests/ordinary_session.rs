use std::{path::Path, process::Stdio, time::Duration};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use longcut_server::{
    AppState, app,
    model::{Command, CommandRequest, CreateProject, MediaStatus, Project},
};
use longcut_session_contract::{Conformance, model::StreamKind};
use tokio::process::Command as ProcessCommand;
use tower::ServiceExt;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

async fn media(path: &Path) -> TestResult {
    let output = ProcessCommand::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x90:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000",
            "-t",
            "2",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-threads",
            "1",
            "-c:a",
            "aac",
            "-ac",
            "2",
            "-map_metadata",
            "-1",
            "-y",
        ])
        .arg(path)
        .stderr(Stdio::piped())
        .output()
        .await?;
    if !output.status.success() {
        return Err(format!(
            "fixture ffmpeg failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

fn multipart(path: &Path) -> TestResult<Vec<u8>> {
    let mut body = b"--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"ordinary.mp4\"\r\nContent-Type: video/mp4\r\n\r\n".to_vec();
    body.extend(std::fs::read(path)?);
    body.extend_from_slice(b"\r\n--boundary--\r\n");
    Ok(body)
}

#[tokio::test]
async fn ordinary_upload_registers_persisted_provenance_in_one_undoable_revision() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let input = root.path().join("input.mp4");
    media(&input).await?;
    let state = AppState::initialize(root.path()).await?;
    let project = state
        .store
        .create(CreateProject {
            title: "ordinary".into(),
            frame_rate: 30,
        })
        .await?;

    // When
    let response = app(state.clone())
        .oneshot(
            Request::post(format!("/api/v1/projects/{}/imports", project.id))
                .header("content-type", "multipart/form-data; boundary=boundary")
                .body(Body::from(multipart(&input)?))?,
        )
        .await?;

    // Then
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
    assert_eq!(imported.sources.len(), 1);
    assert_eq!(imported.sources[0].status, MediaStatus::Processing);
    assert_eq!(imported.source_sessions.len(), 1);
    assert_eq!(
        imported.source_sessions[0].conformance,
        Conformance::Degraded
    );
    assert!(imported.source_sessions[0].verification_url.is_some());
    let session = state
        .store
        .source_session(&project.id, &imported.source_sessions[0].id)
        .await?;
    assert_eq!(
        session.package.manifest.session_id,
        imported.source_sessions[0].session_id
    );
    assert!(session.package.time_maps.is_none());
    assert_eq!(
        session.package.registry.sources[0].source_id.0,
        imported.sources[0].id
    );
    assert_eq!(
        session.package.registry.assets[0]
            .size_bytes
            .map(|value| value.get()),
        Some(i64::try_from(std::fs::metadata(&input)?.len())?)
    );
    assert!(
        session.package.registry.assets[0]
            .streams
            .iter()
            .any(|stream| stream.kind == StreamKind::Video && stream.codec == "h264")
    );
    assert!(
        session.package.registry.assets[0]
            .streams
            .iter()
            .any(|stream| stream.kind == StreamKind::Audio && stream.codec == "aac")
    );
    let package_original = root
        .path()
        .join("source-sessions")
        .join(&imported.source_sessions[0].id)
        .join(&session.package.registry.assets[0].uri);
    assert!(package_original.is_file());
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
    let reopened = AppState::initialize(root.path()).await?;
    let persisted = reopened
        .store
        .source_session(&project.id, &session.summary.id)
        .await;
    assert!(persisted.is_err(), "undo must hide the immutable record");
    let redone = reopened
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
    assert_eq!(redone.source_sessions.len(), 1);
    assert_eq!(
        reopened
            .store
            .source_session(&project.id, &session.summary.id)
            .await?
            .package
            .registry
            .assets[0]
            .uri,
        session.package.registry.assets[0].uri
    );
    Ok(())
}

#[tokio::test]
async fn synthetic_demo_registers_the_same_persisted_source_session_model() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let state = AppState::initialize(root.path()).await?;
    let project = state
        .store
        .create(CreateProject {
            title: "demo".into(),
            frame_rate: 30,
        })
        .await?;

    // When
    let response = app(state.clone())
        .oneshot(
            Request::post(format!("/api/v1/projects/{}/demo", project.id)).body(Body::empty())?,
        )
        .await?;

    // Then
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
    assert_eq!(imported.sources.len(), 1);
    assert_eq!(imported.sources[0].name, "SYNTHETIC DEMO.mp4");
    assert_eq!(imported.source_sessions.len(), 1);
    let record = state
        .store
        .source_session(&project.id, &imported.source_sessions[0].id)
        .await?;
    assert_eq!(
        record.package.registry.sources[0].source_id.0,
        imported.sources[0].id
    );
    assert!(
        record.package.registry.assets[0]
            .uri
            .starts_with("original/")
    );
    Ok(())
}

#[tokio::test]
async fn stale_ordinary_upload_cleans_only_its_staging_and_preserves_project() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let input = root.path().join("input.mp4");
    media(&input).await?;
    let state = AppState::initialize(root.path()).await?;
    let project = state
        .store
        .create(CreateProject {
            title: "race".into(),
            frame_rate: 30,
        })
        .await?;
    let sentinel = root.path().join("session-imports/unrelated/keep");
    tokio::fs::create_dir_all(sentinel.parent().ok_or("sentinel parent")?).await?;
    tokio::fs::write(&sentinel, b"keep").await?;
    let request = Request::post(format!("/api/v1/projects/{}/imports", project.id))
        .header("content-type", "multipart/form-data; boundary=boundary")
        .body(Body::from(multipart(&input)?))?;
    let router = app(state.clone());
    let upload = tokio::spawn(async move { router.oneshot(request).await });
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let active = std::fs::read_dir(root.path().join("session-imports"))?
                .filter_map(Result::ok)
                .any(|entry| entry.file_name() != "unrelated");
            if active {
                return Ok::<(), std::io::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await??;

    // When
    state
        .store
        .command(
            &project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: "0".into(),
                command: Command::SetBrief {
                    brief: "preserved".into(),
                },
            },
        )
        .await?;
    let response = upload.await??;

    // Then
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let current = state.store.get(&project.id).await?;
    assert_eq!(current.revision, "1");
    assert_eq!(current.brief, "preserved");
    assert!(current.sources.is_empty() && current.source_sessions.is_empty());
    assert_eq!(tokio::fs::read(&sentinel).await?, b"keep");
    let owned_staging = std::fs::read_dir(root.path().join("session-imports"))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name() != "unrelated")
        .count();
    assert_eq!(owned_staging, 0);
    assert_eq!(std::fs::read_dir(root.path().join("artifacts"))?.count(), 0);
    Ok(())
}
