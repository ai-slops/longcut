mod story_support;

use longcut_server::model::{Command, StoryCommand};
use story_support::{command, fixture};

fn story(action: StoryCommand) -> Command {
    Command::Story { action }
}

#[tokio::test]
async fn outline_can_be_created_before_moments_or_scenes() {
    // Given
    let fixture = fixture().await;

    // When
    let project = command(
        &fixture,
        &fixture.project,
        story(StoryCommand::CreateChapter {
            expected_story_version: "0".into(),
            title: "Act one".into(),
            purpose: "Setup".into(),
            before_chapter_id: None,
        }),
    )
    .await
    .expect("create chapter");

    // Then
    assert_eq!(project.story.version, project.revision);
    assert_eq!(project.story.chapters[0].title, "Act one");
    assert!(project.moments.is_empty());
    assert!(project.scenes.is_empty());
}

#[tokio::test]
async fn beat_moves_across_chapters_without_changing_its_id() {
    // Given
    let fixture = fixture().await;
    let first = command(
        &fixture,
        &fixture.project,
        story(StoryCommand::CreateChapter {
            expected_story_version: "0".into(),
            title: "One".into(),
            purpose: String::new(),
            before_chapter_id: None,
        }),
    )
    .await
    .expect("create first chapter");
    let second = command(
        &fixture,
        &first,
        story(StoryCommand::CreateChapter {
            expected_story_version: first.story.version.clone(),
            title: "Two".into(),
            purpose: String::new(),
            before_chapter_id: None,
        }),
    )
    .await
    .expect("create second chapter");
    let with_beat = command(
        &fixture,
        &second,
        story(StoryCommand::CreateBeat {
            expected_story_version: second.story.version.clone(),
            chapter_id: second.story.chapters[0].id.clone(),
            title: "Beat".into(),
            purpose: String::new(),
            target_frames: Some("90".into()),
            before_beat_id: None,
        }),
    )
    .await
    .expect("create beat");
    let beat_id = with_beat.story.chapters[0].beats[0].id.clone();

    // When
    let moved = command(
        &fixture,
        &with_beat,
        story(StoryCommand::MoveBeat {
            expected_story_version: with_beat.story.version.clone(),
            beat_id: beat_id.clone(),
            chapter_id: with_beat.story.chapters[1].id.clone(),
            before_beat_id: None,
        }),
    )
    .await
    .expect("move beat");

    // Then
    assert!(moved.story.chapters[0].beats.is_empty());
    assert_eq!(moved.story.chapters[1].beats[0].id, beat_id);
}

#[tokio::test]
async fn target_budget_is_advisory_and_does_not_trim_scenes() {
    // Given
    let fixture = fixture().await;
    let chapter = command(
        &fixture,
        &fixture.project,
        story(StoryCommand::CreateChapter {
            expected_story_version: "0".into(),
            title: "Chapter".into(),
            purpose: String::new(),
            before_chapter_id: None,
        }),
    )
    .await
    .expect("create chapter");
    let chapter_id = chapter.story.chapters[0].id.clone();

    // When
    let project = command(
        &fixture,
        &chapter,
        story(StoryCommand::CreateBeat {
            expected_story_version: chapter.story.version.clone(),
            chapter_id,
            title: "Short target".into(),
            purpose: String::new(),
            target_frames: Some("1".into()),
            before_beat_id: None,
        }),
    )
    .await
    .expect("create beat");

    // Then
    assert_eq!(
        project.story.chapters[0].beats[0].target_frames.as_deref(),
        Some("1")
    );
    assert!(project.scenes.is_empty());
}
