mod moments_support;

use longcut_server::{
    error::AppError,
    model::{
        Command, CommandRequest, MomentEvidenceSelection, MomentNarrativeRole, MomentReviewStatus,
    },
};
use moments_support::{command, fixture};

fn create_moment() -> Command {
    Command::CreateMoment {
        source_id: "source".into(),
        start_frame: "30".into(),
        end_frame: "90".into(),
        label: "Opening problem".into(),
        note: String::new(),
        narrative_role: MomentNarrativeRole::Setup,
        evidence: None,
    }
}

fn invalid_update(moment_id: String, expected_moment_version: String) -> Command {
    Command::UpdateMoment {
        moment_id,
        expected_moment_version,
        source_id: "missing-source".into(),
        start_frame: "broken".into(),
        end_frame: "also-broken".into(),
        label: String::new(),
        note: String::new(),
        narrative_role: MomentNarrativeRole::Other,
        evidence: Some(MomentEvidenceSelection {
            transcript_id: "missing-transcript".into(),
            cue_ids: vec!["missing-cue".into()],
            word_ids: vec![],
        }),
    }
}

async fn apply(
    fixture: &moments_support::Fixture,
    base_revision: &str,
    command: Command,
) -> Result<longcut_server::model::Project, AppError> {
    fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: base_revision.into(),
                command,
            },
        )
        .await
}

#[tokio::test]
async fn stale_update_conflict_precedes_invalid_fields_and_evidence() {
    // Given
    let fixture = fixture().await;
    let created = command(&fixture, &fixture.project.revision, create_moment()).await;
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
    let result = apply(
        &fixture,
        &reviewed.revision,
        invalid_update(
            reviewed.moments[0].id.clone(),
            created.moments[0].version.clone(),
        ),
    )
    .await;

    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn archived_update_conflict_precedes_invalid_fields_and_evidence() {
    // Given
    let fixture = fixture().await;
    let created = command(&fixture, &fixture.project.revision, create_moment()).await;
    let archived = command(
        &fixture,
        &created.revision,
        Command::ArchiveMoment {
            moment_id: created.moments[0].id.clone(),
            expected_moment_version: created.moments[0].version.clone(),
        },
    )
    .await;

    // When
    let result = apply(
        &fixture,
        &archived.revision,
        invalid_update(
            archived.moments[0].id.clone(),
            archived.moments[0].version.clone(),
        ),
    )
    .await;

    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn archived_moment_must_be_restored_before_review() {
    // Given
    let fixture = fixture().await;
    let created = command(&fixture, &fixture.project.revision, create_moment()).await;
    let archived = command(
        &fixture,
        &created.revision,
        Command::ArchiveMoment {
            moment_id: created.moments[0].id.clone(),
            expected_moment_version: created.moments[0].version.clone(),
        },
    )
    .await;

    // When
    let result = apply(
        &fixture,
        &archived.revision,
        Command::ReviewMoment {
            moment_id: archived.moments[0].id.clone(),
            expected_moment_version: archived.moments[0].version.clone(),
            review_status: MomentReviewStatus::Reviewed,
        },
    )
    .await;

    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
}
