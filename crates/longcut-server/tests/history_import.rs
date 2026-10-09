mod history_support;

use history_support::{fixture, pool, seed_project};
use longcut_server::model::*;
use longcut_session_contract::Conformance;

fn prepared(job: &SessionImportJob) -> PreparedImport {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/edit-session-packages/complete");
    let (package, _) = longcut_session_contract::load(&root)
        .expect("load package")
        .into_parts();
    PreparedImport {
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
            id: "imported-source".into(),
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
    }
}

#[tokio::test]
async fn import_after_moment_history_remains_exactly_undoable() {
    // Given
    let mut fixture = fixture().await;
    seed_project(&mut fixture, 1, "1").await;
    let edited = fixture
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: "before-import".into(),
                base_revision: "1".into(),
                command: Command::RenameProject {
                    title: "Before import".into(),
                },
            },
        )
        .await
        .expect("create moment history");
    let job = fixture
        .store
        .create_import(&fixture.project.id)
        .await
        .expect("create import");
    let prepared = prepared(&job);
    fixture
        .store
        .prepare_import(&job, &prepared)
        .await
        .expect("prepare import");
    let job = fixture
        .store
        .transition_import(&fixture.project.id, &job.id, ImportStatus::Publishing, None)
        .await
        .expect("publish import");

    // When
    let imported = fixture
        .store
        .commit_import(&job, &prepared, &edited.revision)
        .await
        .expect("commit import");
    let undone = fixture
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: "undo-import".into(),
                base_revision: imported.revision,
                command: Command::Undo,
            },
        )
        .await
        .expect("undo import");

    // Then
    assert_eq!(undone.moments, edited.moments);
    assert_eq!(undone.title, edited.title);
    assert!(undone.sources.is_empty());
    assert!(undone.source_sessions.is_empty());
    let db = pool(&fixture.path).await;
    let result: String =
        sqlx::query_scalar("SELECT result FROM commands WHERE project_id=? AND command_id=?")
            .bind(&fixture.project.id)
            .bind(format!("session-import:{}", job.id))
            .fetch_one(&db)
            .await
            .expect("import command result");
    assert!(result.contains("project_without_moments"));
}
