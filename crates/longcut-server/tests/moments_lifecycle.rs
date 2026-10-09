mod moments_support;

use axum::http::StatusCode;
use longcut_server::{
    error::AppError,
    model::{Command, CommandRequest, MomentNarrativeRole, MomentOrigin, MomentReviewStatus},
};
use moments_support::{body_json, command, fixture, json_request, router};

fn create_moment(source_id: &str, start: &str, end: &str) -> Command {
    Command::CreateMoment {
        source_id: source_id.into(),
        start_frame: start.into(),
        end_frame: end.into(),
        label: "Opening problem".into(),
        note: "Editorial note".into(),
        narrative_role: MomentNarrativeRole::Setup,
        evidence: None,
    }
}

#[tokio::test]
async fn manual_moment_can_be_created_without_transcript() {
    // Given
    let fixture = fixture().await;
    // When
    let project = command(
        &fixture,
        &fixture.project.revision,
        create_moment("source", "30", "90"),
    )
    .await;
    // Then
    let moment = &project.moments[0];
    assert_eq!(moment.source_id, "source");
    assert_eq!(moment.start_frame, "30");
    assert_eq!(moment.end_frame, "90");
    assert_eq!(moment.version, project.revision);
    assert_eq!(moment.origin, MomentOrigin::Manual);
    assert_eq!(moment.review_status, MomentReviewStatus::Draft);
    assert!(moment.evidence.is_empty());
}

#[tokio::test]
async fn moment_range_and_text_are_bounded() {
    // Given
    let fixture = fixture().await;
    // When
    let empty = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: create_moment("source", "90", "90"),
            },
        )
        .await;
    let outside = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: create_moment("source", "0", "301"),
            },
        )
        .await;
    let mut oversized = create_moment("source", "0", "1");
    if let Command::CreateMoment { label, .. } = &mut oversized {
        *label = "x".repeat(201);
    }
    let text = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: oversized,
            },
        )
        .await;
    // Then
    assert!(matches!(empty, Err(AppError::Invalid(_))));
    assert!(matches!(outside, Err(AppError::Invalid(_))));
    assert!(matches!(text, Err(AppError::Invalid(_))));
}

#[tokio::test]
async fn reviewed_moment_update_resets_review_and_rejects_stale_entity_version() {
    // Given
    let fixture = fixture().await;
    let created = command(
        &fixture,
        &fixture.project.revision,
        create_moment("source", "30", "90"),
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
    // When
    let updated = command(
        &fixture,
        &reviewed.revision,
        Command::UpdateMoment {
            moment_id: id.clone(),
            expected_moment_version: reviewed.moments[0].version.clone(),
            source_id: "source".into(),
            start_frame: "40".into(),
            end_frame: "100".into(),
            label: "Changed".into(),
            note: "Changed note".into(),
            narrative_role: MomentNarrativeRole::Insight,
            evidence: None,
        },
    )
    .await;
    let stale = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: updated.revision.clone(),
                command: Command::ArchiveMoment {
                    moment_id: id,
                    expected_moment_version: reviewed.moments[0].version.clone(),
                },
            },
        )
        .await;
    // Then
    assert_eq!(updated.moments[0].review_status, MomentReviewStatus::Draft);
    assert!(matches!(stale, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn archive_and_restore_are_versioned_edits() {
    // Given
    let fixture = fixture().await;
    let created = command(
        &fixture,
        &fixture.project.revision,
        create_moment("source", "30", "90"),
    )
    .await;
    let id = created.moments[0].id.clone();
    // When
    let archived = command(
        &fixture,
        &created.revision,
        Command::ArchiveMoment {
            moment_id: id.clone(),
            expected_moment_version: created.moments[0].version.clone(),
        },
    )
    .await;
    let restored = command(
        &fixture,
        &archived.revision,
        Command::RestoreMoment {
            moment_id: id,
            expected_moment_version: archived.moments[0].version.clone(),
        },
    )
    .await;
    // Then
    assert!(archived.moments[0].archived);
    assert!(!restored.moments[0].archived);
    assert_ne!(created.moments[0].version, archived.moments[0].version);
    assert_ne!(archived.moments[0].version, restored.moments[0].version);
}

#[tokio::test]
async fn project_revision_and_command_id_guards_apply_to_moments() {
    // Given
    let fixture = fixture().await;
    let id = uuid::Uuid::new_v4().to_string();
    let request = CommandRequest {
        command_id: id.clone(),
        base_revision: fixture.project.revision.clone(),
        command: create_moment("source", "30", "90"),
    };
    let first = fixture
        .state
        .store
        .command(&fixture.project.id, &request)
        .await
        .expect("create");
    // When
    let replay = fixture
        .state
        .store
        .command(&fixture.project.id, &request)
        .await
        .expect("replay");
    let stale_response = json_request(router(&fixture), "POST", &format!("/api/v1/projects/{}/commands", fixture.project.id), serde_json::json!({
        "command_id": uuid::Uuid::new_v4().to_string(),
        "base_revision": "0",
        "command": {"type":"create_moment","source_id":"source","start_frame":"1","end_frame":"2","label":"x","note":"","narrative_role":"other","evidence":null}
    })).await;
    let conflict = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: id,
                base_revision: fixture.project.revision.clone(),
                command: create_moment("source", "100", "120"),
            },
        )
        .await;
    // Then
    assert_eq!(first.moments[0].id, replay.moments[0].id);
    assert_eq!(stale_response.status(), StatusCode::CONFLICT);
    assert_eq!(
        body_json(stale_response).await["error"]["code"],
        "stale_revision"
    );
    assert!(matches!(conflict, Err(AppError::Conflict(_))));
}
