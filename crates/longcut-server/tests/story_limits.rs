mod story_support;

use longcut_server::{
    domain,
    error::AppError,
    model::{Beat, Chapter, MomentVersionRef, Story, StoryCommand},
};
use story_support::{fixture, story};

fn beat(id: usize, refs: usize, id_bytes: usize) -> Beat {
    Beat {
        id: format!("beat-{id}"),
        title: "Beat".into(),
        purpose: String::new(),
        target_frames: None,
        moment_refs: (0..refs)
            .map(|index| MomentVersionRef {
                moment_id: format!("{id}-{index}-{}", "m".repeat(id_bytes)),
                version: "1".into(),
            })
            .collect(),
    }
}

#[tokio::test]
async fn sixty_fifth_chapter_is_rejected() {
    // Given
    let fixture = fixture().await;
    let mut project = fixture.project;
    project.story = Story {
        version: "0".into(),
        chapters: (0..64)
            .map(|index| Chapter {
                id: format!("chapter-{index}"),
                title: "Chapter".into(),
                purpose: String::new(),
                beats: vec![],
            })
            .collect(),
    };
    let mut history = longcut_server::model::History {
        undo: vec![],
        redo: vec![],
    };

    // When
    let result = domain::apply(
        &mut project,
        &mut history,
        story(StoryCommand::CreateChapter {
            expected_story_version: "0".into(),
            title: "Overflow".into(),
            purpose: String::new(),
            before_chapter_id: None,
        }),
    );

    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
}

#[tokio::test]
async fn aggregate_beat_and_reference_limits_are_checked() {
    // Given
    let fixture = fixture().await;
    let mut project = fixture.project;
    project.story = Story {
        version: "0".into(),
        chapters: (0..5)
            .map(|chapter| Chapter {
                id: format!("chapter-{chapter}"),
                title: "Chapter".into(),
                purpose: String::new(),
                beats: (0..if chapter < 4 { 128 } else { 1 })
                    .map(|index| beat(chapter * 128 + index, 0, 0))
                    .collect(),
            })
            .collect(),
    };
    let mut history = longcut_server::model::History {
        undo: vec![],
        redo: vec![],
    };

    // When
    let beats = domain::apply(
        &mut project,
        &mut history,
        story(StoryCommand::UpdateChapter {
            expected_story_version: "0".into(),
            chapter_id: "chapter-0".into(),
            title: "Changed".into(),
            purpose: String::new(),
        }),
    );
    project.story.chapters = (0..129)
        .collect::<Vec<_>>()
        .chunks(128)
        .enumerate()
        .map(|(chapter, indexes)| Chapter {
            id: format!("refs-{chapter}"),
            title: "Chapter".into(),
            purpose: String::new(),
            beats: indexes.iter().map(|index| beat(*index, 64, 0)).collect(),
        })
        .collect();
    let refs = domain::apply(
        &mut project,
        &mut history,
        story(StoryCommand::UpdateChapter {
            expected_story_version: "0".into(),
            chapter_id: "refs-0".into(),
            title: "Changed".into(),
            purpose: String::new(),
        }),
    );

    // Then
    assert!(matches!(beats, Err(AppError::Invalid(_))));
    assert!(matches!(refs, Err(AppError::Invalid(_))));
}

#[tokio::test]
async fn serialized_story_over_four_mib_is_rejected() {
    // Given
    let fixture = fixture().await;
    let mut project = fixture.project;
    project.story = Story {
        version: "0".into(),
        chapters: vec![Chapter {
            id: "chapter".into(),
            title: "Chapter".into(),
            purpose: String::new(),
            beats: (0..128).map(|index| beat(index, 64, 600)).collect(),
        }],
    };
    let mut history = longcut_server::model::History {
        undo: vec![],
        redo: vec![],
    };

    // When
    let result = domain::apply(
        &mut project,
        &mut history,
        story(StoryCommand::UpdateChapter {
            expected_story_version: "0".into(),
            chapter_id: "chapter".into(),
            title: "Changed".into(),
            purpose: String::new(),
        }),
    );

    // Then
    assert!(matches!(result, Err(AppError::Invalid(_))));
}
