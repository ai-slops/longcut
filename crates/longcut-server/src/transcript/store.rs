use super::{Cue, ImportTranscript, Search, TranscriptOrigin, TranscriptPage, parse};
use crate::{
    error::{AppError, AppResult},
    model::Project,
    store::Store,
};
use chrono::Utc;
use sqlx::Row;

impl Store {
    pub async fn import_transcript(
        &self,
        project_id: &str,
        source_id: &str,
        input: ImportTranscript,
    ) -> AppResult<TranscriptPage> {
        if input.filename.trim().is_empty() || input.filename.len() > 512 {
            return Err(AppError::Invalid(
                "a bounded transcript filename is required".into(),
            ));
        }
        let mut tx = self.pool.begin().await?;
        let text: String = sqlx::query_scalar("SELECT state FROM projects WHERE id=?")
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let project: Project = serde_json::from_str(&text)?;
        let source = project
            .sources
            .iter()
            .find(|source| source.id == source_id)
            .ok_or(AppError::NotFound)?;
        let duration = source
            .duration_frames
            .parse()
            .map_err(|_| AppError::Invalid("invalid source duration".into()))?;
        let cues = parse::cues(&input.text, project.frame_rate, duration)?;
        let transcript = TranscriptPage {
            id: uuid::Uuid::new_v4().to_string(),
            source_id: source_id.into(),
            filename: input.filename,
            created_at: Utc::now().to_rfc3339(),
            frame_rate: project.frame_rate,
            cue_count: cues.len(),
            origin: TranscriptOrigin::SubtitleImport,
            asr: None,
            cues: vec![],
            next_cursor: None,
        };
        sqlx::query("INSERT INTO transcripts(id,project_id,source_id,summary) VALUES(?,?,?,?)")
            .bind(&transcript.id)
            .bind(project_id)
            .bind(source_id)
            .bind(serde_json::to_string(&transcript)?)
            .execute(&mut *tx)
            .await?;
        for (ordinal, cue) in cues.iter().enumerate() {
            sqlx::query("INSERT INTO transcript_cues(id,transcript_id,ordinal,start_frame,end_frame,text) VALUES(?,?,?,?,?,?)").bind(&cue.id).bind(&transcript.id).bind(i64::try_from(ordinal).map_err(|_| AppError::Invalid("cue count overflow".into()))?).bind(&cue.start_frame).bind(&cue.end_frame).bind(&cue.text).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        self.transcript(
            project_id,
            source_id,
            Search {
                query: None,
                cursor: Some(format!("{}:0", transcript.id)),
            },
        )
        .await?
        .ok_or(AppError::NotFound)
    }

    pub async fn transcript(
        &self,
        project_id: &str,
        source_id: &str,
        search: Search,
    ) -> AppResult<Option<TranscriptPage>> {
        if !self
            .get(project_id)
            .await?
            .sources
            .iter()
            .any(|source| source.id == source_id)
        {
            return Err(AppError::NotFound);
        }
        let (requested, offset) = match search.cursor.as_deref() {
            None => (None, 0_u32),
            Some(cursor) => {
                let (id, offset) = cursor
                    .split_once(':')
                    .ok_or_else(|| AppError::Invalid("invalid transcript cursor".into()))?;
                uuid::Uuid::parse_str(id)
                    .map_err(|_| AppError::Invalid("invalid transcript cursor".into()))?;
                (
                    Some(id),
                    offset
                        .parse::<u32>()
                        .map_err(|_| AppError::Invalid("invalid transcript cursor".into()))?,
                )
            }
        };
        let row = match requested {
            Some(id) => sqlx::query("SELECT id,summary FROM transcripts WHERE project_id=? AND source_id=? AND id=?").bind(project_id).bind(source_id).bind(id).fetch_optional(&self.pool).await?,
            None => sqlx::query("SELECT id,summary FROM transcripts WHERE project_id=? AND source_id=? ORDER BY rowid DESC LIMIT 1").bind(project_id).bind(source_id).fetch_optional(&self.pool).await?,
        };
        let Some(row) = row else {
            return Ok(None);
        };
        let id: String = row.try_get("id")?;
        let mut transcript: TranscriptPage = serde_json::from_str(row.try_get("summary")?)?;
        if offset > 20_000 {
            return Err(AppError::Invalid("transcript cursor exceeds limit".into()));
        }
        let query = search.query.unwrap_or_default().trim().to_string();
        if query.len() > 512 {
            return Err(AppError::Invalid("transcript query is too long".into()));
        }
        let rows = if query.chars().count() >= 3 {
            let query = format!("\"{}\"", query.replace('"', "\"\""));
            sqlx::query("SELECT c.* FROM transcript_cues c JOIN transcript_search f ON f.rowid=c.rowid WHERE c.transcript_id=? AND transcript_search MATCH ? ORDER BY CAST(c.start_frame AS INTEGER),c.ordinal LIMIT 101 OFFSET ?")
                .bind(&id).bind(query).bind(offset).fetch_all(&self.pool).await?
        } else {
            sqlx::query("SELECT * FROM transcript_cues WHERE transcript_id=? AND instr(lower(text),lower(?))>0 ORDER BY CAST(start_frame AS INTEGER),ordinal LIMIT 101 OFFSET ?")
                .bind(&id).bind(query).bind(offset).fetch_all(&self.pool).await?
        };
        transcript.next_cursor =
            (rows.len() > 100).then(|| format!("{}:{}", transcript.id, offset + 100));
        transcript.cues = rows
            .into_iter()
            .take(100)
            .map(|row| {
                Ok(Cue {
                    id: row.try_get("id")?,
                    start_frame: row.try_get("start_frame")?,
                    end_frame: row.try_get("end_frame")?,
                    text: row.try_get("text")?,
                })
            })
            .collect::<AppResult<_>>()?;
        Ok(Some(transcript))
    }
}
