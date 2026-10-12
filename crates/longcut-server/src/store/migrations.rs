use std::path::Path;

use sqlx::Row;

use super::Store;
use crate::asr::checkpoint::{
    Checkpoint, MAX_CHECKPOINT_BYTES, MAX_JOB_CHECKPOINT_BYTES, extend_fingerprint,
};
use crate::error::{AppError, AppResult};

impl Store {
    pub(super) async fn migrate_sessions(&self, path: &Path) -> AppResult<()> {
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await?;
        match version {
            1..=9 => return Ok(()),
            0 => {}
            _ => {
                return Err(AppError::Invalid(
                    "project database schema is newer than this server".into(),
                ));
            }
        }
        let projects: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
            .fetch_one(&self.pool)
            .await?;
        if projects > 0 {
            let backup =
                path.with_extension(format!("before-sessions-{}.sqlite3", uuid::Uuid::new_v4()));
            sqlx::query("VACUUM INTO ?")
                .bind(backup.to_string_lossy().as_ref())
                .execute(&self.pool)
                .await?;
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("CREATE TABLE source_sessions(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,record TEXT NOT NULL); CREATE TABLE session_imports(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,job TEXT NOT NULL,prepared TEXT); PRAGMA user_version=1;")
            .execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Store {
    pub(super) async fn migrate_transcripts(&self, path: &Path) -> AppResult<()> {
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await?;
        if matches!(version, 2..=9) {
            return Ok(());
        }
        if version != 1 {
            return Err(AppError::Invalid(
                "unsupported transcript schema migration".into(),
            ));
        }
        let projects: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
            .fetch_one(&self.pool)
            .await?;
        if projects > 0 {
            let backup = path.with_extension(format!(
                "before-transcripts-{}.sqlite3",
                uuid::Uuid::new_v4()
            ));
            sqlx::query("VACUUM INTO ?")
                .bind(backup.to_string_lossy().as_ref())
                .execute(&self.pool)
                .await?;
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("CREATE TABLE transcripts(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,source_id TEXT NOT NULL,summary TEXT NOT NULL); CREATE TABLE transcript_cues(id TEXT PRIMARY KEY,transcript_id TEXT NOT NULL,ordinal INTEGER NOT NULL,start_frame TEXT NOT NULL,end_frame TEXT NOT NULL,text TEXT NOT NULL); CREATE INDEX transcript_source ON transcripts(project_id,source_id); CREATE INDEX transcript_cue_parent ON transcript_cues(transcript_id); CREATE VIRTUAL TABLE transcript_search USING fts5(text,content='transcript_cues',content_rowid='rowid',tokenize='trigram'); CREATE TRIGGER transcript_cues_insert AFTER INSERT ON transcript_cues BEGIN INSERT INTO transcript_search(rowid,text) VALUES(new.rowid,new.text); END; PRAGMA user_version=2;").execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Store {
    pub(super) async fn migrate_asr(&self, path: &Path) -> AppResult<()> {
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await?;
        if matches!(version, 6..=9) {
            return Ok(());
        }
        if !matches!(version, 2..=5) {
            return Err(AppError::Invalid("unsupported ASR schema migration".into()));
        }
        let projects: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
            .fetch_one(&self.pool)
            .await?;
        if projects > 0 {
            let backup =
                path.with_extension(format!("before-asr-{}.sqlite3", uuid::Uuid::new_v4()));
            sqlx::query("VACUUM INTO ?")
                .bind(backup.to_string_lossy().as_ref())
                .execute(&self.pool)
                .await?;
        }
        let mut tx = self.pool.begin().await?;
        if version == 2 {
            sqlx::query("CREATE TABLE asr_jobs(id TEXT PRIMARY KEY,project_id TEXT NOT NULL,source_id TEXT NOT NULL,job TEXT NOT NULL,status TEXT NOT NULL,base_transcript_id TEXT,created_at TEXT NOT NULL); CREATE INDEX asr_job_source ON asr_jobs(project_id,source_id,created_at); CREATE UNIQUE INDEX asr_one_active_source ON asr_jobs(project_id,source_id) WHERE status IN ('queued','running');")
                .execute(&mut *tx).await?;
        }
        if matches!(version, 2 | 3) {
            sqlx::query("ALTER TABLE transcript_cues ADD COLUMN words TEXT;")
                .execute(&mut *tx)
                .await?;
        }
        if matches!(version, 2..=4) {
            sqlx::query("ALTER TABLE asr_jobs ADD COLUMN resume_snapshot TEXT; ALTER TABLE asr_jobs ADD COLUMN checkpoint_fingerprint TEXT NOT NULL DEFAULT ''; CREATE TABLE asr_checkpoints(job_id TEXT NOT NULL,chunk_index INTEGER NOT NULL,inference_start TEXT NOT NULL,inference_end TEXT NOT NULL,owner_start TEXT NOT NULL,owner_end TEXT NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(job_id,chunk_index),FOREIGN KEY(job_id) REFERENCES asr_jobs(id) ON DELETE CASCADE);")
                .execute(&mut *tx)
                .await?;
        } else {
            let fingerprint_column: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pragma_table_info('asr_jobs') WHERE name='checkpoint_fingerprint'",
            )
            .fetch_one(&mut *tx)
            .await?;
            if fingerprint_column == 0 {
                sqlx::query("ALTER TABLE asr_jobs ADD COLUMN checkpoint_fingerprint TEXT NOT NULL DEFAULT ''")
                    .execute(&mut *tx)
                    .await?;
            }
        }
        backfill_checkpoint_fingerprints(&mut tx).await?;
        sqlx::query("PRAGMA user_version=6")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

impl Store {
    pub(super) async fn migrate_moments(&self, path: &Path) -> AppResult<()> {
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await?;
        match version {
            7..=9 => return Ok(()),
            6 => {}
            _ => {
                return Err(AppError::Invalid(
                    "unsupported moment schema migration".into(),
                ));
            }
        }
        let projects: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
            .fetch_one(&self.pool)
            .await?;
        if projects > 0 {
            let backup =
                path.with_extension(format!("before-moments-{}.sqlite3", uuid::Uuid::new_v4()));
            sqlx::query("VACUUM INTO ?")
                .bind(backup.to_string_lossy().as_ref())
                .execute(&self.pool)
                .await?;
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS moment_versions(project_id TEXT NOT NULL,moment_id TEXT NOT NULL,version TEXT NOT NULL,snapshot TEXT NOT NULL,PRIMARY KEY(project_id,moment_id,version)); CREATE INDEX IF NOT EXISTS moment_version_lookup ON moment_versions(project_id,moment_id,CAST(version AS INTEGER)); PRAGMA user_version=7;")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn backfill_checkpoint_fingerprints(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> AppResult<()> {
    let mut after = String::new();
    loop {
        let job_id: Option<String> = sqlx::query_scalar(
            "SELECT id FROM asr_jobs WHERE checkpoint_fingerprint='' AND id>? ORDER BY id LIMIT 1",
        )
        .bind(&after)
        .fetch_optional(&mut **tx)
        .await?;
        let Some(job_id) = job_id else {
            break;
        };
        let aggregate = sqlx::query("SELECT count(*) AS count,COALESCE(SUM(length(CAST(payload AS BLOB))),0) AS total_bytes,COALESCE(MAX(length(CAST(payload AS BLOB))),0) AS max_bytes FROM asr_checkpoints WHERE job_id=?")
            .bind(&job_id)
            .fetch_one(&mut **tx)
            .await?;
        let count = u32::try_from(aggregate.try_get::<i64, _>("count")?)
            .map_err(|_| AppError::Invalid("ASR checkpoint count is invalid".into()))?;
        let total_bytes = usize::try_from(aggregate.try_get::<i64, _>("total_bytes")?)
            .map_err(|_| AppError::Invalid("ASR checkpoint bytes are invalid".into()))?;
        let max_bytes = usize::try_from(aggregate.try_get::<i64, _>("max_bytes")?)
            .map_err(|_| AppError::Invalid("ASR checkpoint bytes are invalid".into()))?;
        if max_bytes > MAX_CHECKPOINT_BYTES || total_bytes > MAX_JOB_CHECKPOINT_BYTES {
            return Err(AppError::Invalid(
                "ASR checkpoints exceed migration byte limits".into(),
            ));
        }
        let mut fingerprint = String::new();
        for index in 0..count {
            let row = sqlx::query("SELECT chunk_index,inference_start,inference_end,owner_start,owner_end,payload FROM asr_checkpoints WHERE job_id=? AND chunk_index=?")
                .bind(&job_id)
                .bind(i64::from(index))
                .fetch_optional(&mut **tx)
                .await?
                .ok_or_else(|| AppError::Invalid("ASR checkpoints are not contiguous".into()))?;
            fingerprint = extend_fingerprint(
                &fingerprint,
                &Checkpoint {
                    index: u32::try_from(row.try_get::<i64, _>("chunk_index")?)
                        .map_err(|_| AppError::Invalid("ASR checkpoint index is invalid".into()))?,
                    inference_start: migration_bound(&row, "inference_start")?,
                    inference_end: migration_bound(&row, "inference_end")?,
                    owner_start: migration_bound(&row, "owner_start")?,
                    owner_end: migration_bound(&row, "owner_end")?,
                    payload: row.try_get("payload")?,
                },
            );
        }
        if count > 0 {
            sqlx::query("UPDATE asr_jobs SET checkpoint_fingerprint=? WHERE id=? AND checkpoint_fingerprint=''")
                .bind(fingerprint)
                .bind(&job_id)
                .execute(&mut **tx)
                .await?;
        }
        after = job_id;
    }
    Ok(())
}

fn migration_bound(row: &sqlx::sqlite::SqliteRow, name: &str) -> AppResult<u64> {
    row.try_get::<String, _>(name)?
        .parse()
        .map_err(|_| AppError::Invalid("ASR checkpoint bound is invalid".into()))
}
