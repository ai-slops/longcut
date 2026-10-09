mod moments_support;

use longcut_server::{
    error::AppError,
    model::{Command, CommandRequest, MomentEvidenceSelection, MomentNarrativeRole},
    transcript::ImportTranscript,
};
use moments_support::fixture;

fn create(transcript_id: String, cue_ids: Vec<String>, end_frame: &str) -> Command {
    Command::CreateMoment {
        source_id: "source".into(),
        start_frame: "0".into(),
        end_frame: end_frame.into(),
        label: "Bounded evidence".into(),
        note: String::new(),
        narrative_role: MomentNarrativeRole::Context,
        evidence: Some(MomentEvidenceSelection {
            transcript_id,
            cue_ids,
            word_ids: vec![],
        }),
    }
}

#[tokio::test]
async fn overlapping_selected_cue_with_long_earlier_extent_is_rejected() {
    // Given
    let fixture = fixture().await;
    let transcript = fixture.state.store.import_transcript(
        &fixture.project.id,
        "source",
        ImportTranscript {
            filename: "overlap.srt".into(),
            text: "1\n00:00:00,000 --> 00:00:03,333\nLong earlier cue\n\n2\n00:00:00,333 --> 00:00:00,666\nShort later cue".into(),
        },
    ).await.expect("import overlapping transcript");
    let cue_ids = transcript.cues.iter().map(|cue| cue.id.clone()).collect();
    // When
    let result = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: fixture.project.revision.clone(),
                command: create(transcript.id, cue_ids, "20"),
            },
        )
        .await;
    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
    assert!(
        fixture
            .state
            .store
            .get(&fixture.project.id)
            .await
            .expect("project")
            .moments
            .is_empty()
    );
}

#[tokio::test]
async fn multibyte_quote_over_sixteen_kib_is_atomically_rejected() {
    // Given
    let fixture = fixture().await;
    let text = "가".repeat(3_000);
    let transcript = fixture.state.store.import_transcript(
        &fixture.project.id,
        "source",
        ImportTranscript {
            filename: "large.srt".into(),
            text: format!("1\n00:00:00,000 --> 00:00:01,000\n{text}\n\n2\n00:00:01,000 --> 00:00:02,000\n{text}"),
        },
    ).await.expect("import bounded cues");
    let cue_ids = transcript.cues.iter().map(|cue| cue.id.clone()).collect();
    let revision = fixture.project.revision.clone();
    // When
    let result = fixture
        .state
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: revision.clone(),
                command: create(transcript.id, cue_ids, "60"),
            },
        )
        .await;
    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
    let project = fixture
        .state
        .store
        .get(&fixture.project.id)
        .await
        .expect("project");
    assert_eq!(project.revision, revision);
    assert!(project.moments.is_empty());
}
