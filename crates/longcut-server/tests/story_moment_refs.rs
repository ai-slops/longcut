mod moments_support;

use longcut_server::model::{
    Command, MomentNarrativeRole, MomentReviewStatus, MomentVersionRef, StoryCommand,
};
use moments_support::{command, fixture};

#[tokio::test]
async fn pinned_reviewed_moment_survives_head_update_and_archive() {
    // Given
    let fixture = fixture().await;
    let created = command(
        &fixture,
        &fixture.project.revision,
        Command::CreateMoment {
            source_id: "source".into(),
            start_frame: "10".into(),
            end_frame: "20".into(),
            label: "Moment".into(),
            note: String::new(),
            narrative_role: MomentNarrativeRole::Setup,
            evidence: None,
        },
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
    let chapter = command(
        &fixture,
        &reviewed.revision,
        Command::Story {
            action: StoryCommand::CreateChapter {
                expected_story_version: "0".into(),
                title: "Chapter".into(),
                purpose: String::new(),
                before_chapter_id: None,
            },
        },
    )
    .await;
    let with_beat = command(
        &fixture,
        &chapter.revision,
        Command::Story {
            action: StoryCommand::CreateBeat {
                expected_story_version: chapter.story.version.clone(),
                chapter_id: chapter.story.chapters[0].id.clone(),
                title: "Beat".into(),
                purpose: String::new(),
                target_frames: None,
                before_beat_id: None,
            },
        },
    )
    .await;
    let frozen = MomentVersionRef {
        moment_id: reviewed.moments[0].id.clone(),
        version: reviewed.moments[0].version.clone(),
    };
    let linked = command(
        &fixture,
        &with_beat.revision,
        Command::Story {
            action: StoryCommand::SetBeatMomentRefs {
                expected_story_version: with_beat.story.version.clone(),
                beat_id: with_beat.story.chapters[0].beats[0].id.clone(),
                moment_refs: vec![frozen.clone()],
            },
        },
    )
    .await;

    // When
    let updated = command(
        &fixture,
        &linked.revision,
        Command::UpdateMoment {
            moment_id: frozen.moment_id.clone(),
            expected_moment_version: linked.moments[0].version.clone(),
            source_id: "source".into(),
            start_frame: "11".into(),
            end_frame: "21".into(),
            label: "Changed".into(),
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
            moment_id: frozen.moment_id.clone(),
            expected_moment_version: updated.moments[0].version.clone(),
        },
    )
    .await;

    // Then
    assert_eq!(
        archived.story.chapters[0].beats[0].moment_refs,
        vec![frozen]
    );
}
