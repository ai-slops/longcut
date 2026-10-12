use std::path::Path;

use super::Store;
use crate::error::{AppError, AppResult};

impl Store {
    pub(super) async fn migrate_story_versions(&self, path: &Path) -> AppResult<()> {
        let version: i64 = sqlx::query_scalar("PRAGMA user_version")
            .fetch_one(&self.pool)
            .await?;
        match version {
            9 => return Ok(()),
            8 => {}
            _ => {
                return Err(AppError::Invalid(
                    "unsupported story schema migration".into(),
                ));
            }
        }
        let projects: i64 = sqlx::query_scalar("SELECT count(*) FROM projects")
            .fetch_one(&self.pool)
            .await?;
        if projects > 0 {
            let backup = path.with_extension(format!(
                "before-story-versions-{}.sqlite3",
                uuid::Uuid::new_v4()
            ));
            sqlx::query("VACUUM INTO ?")
                .bind(backup.to_string_lossy().as_ref())
                .execute(&self.pool)
                .await?;
        }
        let mut tx = self.pool.begin().await?;
        sqlx::query("CREATE TABLE IF NOT EXISTS story_versions(project_id TEXT NOT NULL,version TEXT NOT NULL,snapshot TEXT NOT NULL,PRIMARY KEY(project_id,version)); PRAGMA user_version=9;")
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
