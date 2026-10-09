mod history_support;

use history_support::{fixture, pool, seed_project};
use longcut_server::model::{Command, CommandRequest};

#[tokio::test]
async fn five_hundred_maximum_moment_heads_keep_history_and_results_bounded() {
    // Given
    let mut fixture = fixture().await;
    seed_project(&mut fixture, 500, "1").await;
    let mut revision = "1".to_owned();
    let first_request = CommandRequest {
        command_id: "edit-000".into(),
        base_revision: revision.clone(),
        command: Command::RenameProject {
            title: "Edit 0".into(),
        },
    };

    // When
    let first = fixture
        .store
        .command(&fixture.project.id, &first_request)
        .await
        .expect("first edit");
    revision.clone_from(&first.revision);
    for index in 1..25 {
        let edited = fixture
            .store
            .command(
                &fixture.project.id,
                &CommandRequest {
                    command_id: format!("edit-{index:03}"),
                    base_revision: revision,
                    command: Command::RenameProject {
                        title: format!("Edit {index}"),
                    },
                },
            )
            .await
            .expect("edit project");
        revision = edited.revision;
    }
    let replayed = fixture
        .store
        .command(&fixture.project.id, &first_request)
        .await
        .expect("replay command");

    // Then
    assert_eq!(replayed.moments, first.moments);
    assert_eq!(replayed.title, "Edit 0");
    let pool = pool(&fixture.path).await;
    let history_bytes: i64 = sqlx::query_scalar("SELECT length(history) FROM projects WHERE id=?")
        .bind(&fixture.project.id)
        .fetch_one(&pool)
        .await
        .expect("history size");
    let result_bytes: i64 =
        sqlx::query_scalar("SELECT sum(length(result)) FROM commands WHERE project_id=?")
            .bind(&fixture.project.id)
            .fetch_one(&pool)
            .await
            .expect("result size");
    let version_bytes: i64 =
        sqlx::query_scalar("SELECT sum(length(snapshot)) FROM moment_versions WHERE project_id=?")
            .bind(&fixture.project.id)
            .fetch_one(&pool)
            .await
            .expect("version size");
    println!(
        "storage bytes: history={history_bytes}, idempotency_results={result_bytes}, immutable_versions={version_bytes}"
    );
    assert!(history_bytes < 2_000_000, "history bytes: {history_bytes}");
    assert!(result_bytes < 2_000_000, "result bytes: {result_bytes}");
    assert!(version_bytes < 12_000_000, "version bytes: {version_bytes}");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM moment_versions WHERE project_id=?")
            .bind(&fixture.project.id)
            .fetch_one(&pool)
            .await
            .expect("version count"),
        500
    );
}
