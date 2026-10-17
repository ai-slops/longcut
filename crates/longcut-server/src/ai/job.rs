use std::time::Duration;

use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::{AppState, error::AppError};

use super::{
    AiRun, AiRunStatus, build_prompt, proposal_schema, sampling, validate_proposal,
    validate_request,
};

pub async fn execute(state: AppState, run: AiRun, cancellation: CancellationToken) {
    let result = timeout(
        Duration::from_secs(10 * 60),
        execute_inner(&state, &run, &cancellation),
    )
    .await;
    let failure = match result {
        Ok(Ok(())) => None,
        Ok(Err(error)) => Some(error.to_string()),
        Err(_) => {
            let _result = state.agent_runtime.cancel_for(run.provider, &run.id).await;
            Some("AI run timed out after ten minutes".into())
        }
    };
    if let Some(message) = failure
        && let Err(error) = state.store.fail_ai(&run.id, message).await
    {
        tracing::error!(run_id = %run.id, error = ?error, "ai.persist_failure_failed");
    }
    state.finish_ai_work(&run.id).await;
}

async fn execute_inner(
    state: &AppState,
    run: &AiRun,
    cancellation: &CancellationToken,
) -> crate::error::AppResult<()> {
    state
        .store
        .transition_ai(&run.id, AiRunStatus::Sampling, 0)
        .await?;
    let request = state.store.ai_request(&run.id).await?;
    let project = state.store.get(&run.project_id).await?;
    if let Err(error) = validate_request(&project, &request) {
        if matches!(error, AppError::Stale) {
            tokio::select! {
                biased;
                () = cancellation.cancelled() => {
                    return Err(AppError::Conflict("AI run was cancelled".into()));
                }
                result = state.store.stale_ai(&run.id) => {
                    result?;
                }
            }
            return Ok(());
        }
        return Err(error);
    }
    let samples =
        sampling::sample_cancellable(&state.artifacts, &project, &request.scope, cancellation)
            .await?;
    let sample_count = u8::try_from(samples.len())
        .map_err(|error| crate::error::AppError::Internal(error.into()))?;
    state
        .store
        .transition_ai(&run.id, AiRunStatus::Generating, sample_count)
        .await?;
    let prompt = build_prompt(&project, &request, &samples)?;
    let generation = state.agent_runtime.generate(
        request.provider,
        &run.id,
        &prompt,
        proposal_schema(),
        &samples,
    );
    let proposal = tokio::select! {
        biased;
        () = cancellation.cancelled() => {
            return Err(crate::error::AppError::Conflict("AI run was cancelled".into()));
        }
        proposal = generation => proposal?,
    };
    validate_proposal(&project, &request.scope, &samples, &proposal)?;
    state.store.publish_ai(&run.id, proposal).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use tempfile::TempDir;
    use tokio_util::sync::CancellationToken;

    use super::execute_inner;
    use crate::{
        AppState,
        ai::{
            AccountAction, AgentRuntimeClient, AiProposal, AiProvider, AiRequest, AiRunStatus,
            AiScope, CodexStatus, EvidenceSample, ProvidersResponse, RuntimeInfo, RuntimeJob,
        },
        error::AppResult,
        model::{Command, CommandRequest, CreateProject, MediaStatus, Source, SourceRole},
    };

    struct UnexpectedRuntime;

    #[async_trait]
    impl AgentRuntimeClient for UnexpectedRuntime {
        async fn codex_status(&self) -> AppResult<CodexStatus> {
            panic!("stale runs must not contact the runtime")
        }

        async fn account(&self, _action: AccountAction) -> AppResult<CodexStatus> {
            panic!("stale runs must not contact the runtime")
        }

        async fn providers(&self) -> AppResult<ProvidersResponse> {
            panic!("stale runs must not contact the runtime")
        }

        async fn runtime_info(&self) -> AppResult<RuntimeInfo> {
            panic!("stale runs must not contact the runtime")
        }

        async fn job_runtime(&self, _id: &str) -> AppResult<RuntimeJob> {
            panic!("stale runs must not contact the runtime")
        }

        async fn generate(
            &self,
            _provider: AiProvider,
            _id: &str,
            _prompt: &str,
            _schema: serde_json::Value,
            _samples: &[EvidenceSample],
        ) -> AppResult<AiProposal> {
            panic!("stale runs must not contact the runtime")
        }

        async fn cancel(&self, _id: &str) -> AppResult<()> {
            panic!("stale runs must not contact the runtime")
        }
    }

    #[tokio::test]
    async fn stale_revision_stops_before_sampling_or_context_transmission() {
        // Given
        let data = TempDir::new().expect("tempdir");
        let state =
            AppState::initialize_with_agent_runtime(data.path(), Arc::new(UnexpectedRuntime))
                .await
                .expect("state");
        let project = state
            .store
            .create(CreateProject {
                title: "Stale worker".into(),
                frame_rate: 30,
            })
            .await
            .expect("project");
        let project = state
            .store
            .add_source(
                &project.id,
                Source {
                    id: "source".into(),
                    name: "missing.mp4".into(),
                    role: SourceRole::Program,
                    duration_frames: "120".into(),
                    width: 1280,
                    height: 720,
                    has_audio: false,
                    media_url: "/api/v1/media/missing.mp4".into(),
                    proxy_url: None,
                    thumbnail_url: None,
                    waveform: vec![],
                    status: MediaStatus::Ready,
                    error: None,
                },
            )
            .await
            .expect("source");
        let run = state
            .store
            .enqueue_ai(
                &project.id,
                AiRequest {
                    provider: AiProvider::Codex,
                    base_revision: project.revision.clone(),
                    instruction: "Inspect selected frames".into(),
                    consent: true,
                    scope: AiScope::Source {
                        source_id: "source".into(),
                        start_frame: "0".into(),
                        end_frame: "120".into(),
                    },
                },
            )
            .await
            .expect("run");
        state
            .store
            .command(
                &project.id,
                &CommandRequest {
                    command_id: uuid::Uuid::new_v4().to_string(),
                    base_revision: project.revision,
                    command: Command::SetBrief {
                        brief: "newer edit".into(),
                    },
                },
            )
            .await
            .expect("newer edit");
        // When
        execute_inner(&state, &run, &CancellationToken::new())
            .await
            .expect("stale completion");
        // Then
        let stored = state
            .store
            .ai_run(&project.id, &run.id)
            .await
            .expect("stored run");
        assert_eq!(stored.status, AiRunStatus::Stale);
        assert_eq!(stored.sample_count, 0);
    }
}
