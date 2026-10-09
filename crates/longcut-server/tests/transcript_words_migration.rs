use longcut_server::{
    model::{CreateProject, MediaStatus, Source, SourceRole},
    store::Store,
    transcript::ImportTranscript,
};
use sqlx::Row;

type TestResult = Result<(), Box<dyn std::error::Error>>;

async fn version_seven_database(
    root: &tempfile::TempDir,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let path = root.path().join("legacy.sqlite3");
    let store = Store::open(&path).await?;
    let project = store
        .create(CreateProject {
            title: "word migration".into(),
            frame_rate: 30,
        })
        .await?;
    store
        .add_source(
            &project.id,
            Source {
                id: "source".into(),
                name: "source.mp4".into(),
                role: SourceRole::Program,
                duration_frames: "300".into(),
                width: 1920,
                height: 1080,
                has_audio: true,
                media_url: "/api/v1/media/source.mp4".into(),
                proxy_url: None,
                thumbnail_url: None,
                waveform: vec![],
                status: MediaStatus::Ready,
                error: None,
            },
        )
        .await?;
    store
        .import_transcript(
            &project.id,
            "source",
            ImportTranscript {
                filename: "legacy.srt".into(),
                text: "1\n00:00:01,000 --> 00:00:02,000\nlegacy".into(),
            },
        )
        .await?;
    drop(store);
    Ok(path)
}

async fn set_words(path: &std::path::Path, words: &str) -> TestResult {
    let pool =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(path))
            .await?;
    sqlx::query("UPDATE transcript_cues SET words=?")
        .bind(words)
        .execute(&pool)
        .await?;
    sqlx::query("DROP TABLE transcript_words")
        .execute(&pool)
        .await?;
    sqlx::query("PRAGMA user_version=7").execute(&pool).await?;
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn version_seven_words_backfill_to_index_and_survive_reopen() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let path = version_seven_database(&root).await?;
    let words = serde_json::json!([
        {"id":"word-b","start_frame":"35","end_frame":"40","text":"hello"},
        {"id":"word-a","start_frame":"40","end_frame":"40","text":"."},
        {"id":"word-c","start_frame":"41","end_frame":"50","text":"안녕"}
    ])
    .to_string();
    set_words(&path, &words).await?;

    // When
    drop(Store::open(&path).await?);
    drop(Store::open(&path).await?);
    let pool =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
            .await?;
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await?;
    let rows = sqlx::query(
        "SELECT word_id,ordinal,start_frame,end_frame,text FROM transcript_words ORDER BY ordinal",
    )
    .fetch_all(&pool)
    .await?;
    let retained: String = sqlx::query_scalar("SELECT words FROM transcript_cues")
        .fetch_one(&pool)
        .await?;

    // Then
    assert_eq!(version, 8);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].get::<String, _>("word_id"), "word-b");
    assert_eq!(rows[0].get::<i64, _>("ordinal"), 0);
    assert_eq!(rows[1].get::<String, _>("word_id"), "word-a");
    assert_eq!(rows[1].get::<String, _>("start_frame"), "40");
    assert_eq!(rows[1].get::<String, _>("end_frame"), "40");
    assert_eq!(rows[2].get::<String, _>("text"), "안녕");
    assert_eq!(retained, words);
    Ok(())
}

#[tokio::test]
async fn malformed_version_seven_words_roll_back_migration_and_keep_backup() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let path = version_seven_database(&root).await?;
    let malformed = "{not-json";
    set_words(&path, malformed).await?;

    // When
    let result = Store::open(&path).await;
    let pool =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
            .await?;
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await?;
    let table_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='transcript_words'",
    )
    .fetch_one(&pool)
    .await?;
    let retained: String = sqlx::query_scalar("SELECT words FROM transcript_cues")
        .fetch_one(&pool)
        .await?;
    let backup_exists = std::fs::read_dir(root.path())?
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .contains("before-transcript-words-")
        });

    // Then
    assert!(result.is_err());
    assert_eq!(version, 7);
    assert_eq!(table_count, 0);
    assert_eq!(retained, malformed);
    assert!(backup_exists);
    Ok(())
}
