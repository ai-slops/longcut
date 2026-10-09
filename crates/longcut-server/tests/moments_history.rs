mod moments_support;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use longcut_server::model::{
    Command, MomentNarrativeRole, MomentReviewStatus, Project, Render, RenderKind, RenderStatus,
};
use moments_support::{audio_source, body_json, command, fixture, router};
use sqlx::Row;
use tower::ServiceExt;

fn create(source_id: &str, label: &str) -> Command {
    Command::CreateMoment {
        source_id: source_id.into(),
        start_frame: "30".into(),
        end_frame: "90".into(),
        label: label.into(),
        note: String::new(),
        narrative_role: MomentNarrativeRole::Setup,
        evidence: None,
    }
}

#[tokio::test]
async fn immutable_versions_survive_undo_redo_and_branch_pruning() {
    // Given
    let fixture = fixture().await;
    let created = command(
        &fixture,
        &fixture.project.revision,
        create("source", "Original"),
    )
    .await;
    let moment_id = created.moments[0].id.clone();
    let version = created.moments[0].version.clone();
    let undone = command(&fixture, &created.revision, Command::Undo).await;
    let redone = command(&fixture, &undone.revision, Command::Redo).await;
    let undone_again = command(&fixture, &redone.revision, Command::Undo).await;
    // When
    let branch = command(&fixture, &undone_again.revision, create("source", "Branch")).await;
    let response = router(&fixture)
        .oneshot(
            Request::get(format!(
                "/api/v1/projects/{}/moments/{moment_id}/versions/{version}",
                fixture.project.id
            ))
            .body(Body::empty())
            .expect("build request"),
        )
        .await
        .expect("route request");
    // Then
    assert!(
        branch.revision.parse::<u64>().expect("revision")
            > version.parse::<u64>().expect("version")
    );
    assert_ne!(branch.moments[0].id, moment_id);
    assert!(!branch.can_redo);
    assert_eq!(response.status(), StatusCode::OK);
    let record = body_json(response).await;
    assert_eq!(record["moment_id"], moment_id);
    assert_eq!(record["version"], version);
    assert_eq!(record["snapshot"]["label"], "Original");
}

#[tokio::test]
async fn reviewed_moment_inserts_scene_with_frozen_reference() {
    // Given
    let fixture = fixture().await;
    let created = command(
        &fixture,
        &fixture.project.revision,
        create("source", "Candidate"),
    )
    .await;
    let id = created.moments[0].id.clone();
    let reviewed = command(
        &fixture,
        &created.revision,
        Command::ReviewMoment {
            moment_id: id.clone(),
            expected_moment_version: created.moments[0].version.clone(),
            review_status: MomentReviewStatus::Reviewed,
        },
    )
    .await;
    let reviewed_version = reviewed.moments[0].version.clone();
    // When
    let inserted = command(
        &fixture,
        &reviewed.revision,
        Command::InsertSceneFromMoment {
            moment_id: id.clone(),
            expected_moment_version: reviewed_version.clone(),
            title: "Selected scene".into(),
        },
    )
    .await;
    let updated = command(
        &fixture,
        &inserted.revision,
        Command::UpdateMoment {
            moment_id: id.clone(),
            expected_moment_version: inserted.moments[0].version.clone(),
            source_id: "source".into(),
            start_frame: "40".into(),
            end_frame: "100".into(),
            label: "Later edit".into(),
            note: String::new(),
            narrative_role: MomentNarrativeRole::Insight,
            evidence: None,
        },
    )
    .await;
    let archived = command(
        &fixture,
        &updated.revision,
        Command::ArchiveMoment {
            moment_id: id.clone(),
            expected_moment_version: updated.moments[0].version.clone(),
        },
    )
    .await;
    // Then
    let scene = &archived.scenes[0];
    let reference = scene.moment.as_ref().expect("moment reference");
    assert_eq!(scene.in_frame, "30");
    assert_eq!(scene.out_frame, "90");
    assert_eq!(reference.moment_id, id);
    assert_eq!(reference.version, reviewed_version);
}

#[tokio::test]
async fn audio_moment_is_valid_but_cannot_be_inserted_as_visual_scene() {
    // Given
    let mut fixture = fixture().await;
    fixture.project = fixture
        .state
        .store
        .add_source(&fixture.project.id, audio_source("audio"))
        .await
        .expect("audio source");
    let created = command(
        &fixture,
        &fixture.project.revision,
        create("audio", "Audio only"),
    )
    .await;
    let reviewed = command(
        &fixture,
        &created.revision,
        Command::ReviewMoment {
            moment_id: created.moments[0].id.clone(),
            expected_moment_version: created.moments[0].version.clone(),
            review_status: MomentReviewStatus::Reviewed,
        },
    )
    .await;
    // When
    let result = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &longcut_server::model::CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: reviewed.revision.clone(),
                command: Command::InsertSceneFromMoment {
                    moment_id: reviewed.moments[0].id.clone(),
                    expected_moment_version: reviewed.moments[0].version.clone(),
                    title: "No".into(),
                },
            },
        )
        .await;
    // Then
    assert!(matches!(
        result,
        Err(longcut_server::error::AppError::Invalid(_))
    ));
}

#[tokio::test]
async fn render_snapshot_keeps_scene_moment_reference_after_later_edits() {
    // Given
    let fixture = fixture().await;
    let created = command(
        &fixture,
        &fixture.project.revision,
        create("source", "Render"),
    )
    .await;
    let reviewed = command(
        &fixture,
        &created.revision,
        Command::ReviewMoment {
            moment_id: created.moments[0].id.clone(),
            expected_moment_version: created.moments[0].version.clone(),
            review_status: MomentReviewStatus::Reviewed,
        },
    )
    .await;
    let inserted = command(
        &fixture,
        &reviewed.revision,
        Command::InsertSceneFromMoment {
            moment_id: reviewed.moments[0].id.clone(),
            expected_moment_version: reviewed.moments[0].version.clone(),
            title: "Render scene".into(),
        },
    )
    .await;
    let render = Render {
        id: "frozen".into(),
        revision: inserted.revision.clone(),
        kind: RenderKind::Final,
        status: RenderStatus::Queued,
        progress: 0.0,
        media_url: None,
        srt_url: None,
        vtt_url: None,
        lineage_url: None,
        error: None,
    };
    fixture
        .state
        .store
        .enqueue_render(&fixture.project.id, render, &inserted.revision)
        .await
        .expect("enqueue render");
    // When
    command(
        &fixture,
        &inserted.revision,
        Command::ArchiveMoment {
            moment_id: inserted.moments[0].id.clone(),
            expected_moment_version: inserted.moments[0].version.clone(),
        },
    )
    .await;
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture._root.path().join("longcut.sqlite3")),
    )
    .await
    .expect("open database");
    let row = sqlx::query("SELECT snapshot FROM render_jobs WHERE id='frozen'")
        .fetch_one(&pool)
        .await
        .expect("render snapshot");
    let frozen: Project = serde_json::from_str(row.get("snapshot")).expect("parse snapshot");
    // Then
    assert_eq!(frozen.scenes[0].moment, inserted.scenes[0].moment);
}
