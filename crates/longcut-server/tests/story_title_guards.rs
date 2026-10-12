mod story_support;

use longcut_server::{
    error::AppError,
    model::{CommandRequest, StoryCommand},
};
use story_support::{beat, chapter, command, fixture, story};

struct Case {
    name: &'static str,
    action: StoryCommand,
}

#[tokio::test]
async fn whitespace_only_titles_reject_atomically_at_current_story_version() {
    // Given
    let fixture = fixture().await;
    let project = chapter(&fixture, &fixture.project, "Chapter").await;
    let chapter_id = project.story.chapters[0].id.clone();
    let project = beat(&fixture, &project, &chapter_id).await;
    let beat_id = project.story.chapters[0].beats[0].id.clone();
    let version = project.story.version.clone();
    let cases = [
        Case {
            name: "create chapter with ASCII spaces",
            action: StoryCommand::CreateChapter {
                expected_story_version: version.clone(),
                title: "   ".into(),
                purpose: String::new(),
                before_chapter_id: None,
            },
        },
        Case {
            name: "update chapter with multibyte spaces",
            action: StoryCommand::UpdateChapter {
                expected_story_version: version.clone(),
                chapter_id: chapter_id.clone(),
                title: "\u{3000}\u{3000}".into(),
                purpose: String::new(),
            },
        },
        Case {
            name: "create beat with ASCII spaces",
            action: StoryCommand::CreateBeat {
                expected_story_version: version.clone(),
                chapter_id,
                title: "   ".into(),
                purpose: String::new(),
                target_frames: None,
                before_beat_id: None,
            },
        },
        Case {
            name: "update beat with multibyte spaces",
            action: StoryCommand::UpdateBeat {
                expected_story_version: version,
                beat_id,
                title: "\u{3000}".into(),
                purpose: String::new(),
                target_frames: None,
            },
        },
    ];
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture.root.path().join("longcut.sqlite3")),
    )
    .await
    .expect("open database");
    let before: (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT CAST(state AS BLOB),CAST(history AS BLOB) FROM projects WHERE id=?")
            .bind(&project.id)
            .fetch_one(&pool)
            .await
            .expect("read persisted project");
    let journal_before: i64 =
        sqlx::query_scalar("SELECT count(*) FROM commands WHERE project_id=?")
            .bind(&project.id)
            .fetch_one(&pool)
            .await
            .expect("read journal count");

    for case in cases {
        // When
        let result = fixture
            .store
            .command(
                &project.id,
                &CommandRequest {
                    command_id: uuid::Uuid::new_v4().to_string(),
                    base_revision: project.revision.clone(),
                    command: story(case.action),
                },
            )
            .await;

        // Then
        assert!(matches!(result, Err(AppError::Invalid(_))), "{}", case.name);
        let after: (Vec<u8>, Vec<u8>) = sqlx::query_as(
            "SELECT CAST(state AS BLOB),CAST(history AS BLOB) FROM projects WHERE id=?",
        )
        .bind(&project.id)
        .fetch_one(&pool)
        .await
        .expect("read persisted project");
        let journal_after: i64 =
            sqlx::query_scalar("SELECT count(*) FROM commands WHERE project_id=?")
                .bind(&project.id)
                .fetch_one(&pool)
                .await
                .expect("read journal count");
        assert_eq!(after, before, "{}", case.name);
        assert_eq!(journal_after, journal_before, "{}", case.name);
    }
}

#[tokio::test]
async fn nonempty_title_preserves_surrounding_whitespace() {
    // Given
    let fixture = fixture().await;
    let project = chapter(&fixture, &fixture.project, "Chapter").await;

    // When
    let updated = command(
        &fixture,
        &project,
        story(StoryCommand::UpdateChapter {
            expected_story_version: project.story.version.clone(),
            chapter_id: project.story.chapters[0].id.clone(),
            title: "  Padded title  ".into(),
            purpose: String::new(),
        }),
    )
    .await
    .expect("update chapter");

    // Then
    assert_eq!(updated.story.chapters[0].title, "  Padded title  ");
}
