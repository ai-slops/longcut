use chrono::Utc;
use sqlx::Row;

use super::{
    Store,
    history_snapshot::{HistoryOperation, StoredHistory},
};
use crate::{
    ai::{AiProposal, AiRun, AiRunStatus, into_operations},
    domain,
    error::{AppError, AppResult},
    model::{Command, CommandRequest, Project},
};

impl Store {
    pub async fn publish_ai(&self, run_id: &str, proposal: AiProposal) -> AppResult<AiRun> {
        let mut tx = self.pool.begin().await?;
        let run_row = sqlx::query("SELECT run FROM agent_runs WHERE id=?")
            .bind(run_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let mut run: AiRun = serde_json::from_str(run_row.try_get("run")?)?;
        if run.status != AiRunStatus::Generating {
            return Err(AppError::Conflict("AI run is no longer active".into()));
        }
        let project_row = sqlx::query("SELECT state,history FROM projects WHERE id=?")
            .bind(&run.project_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let mut project: Project = serde_json::from_str(project_row.try_get("state")?)?;
        if project.revision != run.base_revision {
            run.status = AiRunStatus::Stale;
            run.error = Some("project revision changed; start a new AI run".into());
            run.updated_at = Utc::now().to_rfc3339();
            persist_run(&mut tx, &run).await?;
            tx.commit().await?;
            return Ok(run);
        }
        run.summary = Some(proposal.summary.clone());
        run.warnings = proposal.warnings.clone();
        if proposal.operations.is_empty() {
            run.status = AiRunStatus::Ready;
            run.updated_at = Utc::now().to_rfc3339();
            persist_run(&mut tx, &run).await?;
            tx.commit().await?;
            return Ok(run);
        }
        let (operations, evidence) = into_operations(&proposal);
        let command = Command::CreateChangeSet {
            title: proposal.title,
            intent: run.instruction.clone(),
            operations,
            evidence,
        };
        let request = CommandRequest {
            command_id: run.id.clone(),
            base_revision: run.base_revision.clone(),
            command,
        };
        let request_json = serde_json::to_string(&request)?;
        let mut stored_history = StoredHistory::parse(project_row.try_get("history")?)?;
        let mut history = stored_history
            .domain_history(&mut tx, &run.project_id, HistoryOperation::Edit)
            .await?;
        domain::apply(&mut project, &mut history, request.command.clone())?;
        stored_history.merge(&mut history, HistoryOperation::Edit)?;
        project.updated_at = Utc::now().to_rfc3339();
        project.can_undo = stored_history.can_undo();
        project.can_redo = stored_history.can_redo();
        let state_json = serde_json::to_string(&project)?;
        let history_json = stored_history.json(&mut tx, &run.project_id).await?;
        let result_json = super::history_snapshot::project_json(&project)?;
        sqlx::query("UPDATE projects SET state=?,history=?,updated_at=? WHERE id=?")
            .bind(&state_json)
            .bind(history_json)
            .bind(&project.updated_at)
            .bind(&run.project_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO commands(project_id,command_id,request,result) VALUES(?,?,?,?)")
            .bind(&run.project_id)
            .bind(&run.id)
            .bind(request_json)
            .bind(result_json)
            .execute(&mut *tx)
            .await?;
        run.change_set_id = project
            .change_sets
            .last()
            .map(|change_set| change_set.id.clone());
        run.status = AiRunStatus::Ready;
        run.updated_at = Utc::now().to_rfc3339();
        persist_run(&mut tx, &run).await?;
        tx.commit().await?;
        Ok(run)
    }
}

async fn persist_run(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, run: &AiRun) -> AppResult<()> {
    sqlx::query("UPDATE agent_runs SET run=?,status=? WHERE id=?")
        .bind(serde_json::to_string(run)?)
        .bind(match run.status {
            AiRunStatus::Queued => "queued",
            AiRunStatus::Sampling => "sampling",
            AiRunStatus::Generating => "generating",
            AiRunStatus::Ready => "ready",
            AiRunStatus::Error => "error",
            AiRunStatus::Cancelled => "cancelled",
            AiRunStatus::Stale => "stale",
        })
        .bind(&run.id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}
