use longcut_server::{
    AppState,
    ai::{AiProposal, AiProvider, AiRequest, AiRunStatus, AiScope},
    model::{Command, CommandRequest, CreateProject, MediaStatus, Source, SourceRole},
};
use tempfile::TempDir;

async fn project_with_source() -> (TempDir, AppState, longcut_server::model::Project) {
    let data = TempDir::new().expect("create temp directory");
    let state = AppState::initialize(data.path())
        .await
        .expect("initialize state");
    let project = state
        .store
        .create(CreateProject {
            title: "AI test".into(),
            frame_rate: 30,
        })
        .await
        .expect("create project");
    let source = Source {
        id: "source".into(),
        name: "source.mp4".into(),
        role: SourceRole::Program,
        duration_frames: "180".into(),
        width: 1280,
        height: 720,
        has_audio: false,
        media_url: "/api/v1/media/source-original.mp4".into(),
        proxy_url: None,
        thumbnail_url: None,
        waveform: vec![],
        status: MediaStatus::Ready,
        error: None,
    };
    let project = state
        .store
        .add_source(&project.id, source)
        .await
        .expect("add source");
    (data, state, project)
}

fn source_request(revision: &str) -> AiRequest {
    AiRequest {
        provider: AiProvider::Codex,
        base_revision: revision.into(),
        instruction: "Create a concise opening sequence".into(),
        consent: true,
        scope: AiScope::Source {
            source_id: "source".into(),
            start_frame: "0".into(),
            end_frame: "120".into(),
        },
    }
}

#[tokio::test]
async fn cancelled_run_remains_cancelled_after_late_worker_update() {
    // Given
    let (_data, state, project) = project_with_source().await;
    let run = state
        .store
        .enqueue_ai(&project.id, source_request(&project.revision))
        .await
        .expect("enqueue run");
    // When
    let cancelled = state
        .store
        .cancel_ai(&project.id, &run.id)
        .await
        .expect("cancel run");
    let updated = state
        .store
        .transition_ai(&run.id, AiRunStatus::Generating, 4)
        .await;
    // Then
    assert_eq!(cancelled.status, AiRunStatus::Cancelled);
    assert!(
        updated.is_err(),
        "late workers must not revive cancellation"
    );
    assert_eq!(
        state
            .store
            .ai_run(&project.id, &run.id)
            .await
            .expect("read run")
            .status,
        AiRunStatus::Cancelled
    );
}

#[tokio::test]
async fn restart_marks_interrupted_ai_run_as_actionable_error() {
    // Given
    let (data, state, project) = project_with_source().await;
    let run = state
        .store
        .enqueue_ai(&project.id, source_request(&project.revision))
        .await
        .expect("enqueue run");
    drop(state);
    // When
    let reopened = AppState::initialize(data.path())
        .await
        .expect("reopen state");
    let recovered = reopened
        .store
        .ai_run(&project.id, &run.id)
        .await
        .expect("read recovered run");
    // Then
    assert_eq!(recovered.status, AiRunStatus::Error);
    assert!(
        recovered
            .error
            .as_deref()
            .is_some_and(|message| message.contains("interrupted") && message.contains("retry"))
    );
}

#[tokio::test]
async fn stale_generation_cannot_publish_a_change_set() {
    // Given
    let (_data, state, project) = project_with_source().await;
    let run = state
        .store
        .enqueue_ai(&project.id, source_request(&project.revision))
        .await
        .expect("enqueue run");
    state
        .store
        .transition_ai(&run.id, AiRunStatus::Sampling, 0)
        .await
        .expect("begin sampling");
    state
        .store
        .transition_ai(&run.id, AiRunStatus::Generating, 1)
        .await
        .expect("begin generation");
    state
        .store
        .command(
            &project.id,
            &CommandRequest {
                command_id: uuid::Uuid::new_v4().to_string(),
                base_revision: project.revision,
                command: Command::SetBrief {
                    brief: "manual edit".into(),
                },
            },
        )
        .await
        .expect("manual edit");
    // When
    let published = state
        .store
        .publish_ai(
            &run.id,
            AiProposal {
                title: "Proposal".into(),
                summary: "Summary".into(),
                warnings: vec![],
                operations: vec![],
            },
        )
        .await
        .expect("mark stale");
    // Then
    assert_eq!(published.status, AiRunStatus::Stale);
    let current = state.store.get(&project.id).await.expect("read project");
    assert!(current.change_sets.is_empty());
    assert_eq!(current.brief, "manual edit");
}

#[tokio::test]
async fn invalid_scope_is_rejected_before_a_run_is_persisted() {
    // Given
    let (_data, state, project) = project_with_source().await;
    let mut request = source_request(&project.revision);
    request.scope = AiScope::Source {
        source_id: "source".into(),
        start_frame: "0".into(),
        end_frame: "181".into(),
    };
    // When
    let result = state.store.enqueue_ai(&project.id, request).await;
    // Then
    assert!(result.is_err());
    assert!(
        state
            .store
            .list_ai_runs(&project.id)
            .await
            .expect("list runs")
            .is_empty()
    );
}

#[tokio::test]
async fn missing_consent_is_rejected_before_a_run_is_persisted() {
    // Given
    let (_data, state, project) = project_with_source().await;
    let mut request = source_request(&project.revision);
    request.consent = false;
    // When
    let result = state.store.enqueue_ai(&project.id, request).await;
    // Then
    assert!(result.is_err());
    assert!(
        state
            .store
            .list_ai_runs(&project.id)
            .await
            .expect("list runs")
            .is_empty()
    );
}

#[test]
fn missing_provider_defaults_to_codex_for_stored_requests_and_runs() {
    // Given
    let request = serde_json::json!({
        "base_revision": "0",
        "instruction": "Keep compatibility",
        "consent": true,
        "scope": {"type": "scenes", "scene_ids": ["scene"]}
    });
    let run = serde_json::json!({
        "id": "run",
        "project_id": "project",
        "base_revision": "0",
        "instruction": "Keep compatibility",
        "scope": {"type": "scenes", "scene_ids": ["scene"]},
        "status": "ready",
        "created_at": "2026-09-20T00:00:00Z",
        "updated_at": "2026-09-20T00:00:00Z",
        "change_set_id": null,
        "summary": null,
        "warnings": [],
        "sample_count": 0,
        "error": null
    });
    // When
    let request: AiRequest = serde_json::from_value(request).expect("legacy request");
    let run: longcut_server::ai::AiRun = serde_json::from_value(run).expect("legacy run");
    // Then
    assert_eq!(request.provider, AiProvider::Codex);
    assert_eq!(run.provider, AiProvider::Codex);
}
