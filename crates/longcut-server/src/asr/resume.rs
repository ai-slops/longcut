use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use crate::{
    AppState,
    error::{AppError, AppResult},
};

use super::{AsrJob, Service, fingerprint, resume_store::ResumeMaterial};

impl Service {
    pub(crate) async fn resume(
        self: &Arc<Self>,
        state: &AppState,
        project_id: &str,
        source_id: &str,
        job_id: &str,
    ) -> AppResult<AsrJob> {
        let material = state
            .store
            .resume_material(project_id, source_id, job_id)
            .await?;
        let cancellation = CancellationToken::new();
        self.reserve(job_id, cancellation.clone()).await?;
        let service = Arc::clone(self);
        let worker_state = state.clone();
        tokio::spawn(async move {
            service
                .activate_resume(worker_state, material, cancellation)
                .await
        })
        .await
        .map_err(|error| AppError::Internal(anyhow::anyhow!("ASR resume task failed: {error}")))?
    }

    async fn activate_resume(
        self: Arc<Self>,
        state: AppState,
        material: ResumeMaterial,
        cancellation: CancellationToken,
    ) -> AppResult<AsrJob> {
        let activation = self.prepare_resume(&state, &material, &cancellation).await;
        let job = match activation {
            Ok(job) => job,
            Err(error) => {
                self.finish(&material.job.id).await;
                return Err(error);
            }
        };
        if cancellation.is_cancelled() {
            let cancelled = state
                .store
                .cancel_asr(&job.project_id, &job.source_id, &job.id)
                .await;
            self.finish(&job.id).await;
            return cancelled;
        }
        self.spawn(&state, job.clone(), material.source, cancellation);
        Ok(job)
    }

    async fn prepare_resume(
        &self,
        state: &AppState,
        material: &ResumeMaterial,
        cancellation: &CancellationToken,
    ) -> AppResult<AsrJob> {
        #[cfg(test)]
        self.wait_resume_gate(true).await;
        let source_path = state
            .artifacts
            .join(crate::media::artifact_name(&material.source.media_url)?);
        let current = fingerprint::capture(
            &self.config,
            &source_path,
            &material.source,
            material.job.language,
            cancellation,
        )
        .await?;
        let validated = state.store.validate_resume_checkpoints(material).await?;
        #[cfg(test)]
        self.wait_resume_gate(false).await;
        state.store.resume_asr(material, &current, &validated).await
    }
}
