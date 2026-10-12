mod story_support;

use longcut_server::model::{Command, Render, RenderKind, RenderStatus, StoryCommand};
use story_support::{beat, chapter, command, fixture, source, story};

#[tokio::test]
async fn queued_render_keeps_frozen_outline_after_later_edits() {
    // Given
    let fixture = fixture().await;
    let project = fixture
        .store
        .add_source(&fixture.project.id, source())
        .await
        .expect("source");
    let project = chapter(&fixture, &project, "Frozen chapter").await;
    let chapter_id = project.story.chapters[0].id.clone();
    let project = beat(&fixture, &project, &chapter_id).await;
    let project = command(
        &fixture,
        &project,
        Command::InsertScene {
            source_id: "source".into(),
            in_frame: "0".into(),
            out_frame: "30".into(),
            title: "Scene".into(),
        },
    )
    .await
    .expect("scene");
    let project = command(
        &fixture,
        &project,
        story(StoryCommand::SetSceneBeat {
            expected_story_version: project.story.version.clone(),
            scene_id: project.scenes[0].id.clone(),
            beat_id: Some(project.story.chapters[0].beats[0].id.clone()),
        }),
    )
    .await
    .expect("assignment");
    let frozen_version = project.story.version.clone();
    let (_, snapshot) = fixture
        .store
        .enqueue_render(
            &project.id,
            Render {
                id: "render".into(),
                revision: project.revision.clone(),
                kind: RenderKind::Final,
                status: RenderStatus::Queued,
                progress: 0.0,
                media_url: None,
                srt_url: None,
                vtt_url: None,
                lineage_url: None,
                error: None,
            },
            &project.revision,
        )
        .await
        .expect("enqueue render");

    // When
    let current = command(
        &fixture,
        &project,
        story(StoryCommand::UpdateChapter {
            expected_story_version: project.story.version.clone(),
            chapter_id,
            title: "Later chapter".into(),
            purpose: String::new(),
        }),
    )
    .await
    .expect("edit outline");

    // Then
    assert_eq!(snapshot.story.version, frozen_version);
    assert_eq!(snapshot.story.chapters[0].title, "Frozen chapter");
    assert_eq!(snapshot.scenes[0].beat_id, project.scenes[0].beat_id);
    assert_eq!(current.story.chapters[0].title, "Later chapter");
}
