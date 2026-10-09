use chrono::Utc;
use sqlx::Row;

use super::{Store, history_snapshot::StoredHistory};
use crate::{
    domain,
    error::{AppError, AppResult},
    model::*,
};

impl Store {
    pub async fn create_import(&self, project_id: &str) -> AppResult<SessionImportJob> {
        self.get(project_id).await?;
        let job = SessionImportJob {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            status: ImportStatus::Validating,
            created_at: Utc::now().to_rfc3339(),
            error: None,
            session: None,
        };
        sqlx::query("INSERT INTO session_imports(id,project_id,job) VALUES(?,?,?)")
            .bind(&job.id)
            .bind(project_id)
            .bind(serde_json::to_string(&job)?)
            .execute(&self.pool)
            .await?;
        Ok(job)
    }

    pub async fn list_imports(&self, project_id: &str) -> AppResult<Vec<SessionImportJob>> {
        self.get(project_id).await?;
        let rows =
            sqlx::query("SELECT job FROM session_imports WHERE project_id=? ORDER BY rowid DESC")
                .bind(project_id)
                .fetch_all(&self.pool)
                .await?;
        rows.into_iter()
            .map(|row| Ok(serde_json::from_str(row.try_get("job")?)?))
            .collect()
    }

    pub async fn get_import(&self, project_id: &str, id: &str) -> AppResult<SessionImportJob> {
        let job: String =
            sqlx::query_scalar("SELECT job FROM session_imports WHERE project_id=? AND id=?")
                .bind(project_id)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(AppError::NotFound)?;
        Ok(serde_json::from_str(&job)?)
    }

