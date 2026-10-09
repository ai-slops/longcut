mod moments_support;

use axum::http::StatusCode;
use longcut_server::{
    error::AppError,
    model::{
        Command, CommandRequest, CreateProject, MomentEvidenceSelection, MomentNarrativeRole,
        MomentReviewStatus,
    },
    transcript::ImportTranscript,
};
use moments_support::{fixture, json_request, router, transcript, visual_source};

fn create_with_evidence(
    transcript_id: &str,
    cue_ids: Vec<String>,
    start_frame: &str,
    end_frame: &str,
) -> Command {
    Command::CreateMoment {
        source_id: "source".into(),
        start_frame: start_frame.into(),
        end_frame: end_frame.into(),
        label: "Evidence".into(),
        note: String::new(),
        narrative_role: MomentNarrativeRole::Context,
        evidence: Some(MomentEvidenceSelection {
            transcript_id: transcript_id.into(),
            cue_ids,
            word_ids: vec![],
        }),
    }
}

#[tokio::test]
async fn cue_evidence_is_resolved_and_frozen_server_side() {
    // Given
    let fixture = fixture().await;
    let transcript = transcript(&fixture).await;
    let cue_ids = transcript.cues.iter().map(|cue| cue.id.clone()).collect();
    // When
    let project = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: create_with_evidence(&transcript.id, cue_ids, "30", "90"),
            },
        )
        .await
        .expect("create moment");
    // Then
    let evidence = &project.moments[0].evidence[0];
    assert_eq!(evidence.transcript_id, transcript.id);
    assert_eq!(evidence.quote, "First exact quote\nSecond exact quote");
    assert_eq!(evidence.start_frame, "30");
    assert_eq!(evidence.end_frame, "90");
}

#[tokio::test]
async fn client_supplied_quote_is_rejected() {
    // Given
    let fixture = fixture().await;
    let transcript = transcript(&fixture).await;
    // When
    let response = json_request(router(&fixture), "POST", &format!("/api/v1/projects/{}/commands", fixture.project.id), serde_json::json!({
        "command_id": uuid::Uuid::new_v4().to_string(),
        "base_revision": fixture.project.revision,
        "command": {
            "type":"create_moment", "source_id":"source", "start_frame":"30", "end_frame":"60",
            "label":"Forged", "note":"", "narrative_role":"other",
            "evidence":{"transcript_id":transcript.id,"cue_ids":[transcript.cues[0].id],"word_ids":[],"quote":"forged"}
        }
    })).await;
    // Then
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn evidence_rejects_cross_project_and_out_of_range_references() {
    // Given
    let fixture = fixture().await;
    let transcript = transcript(&fixture).await;
    let other = fixture
        .state
        .store
        .create(CreateProject {
            title: "Other".into(),
            frame_rate: 30,
        })
        .await
        .expect("create other project");
    let other = fixture
        .state
        .store
        .add_source(&other.id, visual_source("source"))
        .await
        .expect("add source");
    // When
    let cross_project = fixture
        .state
        .store
        .command(
            &other.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: other.revision,
                command: create_with_evidence(
                    &transcript.id,
                    vec![transcript.cues[0].id.clone()],
                    "30",
                    "60",
                ),
            },
        )
        .await;
    let out_of_range = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: create_with_evidence(
                    &transcript.id,
                    vec![transcript.cues[0].id.clone()],
                    "0",
                    "20",
                ),
            },
        )
        .await;
    // Then
    assert!(matches!(cross_project, Err(AppError::Invalid(_))));
    assert!(matches!(out_of_range, Err(AppError::Invalid(_))));
}

#[tokio::test]
async fn evidence_stays_pinned_after_new_transcript_import_and_mutation_resets_review() {
    // Given
    let fixture = fixture().await;
    let old = transcript(&fixture).await;
    let created = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: create_with_evidence(&old.id, vec![old.cues[0].id.clone()], "30", "60"),
            },
        )
        .await
        .expect("create moment");
    let id = created.moments[0].id.clone();
    let reviewed = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: created.revision.clone(),
                command: Command::ReviewMoment {
                    moment_id: id.clone(),
                    expected_moment_version: created.moments[0].version.clone(),
                    review_status: MomentReviewStatus::Reviewed,
                },
            },
        )
        .await
        .expect("review");
    let new = fixture
        .state
        .store
        .import_transcript(
            &fixture.project.id,
            "source",
            ImportTranscript {
                filename: "replacement.srt".into(),
                text: "1\n00:00:01,000 --> 00:00:02,000\nReplacement".into(),
            },
        )
        .await
        .expect("replace transcript");
    // When
    let updated = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: reviewed.revision.clone(),
                command: Command::UpdateMoment {
                    moment_id: id,
                    expected_moment_version: reviewed.moments[0].version.clone(),
                    source_id: "source".into(),
                    start_frame: "30".into(),
                    end_frame: "60".into(),
                    label: "Replacement evidence".into(),
                    note: String::new(),
                    narrative_role: MomentNarrativeRole::Insight,
                    evidence: Some(MomentEvidenceSelection {
                        transcript_id: old.id.clone(),
                        cue_ids: vec![old.cues[0].id.clone()],
                        word_ids: vec![],
                    }),
                },
            },
        )
        .await
        .expect("update");
    // Then
    assert_ne!(new.id, old.id);
    assert_eq!(updated.moments[0].evidence[0].transcript_id, old.id);
    assert_eq!(updated.moments[0].evidence[0].quote, "First exact quote");
    assert_eq!(updated.moments[0].review_status, MomentReviewStatus::Draft);
}

#[tokio::test]
async fn empty_evidence_selection_is_rejected() {
    // Given
    let fixture = fixture().await;
    let transcript = transcript(&fixture).await;
    // When
    let result = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: create_with_evidence(&transcript.id, vec![], "30", "60"),
            },
        )
        .await;
    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
}
