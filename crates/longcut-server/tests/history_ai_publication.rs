mod history_support;

use history_support::{fixture, pool, seed_project, visual_source};
use longcut_server::{
    ai::{AiOperation, AiProposal, AiProvider, AiRequest, AiRunStatus, AiScope, validate_proposal},
    model::{Command, CommandRequest},
};

#[tokio::test]
async fn successful_ai_publication_compacts_moment_history_and_cached_result() {
    // Given
    let mut fixture = fixture().await;
    fixture.project = fixture
        .store
        .add_source(&fixture.project.id, visual_source())
        .await
        .expect("add source");
    let revision = fixture.project.revision.clone();
    seed_project(&mut fixture, 1, &revision).await;
    let run = fixture
        .store
        .enqueue_ai(
            &fixture.project.id,
            AiRequest {
                provider: AiProvider::Codex,
                base_revision: revision,
                instruction: "Create an opening".into(),
                consent: true,
                scope: AiScope::Source {
                    source_id: "source".into(),
                    start_frame: "0".into(),
                    end_frame: "120".into(),
                },
            },
        )
        .await
        .expect("enqueue AI run");
    fixture
        .store
        .transition_ai(&run.id, AiRunStatus::Sampling, 0)
        .await
        .expect("begin sampling");
    let run = fixture
        .store
        .transition_ai(&run.id, AiRunStatus::Generating, 1)
        .await
        .expect("begin generation");
    let proposal = AiProposal {
        title: "Opening proposal".into(),
        summary: "One source-backed opening".into(),
        warnings: vec![],
        operations: vec![AiOperation::InsertScene {
            source_id: "source".into(),
            in_frame: "0".into(),
            out_frame: "60".into(),
            title: "Opening".into(),
            rationale: "Use the selected opening range".into(),
            evidence_ids: vec!["source_source".into()],
        }],
    };
    validate_proposal(&fixture.project, &run.scope, &[], &proposal).expect("valid proposal");

    // When
    let published = fixture
        .store
        .publish_ai(&run.id, proposal)
        .await
        .expect("publish AI proposal");
    let db = pool(&fixture.path).await;
    let (request_json, result_json): (String, String) =
        sqlx::query_as("SELECT request,result FROM commands WHERE project_id=? AND command_id=?")
            .bind(&fixture.project.id)
            .bind(&run.id)
            .fetch_one(&db)
            .await
            .expect("AI command journal");
    let history_json: String = sqlx::query_scalar("SELECT history FROM projects WHERE id=?")
        .bind(&fixture.project.id)
        .fetch_one(&db)
        .await
        .expect("project history");
    db.close().await;
    let cached_request: CommandRequest =
        serde_json::from_str(&request_json).expect("parse cached request");
    let cached = fixture
        .store
        .command(&fixture.project.id, &cached_request)
        .await
        .expect("replay AI command");
    let undone = fixture
        .store
        .command(
            &fixture.project.id,
            &CommandRequest {
                command_id: "undo-ai-publication".into(),
                base_revision: cached.revision.clone(),
                command: Command::Undo,
            },
        )
        .await
        .expect("undo AI publication");

    // Then
    assert_eq!(published.status, AiRunStatus::Ready);
    assert!(published.change_set_id.is_some());
    assert_eq!(cached.change_sets.len(), 1);
    assert_eq!(cached.moments, fixture.project.moments);
    assert!(history_json.contains("project_without_moments"));
    assert!(result_json.contains("project_without_moments"));
    assert!(!history_json.contains(&"q".repeat(1_000)));
    assert!(!result_json.contains(&"q".repeat(1_000)));
    assert!(undone.change_sets.is_empty());
    assert_eq!(undone.moments, fixture.project.moments);
}
