use longcut_server::{
    model::*,
    store::Store,
    transcript::{ImportTranscript, Search},
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

async fn fixture() -> Result<(tempfile::TempDir, Store, Project), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let store = Store::open(&root.path().join("project.sqlite3")).await?;
    let project = store
        .create(CreateProject {
            title: "transcripts".into(),
            frame_rate: 30,
        })
        .await?;
    let project = store
        .add_source(
            &project.id,
            Source {
                id: "source".into(),
                name: "original".into(),
                role: SourceRole::Program,
                duration_frames: "300".into(),
                width: 160,
                height: 90,
                has_audio: true,
                media_url: "/api/v1/media/original.mp4".into(),
                proxy_url: None,
                thumbnail_url: None,
                waveform: vec![],
                status: MediaStatus::Ready,
                error: None,
            },
        )
        .await?;
    Ok((root, store, project))
}

#[tokio::test]
async fn imported_korean_transcript_is_searchable_without_changing_edit_revision() -> TestResult {
    // Given
    let (_root, store, project) = fixture().await?;
    let imported = store.import_transcript(&project.id, "source", ImportTranscript { filename: "자막.srt".into(), text: "1\n00:00:00,000 --> 00:00:01,000\n첫 번째 실패 원인\n\n2\n00:00:02,000 --> 00:00:03,000\n전략 변경 후 성공".into() }).await?;
    // When
    let phrase = store
        .transcript(
            &project.id,
            "source",
            Search {
                query: Some("실패 원인".into()),
                cursor: None,
            },
        )
        .await?
        .ok_or("transcript")?;
    let short = store
        .transcript(
            &project.id,
            "source",
            Search {
                query: Some("성공".into()),
                cursor: None,
            },
        )
        .await?
        .ok_or("transcript")?;
    // Then
    assert_eq!(imported.cue_count, 2);
    assert!(imported.cues[0].words.is_none());
    assert!(
        serde_json::to_value(&imported.cues[0])?
            .get("words")
            .is_none()
    );
    assert_eq!(phrase.cues.len(), 1);
    assert_eq!(phrase.cues[0].start_frame, "0");
    assert_eq!(short.cues[0].start_frame, "60");
    assert_eq!(store.get(&project.id).await?.revision, project.revision);
    Ok(())
}

#[tokio::test]
async fn version_three_database_migrates_nullable_word_storage() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let path = root.path().join("legacy.sqlite3");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options).await?;
    sqlx::query("CREATE TABLE transcript_cues(id TEXT PRIMARY KEY,transcript_id TEXT NOT NULL,ordinal INTEGER NOT NULL,start_frame TEXT NOT NULL,end_frame TEXT NOT NULL,text TEXT NOT NULL); CREATE TABLE asr_jobs(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,source_id TEXT NOT NULL,job TEXT NOT NULL,status TEXT NOT NULL,base_transcript_id TEXT,created_at TEXT NOT NULL); PRAGMA user_version=3;")
        .execute(&pool)
        .await?;
    sqlx::query("INSERT INTO transcript_cues(id,transcript_id,ordinal,start_frame,end_frame,text) VALUES('legacy-cue','legacy-transcript',0,'3','18','legacy text')")
        .execute(&pool)
        .await?;
    pool.close().await;
    // When
    let store = Store::open(&path).await?;
    drop(store);
    let pool =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
            .await?;
    let columns = sqlx::query("PRAGMA table_info(transcript_cues)")
        .fetch_all(&pool)
        .await?;
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await?;
    let cue = sqlx::query("SELECT id,start_frame,end_frame,text,words FROM transcript_cues")
        .fetch_one(&pool)
        .await?;
    // Then
    use sqlx::Row;
    assert!(
        columns
            .iter()
            .any(|row| row.get::<String, _>("name") == "words")
    );
    assert_eq!(version, 6);
    assert_eq!(cue.get::<String, _>("id"), "legacy-cue");
    assert_eq!(cue.get::<String, _>("start_frame"), "3");
    assert_eq!(cue.get::<String, _>("end_frame"), "18");
    assert_eq!(cue.get::<String, _>("text"), "legacy text");
    assert_eq!(cue.get::<Option<String>, _>("words"), None);
    Ok(())
}

