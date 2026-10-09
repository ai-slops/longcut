use std::fs;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use longcut_server::{
    AppState, app,
    model::{CreateProject, ImportStatus, Project},
};
use longcut_session_contract::model::{Integer, TimeMaps};
use tower::ServiceExt;

#[path = "../src/session_import/adapter/test_fixture.rs"]
mod fixture;
use fixture::{TestResult, prepare_fixture, prepare_import, shift_program, video_packet_bounds};

#[tokio::test]
async fn nonzero_pts_import_is_normalized_without_losing_original_mapping() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let package = tempfile::tempdir()?;
    prepare_fixture(package.path()).await?;
    shift_program(package.path(), 2).await?;
    let state = AppState::initialize(root.path()).await?;
    let project = state
        .store
        .create(CreateProject {
            title: "offset".into(),
            frame_rate: 30,
        })
        .await?;

    // When
    let ready = prepare_import(&state, &project.id, package.path()).await?;

    // Then
    assert_eq!(ready.status, ImportStatus::Prepared, "{ready:?}");
    let prepared = state.store.prepared_import(&project.id, &ready.id).await?;
    let program = prepared
        .record
        .summary
        .sources
        .iter()
        .find(|source| source.source_id == "program")
        .ok_or("missing program source")?;
    assert!(program.editor_source_id.is_some(), "{program:?}");
    let mapping = prepared
        .record
        .mappings
        .iter()
        .find(|mapping| mapping.package_source_id == "program")
        .ok_or("missing program mapping")?;
    assert_eq!(mapping.bindings[0].media_start.ticks.get(), 60);
    assert_eq!(mapping.bindings[0].media_end.ticks.get(), 360);
    assert_eq!(mapping.bindings[1].media_start.ticks.get(), 96_000);
    assert_eq!(mapping.bindings[1].media_end.ticks.get(), 576_000);
    let response = app(state.clone())
        .oneshot(
            Request::post(format!(
                "/api/v1/projects/{}/session-imports/{}/confirm",
                project.id, ready.id
            ))
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
    let source = imported
        .sources
        .iter()
        .find(|source| Some(&source.id) == program.editor_source_id.as_ref())
        .ok_or("missing imported editor source")?;
    assert_eq!(source.duration_frames, "300");
    let artifact = source.media_url.rsplit('/').next().ok_or("artifact name")?;
    let (first_pts, end_pts, time_base) =
        video_packet_bounds(&root.path().join("artifacts").join(artifact)).await?;
    assert_eq!(
        (first_pts, end_pts, time_base.as_str()),
        (0, 153_600, "1/15360")
    );
    let playback = app(state)
        .oneshot(
            Request::get(source.proxy_url.as_ref().ok_or("ready proxy")?)
                .header("range", "bytes=0-31")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(playback.status(), StatusCode::PARTIAL_CONTENT);
    Ok(())
}

#[tokio::test]
async fn unequal_av_media_origins_remain_unsupported() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let package = tempfile::tempdir()?;
    prepare_fixture(package.path()).await?;
    shift_program(package.path(), 2).await?;
    let maps_path = package.path().join("time-maps.json");
    let mut maps: TimeMaps = serde_json::from_slice(&fs::read(&maps_path)?)?;
    maps.segments[1].media_range.start.ticks = Integer::new(96_001);
    maps.segments[1].media_range.end.ticks = Integer::new(576_001);
    fs::write(maps_path, serde_json::to_vec_pretty(&maps)?)?;
    let state = AppState::initialize(root.path()).await?;
    let project = state
        .store
        .create(CreateProject {
            title: "mismatch".into(),
            frame_rate: 30,
        })
        .await?;

    // When
    let ready = prepare_import(&state, &project.id, package.path()).await?;

    // Then
    assert_eq!(ready.status, ImportStatus::Prepared, "{ready:?}");
    let program = ready
        .session
        .as_ref()
        .ok_or("prepared summary")?
        .sources
        .iter()
        .find(|source| source.source_id == "program")
        .ok_or("missing program source")?;
    assert!(program.editor_source_id.is_none());
    assert_eq!(
        program.reason.as_deref(),
        Some("invalid input: AV bindings have different media origins")
    );
    Ok(())
}
