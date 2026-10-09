mod moments_support;

use longcut_server::model::{
    Command, CommandRequest, MomentEvidenceSelection, MomentNarrativeRole,
};
use moments_support::{fixture, transcript, visual_source};

#[tokio::test]
async fn update_with_null_evidence_clears_old_source_evidence() {
    // Given
    let mut fixture = fixture().await;
    fixture.project = fixture
        .state
        .store
        .add_source(&fixture.project.id, visual_source("replacement"))
        .await
        .expect("add replacement source");
    let transcript = transcript(&fixture).await;
    let created = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: Command::CreateMoment {
                    source_id: "source".into(),
                    start_frame: "30".into(),
                    end_frame: "60".into(),
                    label: "Original".into(),
                    note: String::new(),
                    narrative_role: MomentNarrativeRole::Context,
                    evidence: Some(MomentEvidenceSelection {
                        transcript_id: transcript.id,
                        cue_ids: vec![transcript.cues[0].id.clone()],
                        word_ids: vec![],
                    }),
                },
            },
        )
        .await
        .expect("create evidenced moment");
    // When
    let updated = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: created.revision.clone(),
                command: Command::UpdateMoment {
                    moment_id: created.moments[0].id.clone(),
                    expected_moment_version: created.moments[0].version.clone(),
                    source_id: "replacement".into(),
                    start_frame: "30".into(),
                    end_frame: "60".into(),
                    label: "Replacement".into(),
                    note: String::new(),
                    narrative_role: MomentNarrativeRole::Context,
                    evidence: None,
                },
            },
        )
        .await
        .expect("update moment");
    // Then
    assert_eq!(updated.moments[0].source_id, "replacement");
    assert!(updated.moments[0].evidence.is_empty());
}
