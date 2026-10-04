use super::Store;
use crate::{
    error::{AppError, AppResult},
    model::{Project, SourceSessionRecord},
};

impl Store {
    pub async fn source_session(
        &self,
        project_id: &str,
        id: &str,
    ) -> AppResult<SourceSessionRecord> {
        if !self
            .get(project_id)
            .await?
            .source_sessions
            .iter()
            .any(|session| session.id == id)
        {
            return Err(AppError::NotFound);
        }
        self.session_record(project_id, id).await
    }

    pub async fn render_source_session(
        &self,
        project_id: &str,
        render_id: &str,
        session_id: &str,
    ) -> AppResult<SourceSessionRecord> {
        let snapshot: String =
            sqlx::query_scalar("SELECT snapshot FROM render_jobs WHERE project_id=? AND id=?")
                .bind(project_id)
                .bind(render_id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(AppError::NotFound)?;
        let snapshot: Project = serde_json::from_str(&snapshot)?;
        if !snapshot
            .source_sessions
            .iter()
            .any(|session| session.id == session_id)
        {
            return Err(AppError::NotFound);
        }
        self.session_record(project_id, session_id).await
    }

    async fn session_record(&self, project_id: &str, id: &str) -> AppResult<SourceSessionRecord> {
        let text: String =
            sqlx::query_scalar("SELECT record FROM source_sessions WHERE project_id=? AND id=?")
                .bind(project_id)
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .ok_or(AppError::NotFound)?;
        Ok(serde_json::from_str(&text)?)
    }
}
