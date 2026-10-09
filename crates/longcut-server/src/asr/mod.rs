mod audio;
mod config;
mod conversion;
pub mod http;
mod process;
mod publication;
mod store;
#[cfg(test)]
mod tests;
mod types;
mod whisper;
mod worker;

use std::{collections::HashMap, path::PathBuf, sync::Arc};

pub use config::AsrConfig;
pub(crate) use types::SourceSnapshot;
pub use types::{
    AsrJob, AsrJobStatus, AsrLanguage, AsrOwnershipPolicy, AsrProgress, AsrRuntimeStatus,
    AsrTimingQuality, AsrTranscriptMetadata, StartAsr,
};

use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

use crate::{AppState, error::AppResult};

pub struct Service {
    config: AsrConfig,
    work_root: PathBuf,
    slots: Arc<Semaphore>,
    cancellations: Mutex<HashMap<String, CancellationToken>>,
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
        let model = runtime
            .model
            .ok_or_else(|| crate::error::AppError::Conflict("ASR model is unavailable".into()))?;
        let (job, source) = state
            .store
            .enqueue_asr(project_id, source_id, request.language, &model)
            .await?;
        let cancellation = CancellationToken::new();
        self.cancellations
            .lock()
            .await
            .insert(job.id.clone(), cancellation.clone());
        let worker_state = state.clone();
        let service = Arc::clone(self);
        let worker_job = job.clone();
        tokio::spawn(async move {
            worker::execute(worker::WorkerInput {
                state: worker_state,
                service,
                job: worker_job,
                source,
                cancellation,
            })
            .await;
        });
        Ok(job)
    }

    pub(crate) async fn cancel(&self, job_id: &str) {
        if let Some(cancellation) = self.cancellations.lock().await.get(job_id) {
            cancellation.cancel();
        }
    }

    async fn finish(&self, job_id: &str) {
        self.cancellations.lock().await.remove(job_id);
    }
}
