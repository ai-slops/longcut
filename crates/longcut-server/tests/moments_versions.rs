mod moments_support;

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use longcut_server::{
    domain,
    model::{Command, CommandRequest, CreateProject, MomentNarrativeRole, Project},
};
use moments_support::{command, fixture, router};
use tower::ServiceExt;

fn create() -> Command {
    Command::CreateMoment {
        source_id: "source".into(),
        start_frame: "30".into(),
        end_frame: "90".into(),
        label: "Private".into(),
        note: String::new(),
        narrative_role: MomentNarrativeRole::Setup,
        evidence: None,
    }
}

#[tokio::test]
async fn new_database_uses_schema_version_eight() {
    // Given
    let fixture = fixture().await;
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture._root.path().join("longcut.sqlite3")),
    )
    .await
    .expect("open database");
    // When
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&pool)
        .await
        .expect("schema version");
    // Then
    assert_eq!(version, 8);
}

#[tokio::test]
async fn moment_version_read_is_project_scoped() {
    // Given
    let fixture = fixture().await;
    let created = command(&fixture, &fixture.project.revision, create()).await;
    let other = fixture
        .state
        .store
        .create(CreateProject {
            title: "Other".into(),
            frame_rate: 30,
        })
        .await
        .expect("other project");
    // When
    let response = router(&fixture)
        .oneshot(
            Request::get(format!(
                "/api/v1/projects/{}/moments/{}/versions/{}",
                other.id, created.moments[0].id, created.moments[0].version
            ))
            .body(Body::empty())
            .expect("build request"),
        )
        .await
        .expect("route request");
    // Then
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn legacy_project_and_scene_json_default_new_collections_and_links() {
    // Given
    let fixture = fixture().await;
    let mut value = serde_json::to_value(&fixture.project).expect("serialize project");
    value
        .as_object_mut()
        .expect("project object")
        .remove("moments");
    value["scenes"] = serde_json::json!([{"id":"legacy","title":"Legacy","purpose":"","source_id":"source","in_frame":"0","out_frame":"10","camera":null,"caption":"","locked":false,"manual":true}]);
    // When
    let legacy: Project = serde_json::from_value(value).expect("read legacy project");
    // Then
    assert!(legacy.moments.is_empty());
    assert!(legacy.scenes[0].moment.is_none());
}

#[tokio::test]
async fn duplicate_moment_version_fails_without_overwriting_immutable_snapshot() {
    // Given
    let fixture = fixture().await;
    let created = command(&fixture, &fixture.project.revision, create()).await;
    let moment = &created.moments[0];
    let duplicate_version = domain::next_revision(&created.revision).expect("next revision");
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(fixture._root.path().join("longcut.sqlite3")),
    )
    .await
    .expect("open database");
    sqlx::query(
        "INSERT INTO moment_versions(project_id,moment_id,version,snapshot) VALUES(?,?,?,?)",
    )
    .bind(&created.id)
    .bind(&moment.id)
    .bind(&duplicate_version)
    .bind("immutable sentinel")
    .execute(&pool)
    .await
    .expect("seed duplicate version");

    // When
    let result = fixture
        .state
        .store
        .command(
            &created.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: created.revision.clone(),
                command: Command::UpdateMoment {
                    moment_id: moment.id.clone(),
                    expected_moment_version: moment.version.clone(),
                    source_id: "source".into(),
                    start_frame: "40".into(),
                    end_frame: "100".into(),
                    label: "Changed".into(),
                    note: String::new(),
                    narrative_role: MomentNarrativeRole::Insight,
                    evidence: None,
                },
            },
        )
        .await;
    let snapshot: String = sqlx::query_scalar(
        "SELECT snapshot FROM moment_versions WHERE project_id=? AND moment_id=? AND version=?",
    )
    .bind(&created.id)
    .bind(&moment.id)
    .bind(&duplicate_version)
    .fetch_one(&pool)
    .await
    .expect("read duplicate version");

    // Then
    assert!(result.is_err());
    assert_eq!(snapshot, "immutable sentinel");
}
