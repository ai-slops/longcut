use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    store::Store,
};

use super::{
    AsrJob, ResumeSnapshot,
    checkpoint::{
        Checkpoint, CheckpointStamp, MAX_CHECKPOINT_BYTES, MAX_JOB_CHECKPOINT_BYTES, Replay,
        ReplayBuilder,
    },
    conversion::ChunkLayout,
};

#[derive(Clone, Debug)]
pub(super) struct ValidatedCheckpoints {
    stamp: CheckpointStamp,
    derived_total: u32,
}

struct Aggregate {
    count: u32,
    total_bytes: usize,
    max_bytes: usize,
}

impl ValidatedCheckpoints {
    pub(super) fn validate_progress(&self, job: &AsrJob) -> AppResult<()> {
        if job.progress.completed_chunks != self.stamp.count {
            return Err(AppError::Conflict("ASR checkpoint progress changed".into()));
        }
        match job.progress.total_chunks {
            0 if self.stamp.count == 0 => Ok(()),
            0 => Err(AppError::Conflict(
                "ASR checkpoint progress is incomplete".into(),
            )),
            total if total == self.derived_total => Ok(()),
            _ => Err(AppError::Conflict("ASR total chunk count changed".into())),
        }
    }
}

impl Store {
    pub(super) async fn validate_resume_checkpoints(
        &self,
        material: &super::resume_store::ResumeMaterial,
    ) -> AppResult<ValidatedCheckpoints> {
        let (layout, derived_total) = layout(&material.snapshot)?;
        let row = sqlx::query("SELECT job,checkpoint_fingerprint FROM asr_jobs WHERE id=?")
            .bind(&material.job.id)
            .fetch_one(&self.pool)
            .await?;
        let job: AsrJob = serde_json::from_str(row.try_get("job")?)?;
        let (replay, stamp) = self
            .read_checkpoint_replay(
                &material.job.id,
                layout,
                derived_total,
                row.try_get("checkpoint_fingerprint")?,
            )
            .await?;
        drop(replay);
        let validated = ValidatedCheckpoints {
            stamp,
            derived_total,
        };
        validated.validate_progress(&job)?;
        Ok(validated)
    }

    pub(super) async fn load_asr_replay(
        &self,
        job_id: &str,
        layout: ChunkLayout,
    ) -> AppResult<Replay> {
        let row = sqlx::query("SELECT job,checkpoint_fingerprint FROM asr_jobs WHERE id=?")
            .bind(job_id)
            .fetch_one(&self.pool)
            .await?;
        let job: AsrJob = serde_json::from_str(row.try_get("job")?)?;
        let derived_total = total_chunks(layout)?;
        let (replay, stamp) = self
            .read_checkpoint_replay(
                job_id,
                layout,
                derived_total,
                row.try_get("checkpoint_fingerprint")?,
            )
            .await?;
        ValidatedCheckpoints {
            stamp,
            derived_total,
        }
        .validate_progress(&job)?;
        Ok(replay)
    }

    async fn read_checkpoint_replay(
        &self,
        job_id: &str,
        layout: ChunkLayout,
        allowed_count: u32,
        persisted_fingerprint: &str,
    ) -> AppResult<(Replay, CheckpointStamp)> {
        let aggregate = aggregate_pool(self, job_id).await?;
        validate_aggregate(&aggregate, allowed_count)?;
        let mut builder = ReplayBuilder::new();
        for index in 0..aggregate.count {
            let row = sqlx::query("SELECT chunk_index,inference_start,inference_end,owner_start,owner_end,payload FROM asr_checkpoints WHERE job_id=? AND chunk_index=?")
                .bind(job_id).bind(i64::from(index)).fetch_optional(&self.pool).await?
                .ok_or_else(|| AppError::Conflict("ASR checkpoints are not contiguous".into()))?;
            builder.append(checkpoint(&row)?, layout)?;
        }
        let (replay, stamp) = builder.finish();
        if stamp.total_bytes != aggregate.total_bytes || stamp.fingerprint != persisted_fingerprint
        {
            return Err(AppError::Conflict(
                "ASR checkpoint fingerprint changed".into(),
            ));
        }
        Ok((replay, stamp))
    }

