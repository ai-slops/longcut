mod history_support;

use history_support::{fixture, moment, pool};
use longcut_server::{
    error::AppError,
    model::{Command, CommandRequest, History},
};

#[tokio::test]
async fn legacy_compaction_inserts_pending_key_among_many_unrelated_versions() {
    // Given
    let mut fixture = fixture().await;
    fixture.project.revision = "2".into();
    let mut previous = fixture.project.clone();
    previous.revision = "1".into();
    previous.moments = vec![moment(0, "1")];
    let history = History {
        undo: vec![previous],
        redo: vec![],
    };
    let db = pool(&fixture.path).await;
    sqlx::query("UPDATE projects SET state=?,history=? WHERE id=?")
        .bind(serde_json::to_string(&fixture.project).expect("serialize state"))
        .bind(serde_json::to_string(&history).expect("serialize history"))
        .bind(&fixture.project.id)
        .execute(&db)
        .await
        .expect("seed legacy history");
    sqlx::query(
        "WITH RECURSIVE keys(value) AS (SELECT 1 UNION ALL SELECT value+1 FROM keys WHERE value<2000) INSERT INTO moment_versions(project_id,moment_id,version,snapshot) SELECT ?,printf('unrelated-%04d',value),'1',? FROM keys",
    )
    .bind(&fixture.project.id)
    .bind(serde_json::to_string(&moment(9_999, "1")).expect("serialize unrelated snapshot"))
    .execute(&db)
    .await
    .expect("seed unrelated versions");
    db.close().await;

    // When
    fixture
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: "compact-with-unrelated-versions".into(),
                base_revision: "2".into(),
                command: Command::RenameProject {
                    title: "Compacted".into(),
                },
            },
        )
        .await
        .expect("compact history");

    // Then
    let db = pool(&fixture.path).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM moment_versions WHERE project_id=?")
        .bind(&fixture.project.id)
        .fetch_one(&db)
        .await
        .expect("version count");
    let target: String = sqlx::query_scalar(
        "SELECT snapshot FROM moment_versions WHERE project_id=? AND moment_id='moment-000' AND version='1'",
    )
    .bind(&fixture.project.id)
    .fetch_one(&db)
    .await
    .expect("pending version");
    assert_eq!(count, 2_001);
    assert_eq!(
        serde_json::from_str::<longcut_server::model::Moment>(&target)
            .expect("parse pending version"),
        moment(0, "1")
    );
}

#[tokio::test]
async fn conflicting_legacy_version_fails_without_mutating_project_or_journal() {
    // Given
    let mut fixture = fixture().await;
    fixture.project.revision = "2".into();
    let mut previous = fixture.project.clone();
    previous.revision = "1".into();
    previous.moments = vec![moment(0, "1")];
    let history = History {
        undo: vec![previous],
        redo: vec![],
    };
    let mut conflicting = moment(0, "1");
    conflicting.label = "Conflicting immutable payload".into();
    let db = pool(&fixture.path).await;
    sqlx::query("UPDATE projects SET state=?,history=? WHERE id=?")
        .bind(serde_json::to_string(&fixture.project).expect("serialize state"))
        .bind(serde_json::to_string(&history).expect("serialize history"))
        .bind(&fixture.project.id)
        .execute(&db)
        .await
        .expect("seed legacy history");
    sqlx::query(
        "INSERT INTO moment_versions(project_id,moment_id,version,snapshot) VALUES(?,?,?,?)",
    )
    .bind(&fixture.project.id)
    .bind(&conflicting.id)
    .bind(&conflicting.version)
    .bind(serde_json::to_string(&conflicting).expect("serialize conflict"))
    .execute(&db)
    .await
    .expect("seed conflicting version");
    let before: (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT CAST(state AS BLOB),CAST(history AS BLOB) FROM projects WHERE id=?")
            .bind(&fixture.project.id)
            .fetch_one(&db)
            .await
            .expect("snapshot project bytes");
    db.close().await;

    // When
    let result = fixture
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: "conflicting-version-undo".into(),
                base_revision: "2".into(),
                command: Command::Undo,
            },
        )
        .await;

    // Then
    assert!(matches!(result, Err(AppError::Conflict(_))));
    let db = pool(&fixture.path).await;
    let after: (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT CAST(state AS BLOB),CAST(history AS BLOB) FROM projects WHERE id=?")
            .bind(&fixture.project.id)
            .fetch_one(&db)
            .await
            .expect("read project bytes");
    let command_count: i64 = sqlx::query_scalar("SELECT count(*) FROM commands WHERE project_id=?")
        .bind(&fixture.project.id)
        .fetch_one(&db)
        .await
        .expect("command count");
    assert_eq!(after, before);
    assert_eq!(command_count, 0);
}
