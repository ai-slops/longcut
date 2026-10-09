mod history_support;

use history_support::{fixture, pool, seed_project};
use longcut_server::{
    error::AppError,
    model::{Command, CommandRequest},
};

#[tokio::test]
async fn undo_with_missing_immutable_version_leaves_state_history_and_journal_unchanged() {
    // Given
    let mut fixture = fixture().await;
    seed_project(&mut fixture, 1, "1").await;
    let edited = fixture
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: "create-compact-history".into(),
                base_revision: "1".into(),
                command: Command::RenameProject {
                    title: "Edited".into(),
                },
            },
        )
        .await
        .expect("create compact history");
    let db = pool(&fixture.path).await;
    sqlx::query("DELETE FROM moment_versions WHERE project_id=? AND moment_id=? AND version='1'")
        .bind(&fixture.project.id)
        .bind(&fixture.project.moments[0].id)
        .execute(&db)
        .await
        .expect("delete immutable version");
    let before: (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT CAST(state AS BLOB),CAST(history AS BLOB) FROM projects WHERE id=?")
            .bind(&fixture.project.id)
            .fetch_one(&db)
            .await
            .expect("snapshot project bytes");
    let journal_before: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT command_id,request,result FROM commands WHERE project_id=? ORDER BY command_id",
    )
    .bind(&fixture.project.id)
    .fetch_all(&db)
    .await
    .expect("snapshot journal");
    db.close().await;

    // When
    let result = fixture
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: "missing-version-undo".into(),
                base_revision: edited.revision,
                command: Command::Undo,
            },
        )
        .await;

    // Then
    assert!(matches!(result, Err(AppError::NotFound)));
    let db = pool(&fixture.path).await;
    let after: (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT CAST(state AS BLOB),CAST(history AS BLOB) FROM projects WHERE id=?")
            .bind(&fixture.project.id)
            .fetch_one(&db)
            .await
            .expect("read project bytes");
    let journal_after: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT command_id,request,result FROM commands WHERE project_id=? ORDER BY command_id",
    )
    .bind(&fixture.project.id)
    .fetch_all(&db)
    .await
    .expect("read journal");
    let version_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM moment_versions WHERE project_id=? AND moment_id=? AND version='1'",
    )
    .bind(&fixture.project.id)
    .bind(&fixture.project.moments[0].id)
    .fetch_one(&db)
    .await
    .expect("version count");
    assert_eq!(after, before);
    assert_eq!(journal_after, journal_before);
    assert_eq!(version_count, 0);
    assert!(
        journal_after
            .iter()
            .all(|(command_id, _, _)| command_id != "missing-version-undo")
    );
}
