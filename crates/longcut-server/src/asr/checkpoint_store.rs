use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    store::Store,
};

use super::{
    AsrJob,
    checkpoint::{Checkpoint, MAX_CHECKPOINT_BYTES, MAX_JOB_CHECKPOINT_BYTES, extend_fingerprint},
};

impl Store {
    pub(super) async fn checkpoint_asr(
        &self,
        job_id: &str,
        checkpoint: &Checkpoint,
    ) -> AppResult<AsrJob> {
        if checkpoint.payload.len() > MAX_CHECKPOINT_BYTES {
            return Err(AppError::Invalid(
                "ASR checkpoint exceeds byte limit".into(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT job,status,checkpoint_fingerprint FROM asr_jobs WHERE id=?")
            .bind(job_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        if row.try_get::<String, _>("status")? != "running" {
            return Err(AppError::Conflict("ASR job is no longer running".into()));
        }
        let mut job: AsrJob = serde_json::from_str(row.try_get("job")?)?;
        if job.progress.completed_chunks != checkpoint.index {
            return Err(AppError::Conflict(
                "ASR checkpoint index is not next".into(),
            ));
        }
        if checkpoint.index >= job.progress.total_chunks {
            return Err(AppError::Conflict(
                "ASR checkpoint index exceeds total chunks".into(),
            ));
        }
        let persisted_bytes: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(length(CAST(payload AS BLOB))),0) FROM asr_checkpoints WHERE job_id=?",
        )
        .bind(job_id)
        .fetch_one(&mut *tx)
        .await?;
        let persisted_bytes = usize::try_from(persisted_bytes)
            .map_err(|_| AppError::Conflict("ASR checkpoint byte count is invalid".into()))?;
        let total_bytes = persisted_bytes
            .checked_add(checkpoint.payload.len())
            .ok_or_else(|| AppError::Invalid("ASR checkpoint byte count overflow".into()))?;
        if total_bytes > MAX_JOB_CHECKPOINT_BYTES {
            return Err(AppError::Invalid(
                "ASR checkpoints exceed 134217728 bytes".into(),
            ));
        }
        sqlx::query("INSERT INTO asr_checkpoints(job_id,chunk_index,inference_start,inference_end,owner_start,owner_end,payload) VALUES(?,?,?,?,?,?,?)")
            .bind(job_id)
            .bind(i64::from(checkpoint.index))
            .bind(checkpoint.inference_start.to_string())
            .bind(checkpoint.inference_end.to_string())
            .bind(checkpoint.owner_start.to_string())
            .bind(checkpoint.owner_end.to_string())
            .bind(&checkpoint.payload)
            .execute(&mut *tx)
            .await?;
        job.progress.completed_chunks = checkpoint
            .index
            .checked_add(1)
            .ok_or_else(|| AppError::Invalid("ASR checkpoint progress overflow".into()))?;
        job.updated_at = chrono::Utc::now().to_rfc3339();
        let fingerprint = extend_fingerprint(row.try_get("checkpoint_fingerprint")?, checkpoint);
        let changed = sqlx::query(
            "UPDATE asr_jobs SET job=?,checkpoint_fingerprint=? WHERE id=? AND status='running'",
        )
        .bind(serde_json::to_string(&job)?)
        .bind(fingerprint)
        .bind(job_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if changed != 1 {
            return Err(AppError::Conflict("ASR job is no longer running".into()));
        }
        tx.commit().await?;
        Ok(job)
    }
}