#[tokio::test]
async fn version_four_database_migrates_nullable_asr_resume_storage() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let path = root.path().join("legacy-v4.sqlite3");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options).await?;
    sqlx::query("CREATE TABLE projects(id TEXT PRIMARY KEY,state TEXT NOT NULL,history TEXT NOT NULL,updated_at TEXT NOT NULL); CREATE TABLE transcript_cues(id TEXT PRIMARY KEY,transcript_id TEXT NOT NULL,ordinal INTEGER NOT NULL,start_frame TEXT NOT NULL,end_frame TEXT NOT NULL,text TEXT NOT NULL,words TEXT); CREATE TABLE asr_jobs(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,source_id TEXT NOT NULL,job TEXT NOT NULL,status TEXT NOT NULL,base_transcript_id TEXT,created_at TEXT NOT NULL); INSERT INTO asr_jobs VALUES('legacy','project','source','{\"id\":\"legacy\",\"project_id\":\"project\",\"source_id\":\"source\",\"status\":\"failed\",\"progress\":{\"completed_chunks\":0,\"total_chunks\":0},\"language\":\"ko\",\"engine\":\"whisper.cpp\",\"model\":\"model\",\"created_at\":\"now\",\"updated_at\":\"now\",\"transcript_id\":null,\"error\":null}', 'failed',NULL,'now'); PRAGMA user_version=4;")
        .execute(&pool)
        .await?;
    pool.close().await;
    // When
    let store = Store::open(&path).await?;
    drop(store);
    let pool =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&path))
            .await?;
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await?;
    let snapshot: Option<String> =
        sqlx::query_scalar("SELECT resume_snapshot FROM asr_jobs WHERE id='legacy'")
            .fetch_one(&pool)
            .await?;
    // Then
    assert_eq!(version, 6);
    assert_eq!(snapshot, None);
    Ok(())
}

#[tokio::test]
async fn paging_is_pinned_to_an_immutable_transcript_version() -> TestResult {
    // Given
    let (_root, store, project) = fixture().await?;
    let text = (0..101)
        .map(|index| format!("{index}\n00:00:00,000 --> 00:00:01,000\n원문 {index}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let old = store
        .import_transcript(
            &project.id,
            "source",
            ImportTranscript {
                filename: "old.srt".into(),
                text,
            },
        )
        .await?;
    store
        .import_transcript(
            &project.id,
            "source",
            ImportTranscript {
                filename: "new.srt".into(),
                text: "1\n00:00:00,000 --> 00:00:01,000\n새 버전".into(),
            },
        )
        .await?;
    // When
    let page = store
        .transcript(
            &project.id,
            "source",
            Search {
                query: None,
                cursor: old.next_cursor,
            },
        )
        .await?
        .ok_or("old page")?;
    // Then
    assert_eq!(page.id, old.id);
    assert_eq!(page.cues.len(), 1);
    assert_eq!(page.cues[0].text, "원문 100");
    assert!(page.next_cursor.is_none());
    Ok(())
}

#[tokio::test]
async fn invalid_cues_do_not_replace_previous_transcript() -> TestResult {
    // Given
    let (_root, store, project) = fixture().await?;
    let old = store
        .import_transcript(
            &project.id,
            "source",
            ImportTranscript {
                filename: "old.srt".into(),
                text: "1\n00:00:00,000 --> 00:00:01,000\n유지".into(),
            },
        )
        .await?;
    // When
    let result = store
        .import_transcript(
            &project.id,
            "source",
            ImportTranscript {
                filename: "bad.srt".into(),
                text: "1\n00:00:00,000 --> 00:01:00,000\n범위 초과".into(),
            },
        )
        .await;
    // Then
    assert!(result.is_err());
    assert_eq!(
        store
            .transcript(&project.id, "source", Search::default())
            .await?
            .ok_or("old")?
            .id,
        old.id
    );
    assert!(
        store
            .transcript(&project.id, "other", Search::default())
            .await
            .is_err()
    );
    Ok(())
}
