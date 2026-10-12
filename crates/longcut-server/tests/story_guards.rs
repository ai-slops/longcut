mod story_support;

use longcut_server::{
    error::AppError,
    model::{Command, StoryCommand},
};
use story_support::{beat, chapter, command, fixture, source, story};

#[tokio::test]
async fn chapter_delete_requires_an_empty_chapter() {
    // Given
    let fixture = fixture().await;
    let chapter = chapter(&fixture, &fixture.project, "Chapter").await;
    let chapter_id = chapter.story.chapters[0].id.clone();
    let project = beat(&fixture, &chapter, &chapter_id).await;

    // When
    let result = command(
        &fixture,
        &project,
        story(StoryCommand::DeleteChapter {
            expected_story_version: project.story.version.clone(),
            chapter_id,
        }),
    )
    .await;

    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn stale_story_version_wins_before_payload_validation() {
    // Given
    let fixture = fixture().await;
    let project = chapter(&fixture, &fixture.project, "Chapter").await;
    let chapter_id = project.story.chapters[0].id.clone();

    // When
    let result = command(
        &fixture,
        &project,
        story(StoryCommand::UpdateChapter {
            expected_story_version: "0".into(),
            chapter_id,
            title: "\u{3000}".into(),
            purpose: String::new(),
        }),
    )
    .await;

    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn malformed_or_overflowing_budget_is_rejected() {
    // Given
    let fixture = fixture().await;
    let project = chapter(&fixture, &fixture.project, "Chapter").await;
    let chapter_id = project.story.chapters[0].id.clone();

    // When
    let malformed = command(
        &fixture,
        &project,
        story(StoryCommand::CreateBeat {
            expected_story_version: project.story.version.clone(),
            chapter_id: chapter_id.clone(),
            title: "Beat".into(),
            purpose: String::new(),
            target_frames: Some("1.5".into()),
            before_beat_id: None,
        }),
    )
    .await;
    let overflow = command(
        &fixture,
        &project,
        story(StoryCommand::CreateBeat {
            expected_story_version: project.story.version.clone(),
            chapter_id,
            title: "Beat".into(),
            purpose: String::new(),
            target_frames: Some("2147483648".into()),
            before_beat_id: None,
        }),
    )
    .await;

    // Then
    assert!(matches!(malformed, Err(AppError::Invalid(_))));
    assert!(matches!(overflow, Err(AppError::Invalid(_))));
}

#[tokio::test]
async fn duplicate_moment_ids_inside_one_beat_are_rejected() {
    // Given
    let fixture = fixture().await;
    let project = fixture
        .store
        .add_source(&fixture.project.id, source())
        .await
        .expect("add source");
    let project = chapter(&fixture, &project, "Chapter").await;
    let chapter_id = project.story.chapters[0].id.clone();
    let project = beat(&fixture, &project, &chapter_id).await;
    let beat_id = project.story.chapters[0].beats[0].id.clone();

    // When
    let result = command(
        &fixture,
        &project,
        story(StoryCommand::SetBeatMomentRefs {
            expected_story_version: project.story.version.clone(),
            beat_id,
            moment_refs: vec![
                longcut_server::model::MomentVersionRef {
                    moment_id: "same".into(),
                    version: "1".into(),
                },
                longcut_server::model::MomentVersionRef {
                    moment_id: "same".into(),
                    version: "1".into(),
                },
            ],
        }),
    )
    .await;

    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
}

#[tokio::test]
async fn unknown_story_action_does_not_mutate_project() {
    // Given
    let fixture = fixture().await;

    // When
    let result = command(&fixture, &fixture.project, Command::Undo).await;

    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
}
