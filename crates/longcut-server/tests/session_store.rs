use longcut_server::{error::AppError, model::*, store::Store};
use longcut_session_contract::Conformance;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn prepared(job: &SessionImportJob) -> Result<PreparedImport, Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/edit-session-packages/complete");
    let (package, _) = longcut_session_contract::load(&root)
        .map_err(|_| "fixture metadata")?
        .into_parts();
    Ok(PreparedImport {
        record: SourceSessionRecord {
            summary: SourceSessionSummary {
                id: job.id.clone(),
                session_id: package.manifest.session_id.clone(),
                title: "Imported".into(),
                duration_ns: "10000000000".into(),
                conformance: Conformance::Conformant,
                imported_at: job.created_at.clone(),
                assets: vec![],
                sources: vec![],
                warnings: vec![],
                verification_url: None,
            },
            package,
            mappings: vec![],
        },
        sources: vec![Source {
            id: "editor-source".into(),
            name: "original".into(),
            role: SourceRole::Program,
            duration_frames: "300".into(),
            width: 160,
            height: 90,
            has_audio: true,
            media_url: "/api/v1/media/example.mp4".into(),
            proxy_url: None,
            thumbnail_url: None,
            waveform: vec![],
            status: MediaStatus::Ready,
            error: None,
        }],
        artifacts: vec![],
    })
}

#[tokio::test]
async fn session_import_commits_one_undo_step_and_retains_original_provenance() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let store = Store::open(&root.path().join("project.sqlite3")).await?;
    let project = store
        .create(CreateProject {
            title: "test".into(),
            frame_rate: 30,
        })
        .await?;
    let job = store.create_import(&project.id).await?;
    let prepared = prepared(&job)?;
    store.prepare_import(&job, &prepared).await?;
    let job = store
        .transition_import(&project.id, &job.id, ImportStatus::Publishing, None)
        .await?;
    // When
    let imported = store.commit_import(&job, &prepared, "0").await?;
    // Then
    assert_eq!(imported.revision, "1");
    assert_eq!(imported.sources.len(), 1);
    assert_eq!(imported.source_sessions.len(), 1);
    let undone = store
        .command(
            &project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: "1".into(),
                command: Command::Undo,
            },
        )
        .await?;
    assert!(undone.sources.is_empty());
    assert!(undone.source_sessions.is_empty());
    let restored = store
        .command(
            &project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: "2".into(),
                command: Command::Redo,
            },
        )
        .await?;
    assert_eq!(restored.source_sessions.len(), 1);
    let session = store.source_session(&project.id, &job.id).await?;
    assert_eq!(session.package.registry.sources.len(), 3);
    assert_eq!(
        store.get_import(&project.id, &job.id).await?.status,
        ImportStatus::Confirmed
    );
    Ok(())
}

#[tokio::test]
async fn stale_import_cannot_partially_register_sources_or_session() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let store = Store::open(&root.path().join("project.sqlite3")).await?;
    let project = store
        .create(CreateProject {
            title: "test".into(),
            frame_rate: 30,
        })
        .await?;
    let job = store.create_import(&project.id).await?;
    let prepared = prepared(&job)?;
    store.prepare_import(&job, &prepared).await?;
    let job = store
        .transition_import(&project.id, &job.id, ImportStatus::Publishing, None)
        .await?;
    // When
    let result = store.commit_import(&job, &prepared, "99").await;
    // Then
    assert!(matches!(result, Err(AppError::Stale)));
    let project = store.get(&project.id).await?;
    assert_eq!(project.revision, "0");
    assert!(project.sources.is_empty() && project.source_sessions.is_empty());
    assert!(matches!(
        store.source_session(&project.id, &job.id).await,
        Err(AppError::NotFound)
    ));
    Ok(())
}

#[tokio::test]
async fn frozen_render_keeps_session_provenance_after_import_undo() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let store = Store::open(&root.path().join("project.sqlite3")).await?;
    let project = store
        .create(CreateProject {
            title: "test".into(),
            frame_rate: 30,
        })
        .await?;
    let job = store.create_import(&project.id).await?;
    let prepared = prepared(&job)?;
    store.prepare_import(&job, &prepared).await?;
    let job = store
        .transition_import(&project.id, &job.id, ImportStatus::Publishing, None)
        .await?;
    store.commit_import(&job, &prepared, "0").await?;
    let render = Render {
        id: uuid::Uuid::new_v4().to_string(),
        revision: "1".into(),
        kind: RenderKind::Preview,
        status: RenderStatus::Queued,
        progress: 0.0,
        media_url: None,
        srt_url: None,
        vtt_url: None,
        lineage_url: None,
        error: None,
    };
    store
        .enqueue_render(&project.id, render.clone(), "1")
        .await?;
    // When
    store
        .command(
            &project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: "1".into(),
                command: Command::Undo,
            },
        )
        .await?;
    // Then
    assert!(matches!(
        store.source_session(&project.id, &job.id).await,
        Err(AppError::NotFound)
    ));
    assert_eq!(
        store
            .render_source_session(&project.id, &render.id, &job.id)
            .await?
            .summary
            .id,
        job.id
    );
    assert!(matches!(
        store
            .render_source_session("other-project", &render.id, &job.id)
            .await,
        Err(AppError::NotFound)
    ));
    Ok(())
}

#[tokio::test]
async fn v0_database_is_backed_up_and_old_project_json_remains_readable() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let path = root.path().join("project.sqlite3");
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename(&path)
                .create_if_missing(true),
        )
        .await?;
    sqlx::query("CREATE TABLE projects(id TEXT PRIMARY KEY,state TEXT NOT NULL,history TEXT NOT NULL,updated_at TEXT NOT NULL)").execute(&pool).await?;
    let state = serde_json::json!({"id":"old", "title":"Old", "revision":"0", "frame_rate":30, "updated_at":"", "brief":"", "sources":[], "scenes":[], "feedback":[], "change_sets":[], "renders":[], "can_undo":false, "can_redo":false});
    sqlx::query("INSERT INTO projects VALUES('old',?,'{\"undo\":[],\"redo\":[]}','')")
        .bind(state.to_string())
        .execute(&pool)
        .await?;
    pool.close().await;
    // When
    let store = Store::open(&path).await?;
    // Then
    assert!(store.get("old").await?.source_sessions.is_empty());
    assert!(
        std::fs::read_dir(root.path())?
            .filter_map(Result::ok)
            .any(|entry| entry
                .file_name()
                .to_string_lossy()
                .starts_with("project.before-sessions-"))
    );
    Ok(())
}
