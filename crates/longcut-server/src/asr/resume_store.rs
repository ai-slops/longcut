use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    model::{MediaStatus, Project},
    store::Store,
};

use super::{
    AsrJob, AsrJobStatus, ResumeSnapshot, SourceSnapshot,
    resume_checkpoint_store::ValidatedCheckpoints,
};

pub(crate) struct ResumeMaterial {
    pub job: AsrJob,
    pub snapshot: ResumeSnapshot,
    pub source: SourceSnapshot,
}

impl Store {
    pub(crate) async fn asr_source_snapshot(
        &self,
        project_id: &str,
        source_id: &str,
    ) -> AppResult<SourceSnapshot> {
        source_snapshot(&self.get(project_id).await?, source_id)
    }

    pub(crate) async fn resume_material(
        &self,
        project_id: &str,
        source_id: &str,
        job_id: &str,
    ) -> AppResult<ResumeMaterial> {
        let row = sqlx::query("SELECT job,base_transcript_id,resume_snapshot FROM asr_jobs WHERE id=? AND project_id=? AND source_id=?")
            .bind(job_id).bind(project_id).bind(source_id).fetch_optional(&self.pool).await?
            .ok_or(AppError::NotFound)?;
        let mut job: AsrJob = serde_json::from_str(row.try_get("job")?)?;
        if !matches!(job.status, AsrJobStatus::Failed | AsrJobStatus::Cancelled) {
            return Err(AppError::Conflict(
                "ASR job cannot be resumed from its current state".into(),
            ));
        }
        let snapshot: ResumeSnapshot = row
            .try_get::<Option<&str>, _>("resume_snapshot")?
            .ok_or_else(|| AppError::Conflict("legacy ASR jobs cannot be resumed".into()))
            .and_then(|value| serde_json::from_str(value).map_err(AppError::from))?;
        job.base_transcript_id = row.try_get("base_transcript_id")?;
        job.resume_eligible = true;
        let source = SourceSnapshot {
            media_url: snapshot.source_media_url.clone(),
            duration_frames: snapshot.duration_frames,
            frame_rate: snapshot.frame_rate,
        };
        Ok(ResumeMaterial {
            job,
            snapshot,
            source,
        })
    }

    pub(super) async fn resume_asr(
        &self,
        material: &ResumeMaterial,
        current: &ResumeSnapshot,
        validated: &ValidatedCheckpoints,
    ) -> AppResult<AsrJob> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT job,status,base_transcript_id,resume_snapshot,checkpoint_fingerprint FROM asr_jobs WHERE id=? AND project_id=? AND source_id=?")
            .bind(&material.job.id).bind(&material.job.project_id).bind(&material.job.source_id)
            .fetch_optional(&mut *tx).await?.ok_or(AppError::NotFound)?;
        let status: String = row.try_get("status")?;
        if !matches!(status.as_str(), "failed" | "cancelled") {
            return Err(AppError::Conflict(
                "ASR job cannot be resumed from its current state".into(),
            ));
        }
        let persisted: ResumeSnapshot = serde_json::from_str(
            row.try_get::<Option<&str>, _>("resume_snapshot")?
                .ok_or_else(|| AppError::Conflict("legacy ASR jobs cannot be resumed".into()))?,
        )?;
        if &persisted != current || persisted != material.snapshot {
            return Err(AppError::Conflict(
                "ASR resume inputs or runtime changed".into(),
            ));
        }
        let state: String = sqlx::query_scalar("SELECT state FROM projects WHERE id=?")
            .bind(&material.job.project_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let project: Project = serde_json::from_str(&state)?;
        if source_snapshot(&project, &material.job.source_id)? != material.source {
            return Err(AppError::Conflict("ASR source metadata changed".into()));
        }
        let current_transcript: Option<String> = sqlx::query_scalar("SELECT id FROM transcripts WHERE project_id=? AND source_id=? ORDER BY rowid DESC LIMIT 1")
            .bind(&material.job.project_id).bind(&material.job.source_id)
            .fetch_optional(&mut *tx).await?;
        if current_transcript != row.try_get::<Option<String>, _>("base_transcript_id")? {
            return Err(AppError::Conflict(
                "transcript changed after the ASR job started".into(),
            ));
        }
        let active: i64 = sqlx::query_scalar("SELECT count(*) FROM asr_jobs WHERE project_id=? AND source_id=? AND status IN ('queued','running')")
            .bind(&material.job.project_id).bind(&material.job.source_id).fetch_one(&mut *tx).await?;
        if active != 0 {
            return Err(AppError::Conflict(
                "an ASR job is already active for this source".into(),
            ));
        }
        self.recheck_checkpoint_stamp(
            &mut tx,
            &material.job.id,
            validated,
            row.try_get("checkpoint_fingerprint")?,
        )
        .await?;
        let mut job: AsrJob = serde_json::from_str(row.try_get("job")?)?;
        validated.validate_progress(&job)?;
        job.status = AsrJobStatus::Queued;
        job.error = None;
        job.resume_eligible = false;
        job.updated_at = chrono::Utc::now().to_rfc3339();
        let changed = sqlx::query("UPDATE asr_jobs SET job=?,status='queued' WHERE id=? AND status IN ('failed','cancelled')")
            .bind(serde_json::to_string(&job)?).bind(&job.id).execute(&mut *tx).await?.rows_affected();
        if changed != 1 {
            return Err(AppError::Conflict(
                "ASR job was resumed concurrently".into(),
            ));
        }
        tx.commit().await?;
        Ok(job)
    }
}

fn source_snapshot(project: &Project, source_id: &str) -> AppResult<SourceSnapshot> {
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
    Ok(SourceSnapshot {
        media_url: source.media_url.clone(),
        duration_frames: source
            .duration_frames
            .parse()
            .map_err(|_| AppError::Invalid("invalid source duration".into()))?,
        frame_rate: project.frame_rate,
    })
}
