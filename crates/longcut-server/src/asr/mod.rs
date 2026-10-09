mod audio;
pub(crate) mod checkpoint;
mod checkpoint_store;
mod config;
mod conversion;
mod fingerprint;
pub mod http;
mod process;
mod publication;
mod resume;
#[cfg(test)]
mod resume_checkpoint_integrity_tests;
#[cfg(test)]
mod resume_checkpoint_lifecycle_tests;
#[cfg(test)]
mod resume_checkpoint_limits_tests;
#[cfg(test)]
mod resume_checkpoint_migration_tests;
mod resume_checkpoint_store;
#[cfg(test)]
mod resume_service_tests;
mod resume_store;
#[cfg(test)]
mod resume_test_support;
#[cfg(test)]
mod resume_tests;
mod store;
#[cfg(test)]
mod tests;
mod types;
mod whisper;
mod worker;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

pub use config::AsrConfig;
pub use types::{
    AsrJob, AsrJobStatus, AsrLanguage, AsrOwnershipPolicy, AsrProgress, AsrRuntimeStatus,
    AsrTimingQuality, AsrTranscriptMetadata, StartAsr,
};
pub(crate) use types::{ResumeSnapshot, SourceSnapshot};

use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::{AppState, error::AppResult};

pub struct Service {
    config: AsrConfig,
    work_root: PathBuf,
    slots: Arc<Semaphore>,
    cancellations: Mutex<HashMap<String, CancellationToken>>,
    #[cfg(test)]
    resume_test_gates: Mutex<ResumeTestGates>,
}

#[cfg(test)]
#[derive(Default)]
struct ResumeTestGates {
    before_hash: Option<TestGate>,
    before_activation: Option<TestGate>,
}

#[cfg(test)]
#[derive(Clone)]
struct TestGate {
    entered: Arc<tokio::sync::Barrier>,
    release: Arc<tokio::sync::Barrier>,
}

impl Service {
    pub(crate) async fn initialize(
        data: &std::path::Path,
        config: AsrConfig,
    ) -> AppResult<Arc<Self>> {
        let work_root = data.join("asr-work");
        match tokio::fs::remove_dir_all(&work_root).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        tokio::fs::create_dir_all(&work_root).await?;
        Ok(Arc::new(Self {
            slots: Arc::new(Semaphore::new(config.max_concurrency)),
            config,
            work_root,
            cancellations: Mutex::new(HashMap::new()),
            #[cfg(test)]
            resume_test_gates: Mutex::new(ResumeTestGates::default()),
        }))
    }

    pub async fn status(&self) -> AsrRuntimeStatus {
        self.config.status().await
    }

    pub(crate) async fn enqueue(
        self: &Arc<Self>,
        state: &AppState,
        project_id: &str,
        source_id: &str,
        request: StartAsr,
    ) -> AppResult<AsrJob> {
        let runtime = self.status().await;
        if !runtime.available {
            return Err(crate::error::AppError::Conflict(
                "local ASR runtime is not available; inspect /api/v1/asr/status".into(),
            ));
        }
        let source = state
            .store
            .asr_source_snapshot(project_id, source_id)
            .await?;
        let source_path = state
            .artifacts
            .join(crate::media::artifact_name(&source.media_url)?);
        let cancellation = CancellationToken::new();
        let snapshot = fingerprint::capture(
            &self.config,
            &source_path,
            &source,
            request.language,
            &cancellation,
        )
        .await?;
        let (job, source) = state
            .store
            .enqueue_asr(
                project_id,
                source_id,
                request.language,
                &snapshot.model_path,
                &snapshot,
            )
            .await?;
        self.reserve(&job.id, cancellation.clone()).await?;
        self.spawn(state, job.clone(), source, cancellation);
        Ok(job)
    }

    async fn reserve(&self, job_id: &str, cancellation: CancellationToken) -> AppResult<()> {
        let mut cancellations = self.cancellations.lock().await;
        if cancellations.contains_key(job_id) {
            return Err(crate::error::AppError::Conflict(
                "previous ASR worker is still stopping".into(),
            ));
        }
        cancellations.insert(job_id.into(), cancellation);
        Ok(())
    }

    fn spawn(
        self: &Arc<Self>,
        state: &AppState,
        job: AsrJob,
        source: SourceSnapshot,
        cancellation: CancellationToken,
    ) {
        let worker_state = state.clone();
        let service = Arc::clone(self);
        tokio::spawn(async move {
            worker::execute(worker::WorkerInput {
                state: worker_state,
                service,
                job,
                source,
                cancellation,
            })
            .await;
        });
    }

    pub(crate) async fn cancel(&self, job_id: &str) {
        if let Some(cancellation) = self.cancellations.lock().await.get(job_id) {
            cancellation.cancel();
        }
    }

    async fn finish(&self, job_id: &str) {
        self.cancellations.lock().await.remove(job_id);
    }

    #[cfg(test)]
    async fn install_resume_gate(&self, before_hash: bool) -> TestGate {
        let gate = TestGate {
            entered: Arc::new(tokio::sync::Barrier::new(2)),
            release: Arc::new(tokio::sync::Barrier::new(2)),
        };
        let mut gates = self.resume_test_gates.lock().await;
        if before_hash {
            gates.before_hash = Some(gate.clone());
        } else {
            gates.before_activation = Some(gate.clone());
        }
        gate
    }

    #[cfg(test)]
    async fn wait_resume_gate(&self, before_hash: bool) {
        let gate = {
            let mut gates = self.resume_test_gates.lock().await;
            if before_hash {
                gates.before_hash.take()
            } else {
                gates.before_activation.take()
            }
        };
        if let Some(gate) = gate {
            gate.entered.wait().await;
            gate.release.wait().await;
        }
    }
}