    pub async fn prepare_import(
        &self,
        job: &SessionImportJob,
        prepared: &PreparedImport,
    ) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;
        let current: String =
            sqlx::query_scalar("SELECT job FROM session_imports WHERE id=? AND project_id=?")
                .bind(&job.id)
                .bind(&job.project_id)
                .fetch_one(&mut *tx)
                .await?;
        let current: SessionImportJob = serde_json::from_str(&current)?;
        if current.status != ImportStatus::Validating {
            return Err(AppError::Conflict("import is no longer validating".into()));
        }
        let mut ready = job.clone();
        ready.status = ImportStatus::Prepared;
        ready.session = Some(prepared.record.summary.clone());
        sqlx::query("UPDATE session_imports SET job=?,prepared=? WHERE id=?")
            .bind(serde_json::to_string(&ready)?)
            .bind(serde_json::to_string(prepared)?)
            .bind(&job.id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn transition_import(
        &self,
        project_id: &str,
        id: &str,
        status: ImportStatus,
        error: Option<String>,
    ) -> AppResult<SessionImportJob> {
        let mut tx = self.pool.begin().await?;
        let text: String =
            sqlx::query_scalar("SELECT job FROM session_imports WHERE project_id=? AND id=?")
                .bind(project_id)
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppError::NotFound)?;
        let mut job: SessionImportJob = serde_json::from_str(&text)?;
        if matches!(
            job.status,
            ImportStatus::Confirmed | ImportStatus::Cancelled
        ) {
            return Err(AppError::Conflict("import is already terminal".into()));
        }
        if status == ImportStatus::Publishing && job.status != ImportStatus::Prepared {
            return Err(AppError::Conflict("import is not prepared".into()));
        }
        if status == ImportStatus::Cancelled && job.status == ImportStatus::Publishing {
            return Err(AppError::Conflict("import is being committed".into()));
        }
        job.status = status;
        job.error = error;
        sqlx::query("UPDATE session_imports SET job=? WHERE id=?")
            .bind(serde_json::to_string(&job)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(job)
    }

    pub async fn prepared_import(&self, project_id: &str, id: &str) -> AppResult<PreparedImport> {
        let text: Option<String> =
            sqlx::query_scalar("SELECT prepared FROM session_imports WHERE project_id=? AND id=?")
                .bind(project_id)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(AppError::NotFound)?;
        Ok(serde_json::from_str(&text.ok_or_else(|| {
            AppError::Conflict("import is not prepared".into())
        })?)?)
    }

    pub async fn commit_import(
        &self,
        job: &SessionImportJob,
        prepared: &PreparedImport,
        revision: &str,
    ) -> AppResult<Project> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT state,history FROM projects WHERE id=?")
            .bind(&job.project_id)
            .fetch_one(&mut *tx)
            .await?;
        let mut project: Project = serde_json::from_str(row.try_get("state")?)?;
        let mut history = StoredHistory::parse(row.try_get("history")?)?;
        if project.revision != revision {
            return Err(AppError::Stale);
        }
        let active: String =
            sqlx::query_scalar("SELECT job FROM session_imports WHERE id=? AND project_id=?")
                .bind(&job.id)
                .bind(&job.project_id)
                .fetch_one(&mut *tx)
                .await?;
        let active: SessionImportJob = serde_json::from_str(&active)?;
        if active.status != ImportStatus::Publishing {
            return Err(AppError::Conflict("import is not publishing".into()));
        }
        if prepared.record.summary.id != job.id {
            return Err(AppError::Invalid(
                "session identity does not match import job".into(),
            ));
        }
        let mut source_ids: std::collections::HashSet<_> = project
            .sources
            .iter()
            .map(|source| source.id.as_str())
            .collect();
        for source in &prepared.sources {
            if !source_ids.insert(&source.id) {
                return Err(AppError::Conflict(
                    "duplicate editor source identity".into(),
                ));
            }
        }
        if prepared
            .record
            .summary
            .sources
            .iter()
            .filter_map(|source| source.editor_source_id.as_ref())
            .any(|id| !prepared.sources.iter().any(|source| &source.id == id))
        {
            return Err(AppError::Invalid(
                "session source binding is unresolved".into(),
            ));
        }
        if project
            .source_sessions
            .iter()
            .any(|session| session.session_id == prepared.record.summary.session_id)
        {
            return Err(AppError::Conflict(
                "this source session is already imported".into(),
            ));
        }
        history.record_edit(project.clone());
        project.sources.extend(prepared.sources.clone());
        project
            .source_sessions
            .push(prepared.record.summary.clone());
        project.revision = domain::next_revision(&project.revision)?;
        project.updated_at = Utc::now().to_rfc3339();
        project.can_undo = true;
        project.can_redo = false;
        let state = serde_json::to_string(&project)?;
        let history = history.json(&mut tx, &job.project_id).await?;
        let result = super::history_snapshot::project_json(&project)?;
        sqlx::query("INSERT INTO source_sessions(id,project_id,record) VALUES(?,?,?)")
            .bind(&prepared.record.summary.id)
            .bind(&job.project_id)
            .bind(serde_json::to_string(&prepared.record)?)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE projects SET state=?,history=?,updated_at=? WHERE id=?")
            .bind(&state)
            .bind(history)
            .bind(&project.updated_at)
            .bind(&job.project_id)
            .execute(&mut *tx)
            .await?;
        let mut confirmed = job.clone();
        confirmed.status = ImportStatus::Confirmed;
        confirmed.session = Some(prepared.record.summary.clone());
        sqlx::query("UPDATE session_imports SET job=? WHERE id=?")
            .bind(serde_json::to_string(&confirmed)?)
            .bind(&job.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO commands(project_id,command_id,request,result) VALUES(?,?,?,?)").bind(&job.project_id).bind(format!("session-import:{}", job.id)).bind(serde_json::json!({"type":"import_session","import_id":job.id,"base_revision":revision}).to_string()).bind(result).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(project)
    }

    pub async fn recovery_imports(&self) -> AppResult<Vec<SessionImportJob>> {
        let rows = sqlx::query("SELECT job FROM session_imports")
            .fetch_all(&self.pool)
            .await?;
        let jobs: Vec<SessionImportJob> = rows
            .into_iter()
            .map(|row| Ok(serde_json::from_str(row.try_get("job")?)?))
            .collect::<AppResult<_>>()?;
        Ok(jobs)
    }
}
