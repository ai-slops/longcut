mod story_support;

use longcut_server::{
    error::AppError,
    model::{CommandRequest, Story, StoryCommand},
    store::Store,
};
use story_support::{chapter, fixture, story};

#[tokio::test]
async fn compact_history_and_cached_results_store_only_story_version_refs() {
    // Given
    let fixture = fixture().await;
    let first = chapter(&fixture, &fixture.project, "Frozen chapter title").await;
    let chapter_id = first.story.chapters[0].id.clone();
    let request = CommandRequest {
        command_id: uuid::Uuid::new_v4().to_string(),
        base_revision: first.revision.clone(),
        command: story(StoryCommand::UpdateChapter {
            expected_story_version: first.story.version.clone(),
            chapter_id,
            title: "Current chapter title".into(),
            purpose: String::new(),
        }),
    };

    // When
    fixture
        .store
        .command(&first.id, &request)
        .await
        .expect("update chapter");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture.root.path().join("longcut.sqlite3")),
    )
    .await
    .expect("pool");
    let history: String = sqlx::query_scalar("SELECT history FROM projects WHERE id=?")
        .bind(&first.id)
        .fetch_one(&pool)
        .await
        .expect("history");
    let result: String =
        sqlx::query_scalar("SELECT result FROM commands WHERE project_id=? AND command_id=?")
            .bind(&first.id)
            .bind(&request.command_id)
            .fetch_one(&pool)
            .await
            .expect("result");

    // Then
    assert!(history.contains("story_version"));
    assert!(result.contains("story_version"));
    assert!(!history.contains("Frozen chapter title"));
    assert!(!result.contains("Current chapter title"));
}

#[tokio::test]
async fn missing_exact_story_ref_fails_cached_replay_without_mutation() {
    // Given
    let fixture = fixture().await;
    let request = CommandRequest {
        command_id: uuid::Uuid::new_v4().to_string(),
        base_revision: fixture.project.revision.clone(),
        command: story(StoryCommand::CreateChapter {
            expected_story_version: "0".into(),
            title: "Chapter".into(),
            purpose: String::new(),
            before_chapter_id: None,
        }),
    };
    let created = fixture
        .store
        .command(&fixture.project.id, &request)
        .await
        .expect("create chapter");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture.root.path().join("longcut.sqlite3")),
    )
    .await
    .expect("pool");
    sqlx::query("DELETE FROM story_versions WHERE project_id=? AND version=?")
        .bind(&created.id)
        .bind(&created.story.version)
        .execute(&pool)
        .await
        .expect("delete version");

    // When
    let result = fixture.store.command(&created.id, &request).await;

    // Then
    assert!(matches!(result, Err(AppError::NotFound)));
    assert_eq!(
        fixture.store.get(&created.id).await.expect("project").story,
        created.story
    );
}

#[tokio::test]
async fn immutable_story_conflict_rolls_back_project_and_journal() {
    // Given
    let fixture = fixture().await;
    let first = chapter(&fixture, &fixture.project, "Chapter").await;
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture.root.path().join("longcut.sqlite3")),
    )
    .await
    .expect("pool");
    sqlx::query("INSERT INTO story_versions(project_id,version,snapshot) VALUES(?,?,?)")
        .bind(&first.id)
        .bind("2")
        .bind(
            serde_json::to_string(&Story {
                version: "2".into(),
                chapters: vec![],
            })
            .expect("story"),
        )
        .execute(&pool)
        .await
        .expect("seed conflict");
    let request = CommandRequest {
        command_id: uuid::Uuid::new_v4().to_string(),
        base_revision: first.revision.clone(),
        command: story(StoryCommand::CreateBeat {
            expected_story_version: first.story.version.clone(),
            chapter_id: first.story.chapters[0].id.clone(),
            title: "Beat".into(),
            purpose: String::new(),
            target_frames: None,
            before_beat_id: None,
        }),
    };

    // When
    let result = fixture.store.command(&first.id, &request).await;

    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
    assert_eq!(
        fixture.store.get(&first.id).await.expect("project").story,
        first.story
    );
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM commands WHERE project_id=? AND command_id=?")
            .bind(&first.id)
            .bind(&request.command_id)
            .fetch_one(&pool)
            .await
            .expect("journal count");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn schema_eight_migrates_to_story_versions_schema_nine() {
    // Given
    let fixture = fixture().await;
    let path = fixture.root.path().join("longcut.sqlite3");
    let pool =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
            .await
            .expect("pool");
    sqlx::query("DROP TABLE story_versions; PRAGMA user_version=8;")
        .execute(&pool)
        .await
        .expect("downgrade schema");
    pool.close().await;

    // When
    let _migrated = Store::open(&path).await.expect("migrate store");
    let pool =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
            .await
            .expect("pool");
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await
        .expect("version");

    // Then
    assert_eq!(version, 9);
}

#[tokio::test]
async fn compact_v1_without_story_ref_decodes_as_legacy_empty_zero() {
    // Given
    let fixture = fixture().await;
    let request = CommandRequest {
        command_id: uuid::Uuid::new_v4().to_string(),
        base_revision: "0".into(),
        command: longcut_server::model::Command::RenameProject {
            title: "Renamed".into(),
        },
    };
    fixture
        .store
        .command(&fixture.project.id, &request)
        .await
        .expect("rename");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture.root.path().join("longcut.sqlite3")),
    )
    .await
    .expect("pool");
    let result: String =
        sqlx::query_scalar("SELECT result FROM commands WHERE project_id=? AND command_id=?")
            .bind(&fixture.project.id)
            .bind(&request.command_id)
            .fetch_one(&pool)
            .await
            .expect("result");
    let mut value: serde_json::Value = serde_json::from_str(&result).expect("json");
    value
        .as_object_mut()
        .expect("object")
        .remove("story_version");
    sqlx::query("UPDATE commands SET result=? WHERE project_id=? AND command_id=?")
        .bind(serde_json::to_string(&value).expect("json"))
        .bind(&fixture.project.id)
        .bind(&request.command_id)
        .execute(&pool)
        .await
        .expect("legacy result");

    // When
    let replayed = fixture
        .store
        .command(&fixture.project.id, &request)
        .await
        .expect("replay");

    // Then
    assert_eq!(replayed.story, Story::default());
}
