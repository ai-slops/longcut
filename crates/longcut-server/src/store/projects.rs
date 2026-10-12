use chrono::Utc;
use sqlx::Row;

use super::{Store, history_snapshot};
use crate::{
    error::{AppError, AppResult},
    model::{CreateProject, Project, ProjectSummary, Story},
};

impl Store {
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
            moments: vec![],
            story: Story::default(),
            scenes: vec![],
            feedback: vec![],
            change_sets: vec![],
            renders: vec![],
            can_undo: false,
            can_redo: false,
        };
        let history = history_snapshot::StoredHistory::empty();
        sqlx::query("INSERT INTO projects(id,state,history,updated_at) VALUES(?,?,?,?)")
            .bind(&project.id)
            .bind(serde_json::to_string(&project)?)
            .bind(serde_json::to_string(&history)?)
            .bind(&project.updated_at)
            .execute(&self.pool)
            .await?;
        Ok(project)
    }
}
