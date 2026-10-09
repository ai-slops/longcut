mod history_support;

use history_support::{fixture, pool};
use longcut_server::model::{Command, CommandRequest};

#[tokio::test]
async fn revision_overflow_does_not_mutate_the_database() {
    // Given
    let mut fixture = fixture().await;
    fixture.project.revision = u64::MAX.to_string();
    let db = pool(&fixture.path).await;
    sqlx::query("UPDATE projects SET state=? WHERE id=?")
        .bind(serde_json::to_string(&fixture.project).expect("serialize project"))
        .bind(&fixture.project.id)
        .execute(&db)
        .await
        .expect("seed maximum revision");
    db.close().await;

    // When
    let result = fixture
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: "overflow".into(),
                base_revision: fixture.project.revision.clone(),
                command: Command::RenameProject {
                    title: "Must not persist".into(),
                },
            },
        )
        .await;

    // Then
    assert!(result.is_err());
    let unchanged = fixture
        .store
        .get(&fixture.project.id)
        .await
        .expect("read project");
    assert_eq!(unchanged.revision, u64::MAX.to_string());
    assert_eq!(unchanged.title, "History");
    let db = pool(&fixture.path).await;
    let command_count: i64 = sqlx::query_scalar("SELECT count(*) FROM commands WHERE project_id=?")
        .bind(&fixture.project.id)
        .fetch_one(&db)
        .await
        .expect("command count");
    assert_eq!(command_count, 0);
}
