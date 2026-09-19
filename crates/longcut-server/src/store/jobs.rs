use chrono::Utc;
use sqlx::Row;

use super::Store;
use crate::{
    domain,
    error::{AppError, AppResult},
    model::*,
};

impl Store {
    pub async fn add_source(&self, id: &str, source: Source) -> AppResult<Project> {
        self.mutate(id, true, move |project| {
            project.sources.push(source);
            Ok(())
        })
        .await
    }

    pub async fn update_source(&self, id: &str, source: Source) -> AppResult<Project> {
        self.mutate(id, false, move |project| {
            let target = project
                .sources
                .iter_mut()
                .find(|item| item.id == source.id)
                .ok_or(AppError::NotFound)?;
            if target.status != MediaStatus::Processing {
                return Err(AppError::Conflict(
                    "source processing is no longer active".into(),
                ));
            }
            *target = source;
            Ok(())
        })
        .await
    }

    pub async fn enqueue_render(
        &self,
        id: &str,
        render: Render,
        base_revision: &str,
    ) -> AppResult<(Render, Project)> {
        let output = render.clone();
        let render_id = render.id.clone();
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT state FROM projects WHERE id=?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let mut project: Project = serde_json::from_str(row.try_get("state")?)?;
        if project.revision != base_revision {
            return Err(AppError::Stale);
        }
        let snapshot = project.clone();
        let snapshot_json = serde_json::to_string(&snapshot)?;
        project.renders.push(render);
        sqlx::query("UPDATE projects SET state=? WHERE id=?")
            .bind(serde_json::to_string(&project)?)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO render_jobs(id,project_id,snapshot,status) VALUES(?,?,?,'queued')",
        )
        .bind(render_id)
        .bind(id)
        .bind(snapshot_json)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok((output, snapshot))
    }

    pub async fn update_render(&self, project_id: &str, render: Render) -> AppResult<Project> {
        let status = match render.status {
            RenderStatus::Queued => "queued",
            RenderStatus::Running => "running",
            RenderStatus::Ready => "ready",
            RenderStatus::Error => "error",
        };
        let render_id = render.id.clone();
        let project = self
            .mutate(project_id, false, move |project| {
                let target = project
                    .renders
                    .iter_mut()
                    .find(|item| item.id == render.id)
                    .ok_or(AppError::NotFound)?;
                *target = render;
                Ok(())
            })
            .await?;
        sqlx::query("UPDATE render_jobs SET status=? WHERE id=?")
            .bind(status)
            .bind(render_id)
            .execute(&self.pool)
            .await?;
        Ok(project)
    }

    async fn mutate<F>(&self, id: &str, semantic: bool, update: F) -> AppResult<Project>
    where
        F: FnOnce(&mut Project) -> AppResult<()>,
    {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT state,history FROM projects WHERE id=?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let mut project: Project = serde_json::from_str(row.try_get("state")?)?;
        let mut history: History = serde_json::from_str(row.try_get("history")?)?;
        if semantic {
            history.undo.push(project.clone());
            history.redo.clear();
        }
        update(&mut project)?;
        if semantic {
            project.revision = domain::next_revision(&project.revision)?;
        }
        project.updated_at = Utc::now().to_rfc3339();
        project.can_undo = !history.undo.is_empty();
        project.can_redo = !history.redo.is_empty();
        sqlx::query("UPDATE projects SET state=?,history=?,updated_at=? WHERE id=?")
            .bind(serde_json::to_string(&project)?)
            .bind(serde_json::to_string(&history)?)
            .bind(&project.updated_at)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(project)
    }
}
