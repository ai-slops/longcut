mod history_support;

use history_support::{fixture, moment, pool, seed_project};
use longcut_server::{
    error::AppError,
    model::{Command, CommandRequest, History},
};

#[tokio::test]
async fn legacy_inline_history_is_restored_exactly_and_compacted_lazily() {
    // Given
    let mut fixture = fixture().await;
    seed_project(&mut fixture, 1, "2").await;
    let mut previous = fixture.project.clone();
    previous.revision = "1".into();
    previous.title = "Before".into();
    previous.moments = vec![moment(0, "1")];
    let legacy = History {
        undo: vec![previous.clone()],
        redo: vec![],
    };
    let db = pool(&fixture.path).await;
    sqlx::query("UPDATE projects SET history=? WHERE id=?")
        .bind(serde_json::to_string(&legacy).expect("serialize legacy history"))
        .bind(&fixture.project.id)
        .execute(&db)
        .await
        .expect("seed legacy history");
    db.close().await;

    // When
    let request = CommandRequest {
        command_id: "legacy-undo".into(),
        base_revision: "2".into(),
        command: Command::Undo,
    };
    let restored = fixture
        .store
        .command(&fixture.project.id, &request)
        .await
        .expect("undo legacy snapshot");

    // Then
    assert_eq!(restored.title, "Before");
    assert_eq!(restored.moments, previous.moments);
    assert_eq!(
        fixture
            .store
            .command(&fixture.project.id, &request)
            .await
            .expect("replay legacy undo")
            .moments,
        previous.moments
    );
    let db = pool(&fixture.path).await;
    let history: String = sqlx::query_scalar("SELECT history FROM projects WHERE id=?")
        .bind(&fixture.project.id)
        .fetch_one(&db)
        .await
        .expect("stored history");
    assert!(history.contains("project_without_moments"));
    assert!(!history.contains(&"q".repeat(1_000)));
}

#[tokio::test]
async fn legacy_idempotency_result_is_returned_exactly_and_compacted_lazily() {
    // Given
    let mut fixture = fixture().await;
    seed_project(&mut fixture, 1, "1").await;
    let request = CommandRequest {
        command_id: "legacy-result".into(),
        base_revision: "0".into(),
        command: Command::RenameProject {
            title: "Legacy result".into(),
        },
    };
    let db = pool(&fixture.path).await;
    sqlx::query("INSERT INTO commands(project_id,command_id,request,result) VALUES(?,?,?,?)")
        .bind(&fixture.project.id)
        .bind(&request.command_id)
        .bind(serde_json::to_string(&request).expect("serialize request"))
        .bind(serde_json::to_string(&fixture.project).expect("serialize result"))
        .execute(&db)
        .await
        .expect("seed command");
    db.close().await;

    // When
    let replayed = fixture
        .store
        .command(&fixture.project.id, &request)
        .await
        .expect("replay legacy result");

    // Then
    assert_eq!(replayed.moments, fixture.project.moments);
    let db = pool(&fixture.path).await;
    let result: String =
        sqlx::query_scalar("SELECT result FROM commands WHERE project_id=? AND command_id=?")
            .bind(&fixture.project.id)
            .bind(&request.command_id)
            .fetch_one(&db)
            .await
            .expect("stored result");
    assert!(result.contains("project_without_moments"));
}

#[tokio::test]
async fn compact_result_fails_when_exact_project_scoped_version_is_missing() {
    // Given
    let mut fixture = fixture().await;
    seed_project(&mut fixture, 1, "1").await;
    let request = CommandRequest {
        command_id: "scoped-result".into(),
        base_revision: "1".into(),
        command: Command::RenameProject {
            title: "Stored".into(),
        },
    };
    fixture
        .store
        .command(&fixture.project.id, &request)
        .await
        .expect("store compact result");
    let db = pool(&fixture.path).await;
    sqlx::query("DELETE FROM moment_versions WHERE project_id=? AND moment_id=? AND version='1'")
        .bind(&fixture.project.id)
        .bind(&fixture.project.moments[0].id)
        .execute(&db)
        .await
        .expect("remove exact version");
    sqlx::query("INSERT INTO moment_versions(project_id,moment_id,version,snapshot) VALUES('other',?,'1',?)")
        .bind(&fixture.project.moments[0].id)
        .bind(serde_json::to_string(&fixture.project.moments[0]).expect("serialize moment"))
        .execute(&db)
        .await
        .expect("seed other project version");
    db.close().await;

    // When
    let result = fixture.store.command(&fixture.project.id, &request).await;

    // Then
    assert!(matches!(result, Err(AppError::NotFound)));
}
