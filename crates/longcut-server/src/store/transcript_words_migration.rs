use std::path::Path;

use futures_util::TryStreamExt;
use sqlx::Row;

use super::Store;
use crate::error::{AppError, AppResult};

const BACKFILL_BATCH: usize = 256;

struct IndexedWord {
    cue_rowid: i64,
    transcript_id: String,
    cue_id: String,
    word_id: String,
    ordinal: i64,
    start_frame: String,
    end_frame: String,
    text: String,
}

impl Store {
    pub(super) async fn migrate_transcript_words(&self, path: &Path) -> AppResult<()> {
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await?;
        match version {
            8 => return Ok(()),
            7 => {}
            _ => {
                return Err(AppError::Invalid(
                    "unsupported transcript word schema migration".into(),
                ));
            }
        }
        let projects: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
            .fetch_one(&self.pool)
            .await?;
        if projects > 0 {
            let backup = path.with_extension(format!(
                "before-transcript-words-{}.sqlite3",
                uuid::Uuid::new_v4()
            ));
            sqlx::query("VACUUM INTO ?")
                .bind(backup.to_string_lossy().as_ref())
                .execute(&self.pool)
                .await?;
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("CREATE TABLE transcript_words(transcript_id TEXT NOT NULL,cue_id TEXT NOT NULL,word_id TEXT NOT NULL,ordinal INTEGER NOT NULL,start_frame TEXT NOT NULL,end_frame TEXT NOT NULL,text TEXT NOT NULL,PRIMARY KEY(transcript_id,word_id),UNIQUE(cue_id,ordinal),FOREIGN KEY(cue_id) REFERENCES transcript_cues(id) ON DELETE CASCADE); CREATE INDEX transcript_word_cue ON transcript_words(cue_id,ordinal);")
            .execute(&mut *tx)
            .await?;
        reject_invalid_word_json(&mut tx).await?;
        backfill_words(&mut tx).await?;
        sqlx::query("PRAGMA user_version=8")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

async fn reject_invalid_word_json(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> AppResult<()> {
    let invalid: Option<String> = sqlx::query_scalar(
        "SELECT id FROM transcript_cues WHERE words IS NOT NULL AND CASE WHEN json_valid(words) THEN json_type(words)<>'array' ELSE 1 END LIMIT 1",
    )
    .fetch_optional(&mut **tx)
    .await?;
    if invalid.is_some() {
        return Err(AppError::Invalid(
            "stored transcript words are malformed".into(),
        ));
    }
    Ok(())
}

async fn backfill_words(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>) -> AppResult<()> {
    let mut cue_rowid = 0_i64;
    let mut ordinal = -1_i64;
    loop {
        let mut stream = sqlx::query("SELECT cue.rowid AS cue_rowid,cue.transcript_id,json_extract(word.value,'$.id') AS word_id,cue.id AS cue_id,CAST(word.key AS INTEGER) AS ordinal,json_extract(word.value,'$.start_frame') AS start_frame,json_extract(word.value,'$.end_frame') AS end_frame,json_extract(word.value,'$.text') AS text FROM transcript_cues cue JOIN json_each(cue.words) word WHERE cue.words IS NOT NULL AND (cue.rowid>? OR (cue.rowid=? AND CAST(word.key AS INTEGER)>?)) ORDER BY cue.rowid,CAST(word.key AS INTEGER) LIMIT ?")
            .bind(cue_rowid)
            .bind(cue_rowid)
            .bind(ordinal)
            .bind(i64::try_from(BACKFILL_BATCH).map_err(|_| AppError::Internal(anyhow::anyhow!("invalid transcript word batch size")))?)
            .fetch(&mut **tx);
        let mut batch = Vec::with_capacity(BACKFILL_BATCH);
        while let Some(row) = stream.try_next().await? {
            batch.push(parse_word(&row)?);
        }
        drop(stream);
        if batch.is_empty() {
            break;
        }
        for word in &batch {
            sqlx::query("INSERT INTO transcript_words(transcript_id,cue_id,word_id,ordinal,start_frame,end_frame,text) VALUES(?,?,?,?,?,?,?)")
                .bind(&word.transcript_id)
                .bind(&word.cue_id)
                .bind(&word.word_id)
                .bind(word.ordinal)
                .bind(&word.start_frame)
                .bind(&word.end_frame)
                .bind(&word.text)
                .execute(&mut **tx)
                .await?;
        }
        let last = batch
            .last()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("empty transcript word batch")))?;
        cue_rowid = last.cue_rowid;
        ordinal = last.ordinal;
    }
    Ok(())
}

fn parse_word(row: &sqlx::sqlite::SqliteRow) -> AppResult<IndexedWord> {
    let word = IndexedWord {
        cue_rowid: row.try_get("cue_rowid")?,
        transcript_id: row.try_get("transcript_id")?,
        cue_id: row.try_get("cue_id")?,
        word_id: row.try_get("word_id")?,
        ordinal: row.try_get("ordinal")?,
        start_frame: row.try_get("start_frame")?,
        end_frame: row.try_get("end_frame")?,
        text: row.try_get("text")?,
    };
    let start = word
        .start_frame
        .parse::<u64>()
        .map_err(|_| AppError::Invalid("stored transcript word start is invalid".into()))?;
    let end = word
        .end_frame
        .parse::<u64>()
        .map_err(|_| AppError::Invalid("stored transcript word end is invalid".into()))?;
    if word.word_id.is_empty() || word.ordinal < 0 || end < start {
        return Err(AppError::Invalid(
            "stored transcript word is malformed".into(),
        ));
    }
    Ok(word)
}
