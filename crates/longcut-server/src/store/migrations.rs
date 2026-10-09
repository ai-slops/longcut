use std::path::Path;

use super::Store;
use crate::error::{AppError, AppResult};

impl Store {
    pub(super) async fn migrate_sessions(&self, path: &Path) -> AppResult<()> {
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await?;
        match version {
            1..=4 => return Ok(()),
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
        if matches!(version, 2..=4) {
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
        if version == 4 {
            return Ok(());
        }
        if !matches!(version, 2 | 3) {
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
        sqlx::query("ALTER TABLE transcript_cues ADD COLUMN words TEXT; PRAGMA user_version=4;")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
