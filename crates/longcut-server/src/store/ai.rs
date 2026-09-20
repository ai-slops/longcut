use chrono::Utc;
use sqlx::Row;

use super::Store;
use crate::{
    ai::{AiRequest, AiRun, AiRunStatus, validate_request},
    error::{AppError, AppResult},
    model::Project,
};

impl Store {
    pub async fn enqueue_ai(&self, project_id: &str, request: AiRequest) -> AppResult<AiRun> {
        let mut tx = self.pool.begin().await?;
        let project_row = sqlx::query("SELECT state FROM projects WHERE id=?")
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let project: Project = serde_json::from_str(project_row.try_get("state")?)?;
        validate_request(&project, &request)?;
        let active = sqlx::query(
            "SELECT 1 FROM agent_runs WHERE status IN ('queued','sampling','generating') LIMIT 1",
        )
        .fetch_optional(&mut *tx)
        .await?;
        if active.is_some() {
            return Err(AppError::Conflict("another AI job is active".into()));
        }
        let now = Utc::now().to_rfc3339();
        let run = AiRun {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.into(),
            base_revision: request.base_revision.clone(),
            instruction: request.instruction.clone(),
            scope: request.scope.clone(),
            status: AiRunStatus::Queued,
            created_at: now.clone(),
            updated_at: now.clone(),
            change_set_id: None,
            summary: None,
            warnings: vec![],
            sample_count: 0,
            error: None,
        };
        sqlx::query(
            "INSERT INTO agent_runs(id,project_id,run,request,status,created_at) VALUES(?,?,?,?,?,?)",
        )
        .bind(&run.id)
        .bind(project_id)
        .bind(serde_json::to_string(&run)?)
        .bind(serde_json::to_string(&request)?)
        .bind(status_name(run.status))
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(run)
    }

    pub async fn list_ai_runs(&self, project_id: &str) -> AppResult<Vec<AiRun>> {
        sqlx::query(
            "SELECT run FROM agent_runs WHERE project_id=? ORDER BY created_at DESC LIMIT 50",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| Ok(serde_json::from_str(row.try_get("run")?)?))
        .collect()
    }

    pub async fn ai_run(&self, project_id: &str, run_id: &str) -> AppResult<AiRun> {
        let row = sqlx::query("SELECT run FROM agent_runs WHERE id=? AND project_id=?")
            .bind(run_id)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(serde_json::from_str(row.try_get("run")?)?)
    }

    pub(crate) async fn ai_request(&self, run_id: &str) -> AppResult<AiRequest> {
        let row = sqlx::query("SELECT request FROM agent_runs WHERE id=?")
            .bind(run_id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(serde_json::from_str(row.try_get("request")?)?)
    }

    pub async fn transition_ai(
        &self,
        run_id: &str,
        next: AiRunStatus,
        sample_count: u8,
    ) -> AppResult<AiRun> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT run FROM agent_runs WHERE id=?")
            .bind(run_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let mut run: AiRun = serde_json::from_str(row.try_get("run")?)?;
        let valid = matches!(
            (run.status, next),
            (AiRunStatus::Queued, AiRunStatus::Sampling)
                | (AiRunStatus::Sampling, AiRunStatus::Generating)
        );
        if !valid {
            return Err(AppError::Conflict("AI run is no longer active".into()));
        }
        run.status = next;
        run.sample_count = sample_count;
        run.updated_at = Utc::now().to_rfc3339();
        update(&mut tx, &run).await?;
        tx.commit().await?;
        Ok(run)
    }

    pub async fn cancel_ai(&self, project_id: &str, run_id: &str) -> AppResult<AiRun> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT run FROM agent_runs WHERE id=? AND project_id=?")
            .bind(run_id)
            .bind(project_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let mut run: AiRun = serde_json::from_str(row.try_get("run")?)?;
        if run.status.active() {
            run.status = AiRunStatus::Cancelled;
            run.updated_at = Utc::now().to_rfc3339();
            update(&mut tx, &run).await?;
        }
        tx.commit().await?;
        Ok(run)
    }

    pub(crate) async fn fail_ai(&self, run_id: &str, message: String) -> AppResult<AiRun> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT run FROM agent_runs WHERE id=?")
            .bind(run_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let mut run: AiRun = serde_json::from_str(row.try_get("run")?)?;
        if run.status.active() {
            run.status = AiRunStatus::Error;
            run.error = Some(message);
            run.updated_at = Utc::now().to_rfc3339();
            update(&mut tx, &run).await?;
        }
        tx.commit().await?;
        Ok(run)
    }

    pub(crate) async fn stale_ai(&self, run_id: &str) -> AppResult<AiRun> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT run FROM agent_runs WHERE id=?")
            .bind(run_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let mut run: AiRun = serde_json::from_str(row.try_get("run")?)?;
        if run.status.active() {
            run.status = AiRunStatus::Stale;
            run.updated_at = Utc::now().to_rfc3339();
            update(&mut tx, &run).await?;
        }
        tx.commit().await?;
        Ok(run)
    }

    pub(super) async fn recover_ai_runs(&self) -> AppResult<()> {
        let rows = sqlx::query(
            "SELECT run FROM agent_runs WHERE status IN ('queued','sampling','generating')",
        )
        .fetch_all(&self.pool)
        .await?;
        for row in rows {
            let mut run: AiRun = serde_json::from_str(row.try_get("run")?)?;
            run.status = AiRunStatus::Error;
            run.updated_at = Utc::now().to_rfc3339();
            run.error = Some("AI run was interrupted; retry the request".into());
            sqlx::query("UPDATE agent_runs SET run=?,status='error' WHERE id=?")
                .bind(serde_json::to_string(&run)?)
                .bind(&run.id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}

async fn update(tx: &mut sqlx::SqliteTransaction<'_>, run: &AiRun) -> AppResult<()> {
    sqlx::query("UPDATE agent_runs SET run=?,status=? WHERE id=?")
        .bind(serde_json::to_string(run)?)
        .bind(status_name(run.status))
        .bind(&run.id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

const fn status_name(status: AiRunStatus) -> &'static str {
    match status {
        AiRunStatus::Queued => "queued",
        AiRunStatus::Sampling => "sampling",
        AiRunStatus::Generating => "generating",
        AiRunStatus::Ready => "ready",
        AiRunStatus::Error => "error",
        AiRunStatus::Cancelled => "cancelled",
        AiRunStatus::Stale => "stale",
    }
}
