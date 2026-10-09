use chrono::Utc;
use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    model::{MediaStatus, Project},
    store::Store,
};

use super::{AsrJob, AsrJobStatus, AsrLanguage, AsrProgress, SourceSnapshot};

impl Store {
    pub(crate) async fn enqueue_asr(
        &self,
        project_id: &str,
        source_id: &str,
        language: AsrLanguage,
        model: &str,
    ) -> AppResult<(AsrJob, SourceSnapshot)> {
        let mut tx = self.pool.begin().await?;
        let state: String = sqlx::query_scalar("SELECT state FROM projects WHERE id=?")
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let project: Project = serde_json::from_str(&state)?;
        let source = project
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .ok_or(AppError::NotFound)?;
        if source.status != MediaStatus::Ready || !source.has_audio {
            return Err(AppError::Conflict(
                "source must be ready and contain audio".into(),
            ));
        }
        let active: i64 = sqlx::query_scalar("SELECT count(*) FROM asr_jobs WHERE project_id=? AND source_id=? AND status IN ('queued','running')")
            .bind(project_id).bind(source_id).fetch_one(&mut *tx).await?;
        if active > 0 {
            return Err(AppError::Conflict(
                "an ASR job is already active for this source".into(),
            ));
        }
        let base_transcript_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM transcripts WHERE project_id=? AND source_id=? ORDER BY rowid DESC LIMIT 1",
        )
        .bind(project_id)
        .bind(source_id)
        .fetch_optional(&mut *tx)
        .await?;
        let now = Utc::now().to_rfc3339();
        let job = AsrJob {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            source_id: source_id.into(),
            status: AsrJobStatus::Queued,
            progress: AsrProgress {
                completed_chunks: 0,
                total_chunks: 0,
            },
            language,
            engine: "whisper.cpp".into(),
            model: model.into(),
            created_at: now.clone(),
            updated_at: now.clone(),
            transcript_id: None,
            error: None,
            base_transcript_id: base_transcript_id.clone(),
        };
        sqlx::query("INSERT INTO asr_jobs(id,project_id,source_id,job,status,base_transcript_id,created_at) VALUES(?,?,?,?,?,?,?)")
            .bind(&job.id).bind(project_id).bind(source_id).bind(serde_json::to_string(&job)?)
            .bind(job.status.db_value()).bind(base_transcript_id).bind(&now).execute(&mut *tx).await?;
        tx.commit().await?;
        let duration_frames = source
            .duration_frames
            .parse::<u64>()
            .map_err(|_| AppError::Invalid("invalid source duration".into()))?;
        Ok((
            job,
            SourceSnapshot {
                media_url: source.media_url.clone(),
                duration_frames,
                frame_rate: project.frame_rate,
            },
        ))
    }

    pub(crate) async fn get_asr(
        &self,
        project_id: &str,
        source_id: &str,
        job_id: &str,
    ) -> AppResult<AsrJob> {
        self.get(project_id).await?;
        let row = sqlx::query("SELECT job,base_transcript_id FROM asr_jobs WHERE id=? AND project_id=? AND source_id=?")
            .bind(job_id).bind(project_id).bind(source_id).fetch_optional(&self.pool).await?
            .ok_or(AppError::NotFound)?;
        let mut job: AsrJob = serde_json::from_str(row.try_get("job")?)?;
        job.base_transcript_id = row.try_get("base_transcript_id")?;
        Ok(job)
    }

    pub(crate) async fn start_asr(&self, job_id: &str, total_chunks: u32) -> AppResult<AsrJob> {
        self.update_active_asr(job_id, |job| {
            job.status = AsrJobStatus::Running;
            job.progress.total_chunks = total_chunks;
            job.error = None;
        })
        .await
    }

    pub(crate) async fn progress_asr(&self, job_id: &str, completed: u32) -> AppResult<AsrJob> {
        self.update_active_asr(job_id, |job| job.progress.completed_chunks = completed)
            .await
    }

    pub(crate) async fn fail_asr(&self, job_id: &str, message: &str) -> AppResult<()> {
        let Some(row) = sqlx::query("SELECT job FROM asr_jobs WHERE id=?")
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(());
        };
        let mut job: AsrJob = serde_json::from_str(row.try_get("job")?)?;
        if job.status.active() {
            job.status = AsrJobStatus::Failed;
            job.error = Some(message.into());
            job.updated_at = Utc::now().to_rfc3339();
            sqlx::query("UPDATE asr_jobs SET job=?,status='failed' WHERE id=? AND status IN ('queued','running')")
                .bind(serde_json::to_string(&job)?).bind(job_id).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub(crate) async fn cancel_asr(
        &self,
        project_id: &str,
        source_id: &str,
        job_id: &str,
    ) -> AppResult<AsrJob> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT job,base_transcript_id FROM asr_jobs WHERE id=? AND project_id=? AND source_id=?")
            .bind(job_id).bind(project_id).bind(source_id).fetch_optional(&mut *tx).await?
            .ok_or(AppError::NotFound)?;
        let mut job: AsrJob = serde_json::from_str(row.try_get("job")?)?;
        job.base_transcript_id = row.try_get("base_transcript_id")?;
        if job.status.active() {
            job.status = AsrJobStatus::Cancelled;
            job.updated_at = Utc::now().to_rfc3339();
            sqlx::query("UPDATE asr_jobs SET job=?,status='cancelled' WHERE id=? AND status IN ('queued','running')")
                .bind(serde_json::to_string(&job)?).bind(job_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(job)
    }

    async fn update_active_asr(
        &self,
        job_id: &str,
        update: impl FnOnce(&mut AsrJob),
    ) -> AppResult<AsrJob> {
        let row = sqlx::query("SELECT job,base_transcript_id FROM asr_jobs WHERE id=?")
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(AppError::NotFound)?;
        let mut job: AsrJob = serde_json::from_str(row.try_get("job")?)?;
        job.base_transcript_id = row.try_get("base_transcript_id")?;
        if !job.status.active() {
            return Err(AppError::Conflict("ASR job is no longer active".into()));
        }
        update(&mut job);
        job.updated_at = Utc::now().to_rfc3339();
        let changed = sqlx::query(
            "UPDATE asr_jobs SET job=?,status=? WHERE id=? AND status IN ('queued','running')",
        )
        .bind(serde_json::to_string(&job)?)
        .bind(job.status.db_value())
        .bind(job_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(AppError::Conflict("ASR job is no longer active".into()));
        }
        Ok(job)
    }
}
