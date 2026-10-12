mod story_support;

use longcut_server::{
    error::AppError,
    model::{Command, Operation, OperationType, StoryCommand},
};
use story_support::{beat, chapter, command, fixture, source, story};

async fn scene_and_beat() -> (story_support::Fixture, longcut_server::model::Project) {
    let fixture = fixture().await;
    let project = fixture
        .store
        .add_source(&fixture.project.id, source())
        .await
        .expect("add source");
    let project = chapter(&fixture, &project, "Chapter").await;
    let chapter_id = project.story.chapters[0].id.clone();
    let project = beat(&fixture, &project, &chapter_id).await;
    let project = command(
        &fixture,
        &project,
        Command::InsertScene {
            source_id: "source".into(),
            in_frame: "0".into(),
            out_frame: "100".into(),
            title: "Scene".into(),
        },
    )
    .await
    .expect("insert scene");
    (fixture, project)
}

#[tokio::test]
async fn scene_assignment_advances_project_but_not_story_version() {
    // Given
    let (fixture, project) = scene_and_beat().await;
    let story_version = project.story.version.clone();

    // When
    let assigned = command(
        &fixture,
        &project,
        story(StoryCommand::SetSceneBeat {
            expected_story_version: story_version.clone(),
            scene_id: project.scenes[0].id.clone(),
            beat_id: Some(project.story.chapters[0].beats[0].id.clone()),
        }),
    )
    .await
    .expect("assign scene");

    // Then
    assert_ne!(assigned.revision, project.revision);
    assert_eq!(assigned.story.version, story_version);
    assert!(assigned.scenes[0].beat_id.is_some());
}

#[tokio::test]
async fn split_inherits_assignment_and_manual_insert_is_unassigned() {
    // Given
    let (fixture, project) = scene_and_beat().await;
    assert!(project.scenes[0].beat_id.is_none());
    let assigned = command(
        &fixture,
        &project,
        story(StoryCommand::SetSceneBeat {
            expected_story_version: project.story.version.clone(),
            scene_id: project.scenes[0].id.clone(),
            beat_id: Some(project.story.chapters[0].beats[0].id.clone()),
        }),
    )
    .await
    .expect("assign scene");

    // When
    let split = command(
        &fixture,
        &assigned,
        Command::SplitScene {
            scene_id: assigned.scenes[0].id.clone(),
            offset_frame: "50".into(),
        },
    )
    .await
    .expect("split scene");

    // Then
    assert_eq!(split.scenes[0].beat_id, split.scenes[1].beat_id);
}

#[tokio::test]
async fn locked_scene_rejects_assignment() {
    // Given
    let (fixture, project) = scene_and_beat().await;
    let locked = command(
        &fixture,
        &project,
        Command::LockScene {
            scene_id: project.scenes[0].id.clone(),
            locked: true,
        },
    )
    .await
    .expect("lock scene");

    // When
    let result = command(
        &fixture,
        &locked,
        story(StoryCommand::SetSceneBeat {
            expected_story_version: locked.story.version.clone(),
            scene_id: locked.scenes[0].id.clone(),
            beat_id: Some(locked.story.chapters[0].beats[0].id.clone()),
        }),
    )
    .await;

    // Then
    assert!(matches!(result, Err(AppError::Locked(_))));
}

#[tokio::test]
async fn assigned_beat_cannot_be_deleted() {
    // Given
    let (fixture, project) = scene_and_beat().await;
    let beat_id = project.story.chapters[0].beats[0].id.clone();
    let assigned = command(
        &fixture,
        &project,
        story(StoryCommand::SetSceneBeat {
            expected_story_version: project.story.version.clone(),
            scene_id: project.scenes[0].id.clone(),
            beat_id: Some(beat_id.clone()),
        }),
    )
    .await
    .expect("assign scene");

    // When
    let result = command(
        &fixture,
        &assigned,
        story(StoryCommand::DeleteBeat {
            expected_story_version: assigned.story.version.clone(),
            beat_id,
        }),
    )
    .await;

    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn proposal_inserted_scene_is_unassigned() {
    // Given
    let (fixture, project) = scene_and_beat().await;
    let proposed = command(
        &fixture,
        &project,
        Command::CreateChangeSet {
            title: "AI proposal".into(),
            intent: "Insert scene".into(),
            operations: vec![Operation {
                id: "insert".into(),
                scene_id: "proposed-scene".into(),
                kind: OperationType::InsertScene,
                camera: None,
                in_frame: Some("100".into()),
                out_frame: Some("150".into()),
                source_id: Some("source".into()),
                title: Some("Proposed".into()),
                purpose: None,
                caption: None,
                rationale: "Selected source range".into(),
            }],
            evidence: vec![],
        },
    )
    .await
    .expect("create change set");

    // When
    let applied = command(
        &fixture,
        &proposed,
        Command::ApplyChangeSet {
            change_set_id: proposed.change_sets[0].id.clone(),
            operation_ids: vec!["insert".into()],
        },
    )
    .await
    .expect("apply change set");

    // Then
    assert_eq!(applied.scenes.len(), 2);
    assert!(applied.scenes[1].beat_id.is_none());
}
