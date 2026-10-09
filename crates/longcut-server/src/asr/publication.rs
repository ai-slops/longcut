use chrono::Utc;
use sqlx::Row;

use crate::{
    error::{AppError, AppResult},
    model::{MediaStatus, Project},
    store::Store,
    transcript::{Cue, TranscriptOrigin, TranscriptPage},
};

use super::{AsrJob, AsrJobStatus, AsrTranscriptMetadata};

impl Store {
    pub(crate) async fn publish_asr(
        &self,
        job: &AsrJob,
        cues: &[Cue],
        metadata: AsrTranscriptMetadata,
    ) -> AppResult<AsrJob> {
        let mut tx = self.pool.begin().await?;
        let persisted =
            sqlx::query("SELECT job,base_transcript_id,status FROM asr_jobs WHERE id=?")
                .bind(&job.id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppError::NotFound)?;
        if persisted.try_get::<String, _>("status")? != "running" {
            return Err(AppError::Conflict("ASR job is no longer running".into()));
        }
        let base_transcript_id: Option<String> = persisted.try_get("base_transcript_id")?;
        let state: String = sqlx::query_scalar("SELECT state FROM projects WHERE id=?")
            .bind(&job.project_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let project: Project = serde_json::from_str(&state)?;
        let source = project
            .sources
            .iter()
            .find(|source| source.id == job.source_id)
            .ok_or_else(|| AppError::Conflict("source is no longer active".into()))?;
        if source.status != MediaStatus::Ready {
            return Err(AppError::Conflict("source is no longer ready".into()));
        }
        let current: Option<String> = sqlx::query_scalar("SELECT id FROM transcripts WHERE project_id=? AND source_id=? ORDER BY rowid DESC LIMIT 1")
            .bind(&job.project_id).bind(&job.source_id).fetch_optional(&mut *tx).await?;
        if current != base_transcript_id {
            return Err(AppError::Conflict(
                "transcript changed while ASR was running; result was not published".into(),
            ));
        }
        let transcript_id = uuid::Uuid::new_v4().to_string();
        let transcript = TranscriptPage {
            id: transcript_id.clone(),
            source_id: job.source_id.clone(),
            filename: format!("local-asr-{}.json", job.id),
            created_at: Utc::now().to_rfc3339(),
            frame_rate: project.frame_rate,
            cue_count: cues.len(),
            origin: TranscriptOrigin::LocalAsr,
            asr: Some(metadata),
            cues: Vec::new(),
            next_cursor: None,
        };
        sqlx::query("INSERT INTO transcripts(id,project_id,source_id,summary) VALUES(?,?,?,?)")
            .bind(&transcript_id)
            .bind(&job.project_id)
            .bind(&job.source_id)
            .bind(serde_json::to_string(&transcript)?)
            .execute(&mut *tx)
            .await?;
        for (ordinal, cue) in cues.iter().enumerate() {
            let ordinal = i64::try_from(ordinal)
                .map_err(|_| AppError::Invalid("cue count overflow".into()))?;
            let words = cue.words.as_ref().map(serde_json::to_string).transpose()?;
            sqlx::query("INSERT INTO transcript_cues(id,transcript_id,ordinal,start_frame,end_frame,text,words) VALUES(?,?,?,?,?,?,?)")
                .bind(&cue.id).bind(&transcript_id).bind(ordinal).bind(&cue.start_frame)
                .bind(&cue.end_frame).bind(&cue.text).bind(words).execute(&mut *tx).await?;
        }
        let mut ready: AsrJob = serde_json::from_str(persisted.try_get("job")?)?;
        ready.status = AsrJobStatus::Ready;
        ready.progress.completed_chunks = ready.progress.total_chunks;
        ready.transcript_id = Some(transcript_id);
        ready.updated_at = Utc::now().to_rfc3339();
        let changed =
            sqlx::query("UPDATE asr_jobs SET job=?,status='ready',checkpoint_fingerprint='' WHERE id=? AND status='running'")
                .bind(serde_json::to_string(&ready)?)
                .bind(&job.id)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        if changed != 1 {
            return Err(AppError::Conflict("ASR job is no longer running".into()));
        }
        sqlx::query("DELETE FROM asr_checkpoints WHERE job_id=?")
            .bind(&job.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(ready)
    }
}
