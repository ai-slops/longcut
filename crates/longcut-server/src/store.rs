use std::{path::Path, str::FromStr, time::Duration};

use chrono::Utc;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

use crate::{
    domain,
    error::{AppError, AppResult},
    model::*,
};

mod ai;
mod ai_publication;
mod jobs;
mod migrations;
mod session_records;
mod sessions;

#[derive(Clone)]
pub struct Store {
    pub(crate) pool: SqlitePool,
}

impl Store {
    pub async fn open(path: &Path) -> AppResult<Self> {
        let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
            .map_err(|error| AppError::Internal(error.into()))?
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY, state TEXT NOT NULL, history TEXT NOT NULL, updated_at TEXT NOT NULL); CREATE TABLE IF NOT EXISTS commands(project_id TEXT NOT NULL, command_id TEXT NOT NULL, request TEXT NOT NULL, result TEXT NOT NULL, PRIMARY KEY(project_id, command_id)); CREATE TABLE IF NOT EXISTS render_jobs(id TEXT PRIMARY KEY, project_id TEXT NOT NULL, snapshot TEXT NOT NULL, status TEXT NOT NULL); CREATE TABLE IF NOT EXISTS agent_runs(id TEXT PRIMARY KEY, project_id TEXT NOT NULL, run TEXT NOT NULL, request TEXT NOT NULL, status TEXT NOT NULL, created_at TEXT NOT NULL);")
            .execute(&pool).await?;
        let store = Self { pool };
        store.migrate_sessions(path).await?;
        store.migrate_transcripts(path).await?;
        store.recover_interrupted().await?;
        Ok(store)
    }

    pub async fn list(&self) -> AppResult<Vec<ProjectSummary>> {
        let rows = sqlx::query("SELECT state FROM projects ORDER BY updated_at DESC")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                let project: Project = serde_json::from_str(row.try_get("state")?)?;
                Ok(ProjectSummary {
                    id: project.id,
                    title: project.title,
                    revision: project.revision,
                    updated_at: project.updated_at,
                })
            })
            .collect()
    }

    pub async fn healthy(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }

    async fn recover_interrupted(&self) -> AppResult<()> {
        for row in sqlx::query("SELECT id,state FROM projects")
            .fetch_all(&self.pool)
            .await?
        {
            let mut project: Project = serde_json::from_str(row.try_get("state")?)?;
            let mut changed = false;
            for source in &mut project.sources {
                if source.status == MediaStatus::Processing {
                    source.status = MediaStatus::Error;
                    source.error =
                        Some("processing was interrupted; import the source again".into());
                    changed = true;
                }
            }
            for render in &mut project.renders {
                if matches!(render.status, RenderStatus::Queued | RenderStatus::Running) {
                    render.status = RenderStatus::Error;
                    render.error = Some("render was interrupted; request a new render".into());
                    changed = true;
                }
            }
            if changed {
                sqlx::query("UPDATE projects SET state=? WHERE id=?")
                    .bind(serde_json::to_string(&project)?)
                    .bind(row.try_get::<String, _>("id")?)
                    .execute(&self.pool)
                    .await?;
            }
        }
        sqlx::query("UPDATE render_jobs SET status='error' WHERE status IN ('queued','running')")
            .execute(&self.pool)
            .await?;
        self.recover_ai_runs().await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> AppResult<Project> {
        let row = sqlx::query("SELECT state FROM projects WHERE id=?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(serde_json::from_str(row.try_get("state")?)?)
    }

    pub async fn create(&self, input: CreateProject) -> AppResult<Project> {
        if input.title.trim().is_empty() || !matches!(input.frame_rate, 30 | 60) {
            return Err(AppError::Invalid(
                "title is required and frame_rate must be 30 or 60".into(),
            ));
        }
        let project = Project {
            id: uuid::Uuid::new_v4().to_string(),
            title: input.title.trim().into(),
            revision: "0".into(),
            frame_rate: input.frame_rate,
            updated_at: Utc::now().to_rfc3339(),
            brief: String::new(),
            sources: vec![],
            source_sessions: vec![],
            scenes: vec![],
            feedback: vec![],
            change_sets: vec![],
            renders: vec![],
            can_undo: false,
            can_redo: false,
        };
        let history = History {
            undo: vec![],
            redo: vec![],
        };
        sqlx::query("INSERT INTO projects(id,state,history,updated_at) VALUES(?,?,?,?)")
            .bind(&project.id)
            .bind(serde_json::to_string(&project)?)
            .bind(serde_json::to_string(&history)?)
            .bind(&project.updated_at)
            .execute(&self.pool)
            .await?;
        Ok(project)
    }

    pub async fn command(&self, id: &str, request: &CommandRequest) -> AppResult<Project> {
        let request_json = serde_json::to_string(request)?;
        let mut tx = self.pool.begin().await?;
        if let Some(row) =
            sqlx::query("SELECT request,result FROM commands WHERE project_id=? AND command_id=?")
                .bind(id)
                .bind(&request.command_id)
                .fetch_optional(&mut *tx)
                .await?
        {
            if row.try_get::<String, _>("request")? != request_json {
                return Err(AppError::Conflict(
                    "command_id reused with different payload".into(),
                ));
            }
            return Ok(serde_json::from_str(row.try_get("result")?)?);
        }
        let row = sqlx::query("SELECT state,history FROM projects WHERE id=?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let mut project: Project = serde_json::from_str(row.try_get("state")?)?;
        let mut history: History = serde_json::from_str(row.try_get("history")?)?;
        if project.revision != request.base_revision {
            return Err(AppError::Stale);
        }
        domain::apply(&mut project, &mut history, request.command.clone())?;
        project.updated_at = Utc::now().to_rfc3339();
        project.can_undo = !history.undo.is_empty();
        project.can_redo = !history.redo.is_empty();
        let state = serde_json::to_string(&project)?;
        sqlx::query("UPDATE projects SET state=?,history=?,updated_at=? WHERE id=?")
            .bind(&state)
            .bind(serde_json::to_string(&history)?)
            .bind(&project.updated_at)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO commands(project_id,command_id,request,result) VALUES(?,?,?,?)")
            .bind(id)
            .bind(&request.command_id)
            .bind(request_json)
            .bind(&state)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(project)
    }
}