    pub(super) async fn recheck_checkpoint_stamp(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        job_id: &str,
        validated: &ValidatedCheckpoints,
        persisted_fingerprint: &str,
    ) -> AppResult<()> {
        let row = sqlx::query("SELECT count(*) AS count,COALESCE(SUM(length(CAST(payload AS BLOB))),0) AS total_bytes,COALESCE(MAX(length(CAST(payload AS BLOB))),0) AS max_bytes FROM asr_checkpoints WHERE job_id=?")
            .bind(job_id).fetch_one(&mut **tx).await?;
        let aggregate = aggregate_row(&row)?;
        validate_aggregate(&aggregate, validated.derived_total)?;
        if aggregate.count != validated.stamp.count
            || aggregate.total_bytes != validated.stamp.total_bytes
            || persisted_fingerprint != validated.stamp.fingerprint
        {
            return Err(AppError::Conflict(
                "ASR checkpoints changed during resume".into(),
            ));
        }
        Ok(())
    }
}

async fn aggregate_pool(store: &Store, job_id: &str) -> AppResult<Aggregate> {
    let row = sqlx::query("SELECT count(*) AS count,COALESCE(SUM(length(CAST(payload AS BLOB))),0) AS total_bytes,COALESCE(MAX(length(CAST(payload AS BLOB))),0) AS max_bytes FROM asr_checkpoints WHERE job_id=?")
        .bind(job_id).fetch_one(&store.pool).await?;
    aggregate_row(&row)
}

fn aggregate_row(row: &sqlx::sqlite::SqliteRow) -> AppResult<Aggregate> {
    Ok(Aggregate {
        count: u32::try_from(row.try_get::<i64, _>("count")?)
            .map_err(|_| AppError::Conflict("ASR checkpoint count is invalid".into()))?,
        total_bytes: usize::try_from(row.try_get::<i64, _>("total_bytes")?)
            .map_err(|_| AppError::Conflict("ASR checkpoint bytes are invalid".into()))?,
        max_bytes: usize::try_from(row.try_get::<i64, _>("max_bytes")?)
            .map_err(|_| AppError::Conflict("ASR checkpoint bytes are invalid".into()))?,
    })
}

fn validate_aggregate(aggregate: &Aggregate, allowed_count: u32) -> AppResult<()> {
    if aggregate.count > allowed_count {
        return Err(AppError::Conflict(
            "ASR checkpoint count exceeds source chunks".into(),
        ));
    }
    if aggregate.max_bytes > MAX_CHECKPOINT_BYTES {
        return Err(AppError::Conflict(
            "ASR checkpoint exceeds byte limit".into(),
        ));
    }
    if aggregate.total_bytes > MAX_JOB_CHECKPOINT_BYTES {
        return Err(AppError::Conflict(
            "ASR checkpoints exceed job byte limit".into(),
        ));
    }
    Ok(())
}

fn checkpoint(row: &sqlx::sqlite::SqliteRow) -> AppResult<Checkpoint> {
    Ok(Checkpoint {
        index: u32::try_from(row.try_get::<i64, _>("chunk_index")?)
            .map_err(|_| AppError::Conflict("ASR checkpoint index is invalid".into()))?,
        inference_start: bound(row, "inference_start")?,
        inference_end: bound(row, "inference_end")?,
        owner_start: bound(row, "owner_start")?,
        owner_end: bound(row, "owner_end")?,
        payload: row.try_get("payload")?,
    })
}

fn bound(row: &sqlx::sqlite::SqliteRow, name: &str) -> AppResult<u64> {
    row.try_get::<String, _>(name)?
        .parse()
        .map_err(|_| AppError::Conflict("ASR checkpoint bound is invalid".into()))
}

fn layout(snapshot: &ResumeSnapshot) -> AppResult<(ChunkLayout, u32)> {
    let chunk_frames = u64::from(snapshot.chunk_seconds)
        .checked_mul(u64::from(snapshot.frame_rate))
        .ok_or_else(|| AppError::Conflict("ASR checkpoint layout overflow".into()))?;
    if chunk_frames == 0 {
        return Err(AppError::Conflict("ASR checkpoint layout is empty".into()));
    }
    let layout = ChunkLayout {
        chunk_frames,
        duration_frames: snapshot.duration_frames,
        frame_rate: snapshot.frame_rate,
    };
    Ok((layout, total_chunks(layout)?))
}

fn total_chunks(layout: ChunkLayout) -> AppResult<u32> {
    u32::try_from(layout.duration_frames.div_ceil(layout.chunk_frames))
        .map_err(|_| AppError::Conflict("source has too many ASR chunks".into()))
}
