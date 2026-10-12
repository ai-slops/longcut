use super::Store;
use crate::{
    error::{AppError, AppResult},
    model::{Project, Story, StoryCommand, StoryVersionRecord},
};

pub(crate) struct StoryVersionKey<'a> {
    pub project_id: &'a str,
    pub version: &'a str,
}

impl Store {
    pub(super) async fn validate_story_command(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        project: &Project,
        command: &StoryCommand,
    ) -> AppResult<()> {
        crate::domain::preflight_story_command(project, command)?;
        for reference in crate::domain::new_story_moment_refs(project, command)? {
            let snapshot: String = sqlx::query_scalar("SELECT snapshot FROM moment_versions WHERE project_id=? AND moment_id=? AND version=?")
                .bind(&project.id)
                .bind(&reference.moment_id)
                .bind(&reference.version)
                .fetch_optional(&mut **tx)
                .await?
                .ok_or_else(|| AppError::Conflict("referenced moment version is not immutable".into()))?;
            let moment: crate::model::Moment = serde_json::from_str(&snapshot)?;
            if moment.id != reference.moment_id || moment.version != reference.version {
                return Err(AppError::Conflict(
                    "referenced moment version identity conflicts".into(),
                ));
            }
            let head = project
                .moments
                .iter()
                .find(|head| head.id == reference.moment_id && head.version == reference.version)
                .ok_or_else(|| {
                    AppError::Conflict("new moment reference must use current head".into())
                })?;
            if moment != *head {
                return Err(AppError::Conflict(
                    "immutable moment version payload conflicts with current head".into(),
                ));
            }
        }
        Ok(())
    }

    pub(super) async fn persist_story_head(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        project: &Project,
    ) -> AppResult<()> {
        if project.story.version == "0" || project.story.version != project.revision {
            return Ok(());
        }
        persist(tx, &project.id, &project.story).await
    }

    pub(crate) async fn story_version(
        &self,
        key: StoryVersionKey<'_>,
    ) -> AppResult<StoryVersionRecord> {
        let snapshot: String = sqlx::query_scalar(
            "SELECT snapshot FROM story_versions WHERE project_id=? AND version=?",
        )
        .bind(key.project_id)
        .bind(key.version)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AppError::NotFound)?;
        Ok(StoryVersionRecord {
            project_id: key.project_id.into(),
            version: key.version.into(),
            snapshot: serde_json::from_str(&snapshot)?,
        })
    }
}

pub(super) async fn persist(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
    story: &Story,
) -> AppResult<()> {
    if story.version == "0" {
        if story.chapters.is_empty() {
            return Ok(());
        }
        return Err(AppError::Conflict(
            "story version zero must remain empty".into(),
        ));
    }
    let existing: Option<String> =
        sqlx::query_scalar("SELECT snapshot FROM story_versions WHERE project_id=? AND version=?")
            .bind(project_id)
            .bind(&story.version)
            .fetch_optional(&mut **tx)
            .await?;
    if let Some(value) = existing {
        let stored: Story = serde_json::from_str(&value)?;
        if stored != *story {
            return Err(AppError::Conflict(format!(
                "immutable story version payload conflicts for {}",
                story.version
            )));
        }
        return Ok(());
    }
    sqlx::query("INSERT INTO story_versions(project_id,version,snapshot) VALUES(?,?,?)")
        .bind(project_id)
        .bind(&story.version)
        .bind(serde_json::to_string(story)?)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

pub(super) async fn hydrate(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
    version: &str,
) -> AppResult<Story> {
    if version == "0" {
        return Ok(Story::default());
    }
    let value: String =
        sqlx::query_scalar("SELECT snapshot FROM story_versions WHERE project_id=? AND version=?")
            .bind(project_id)
            .bind(version)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(AppError::NotFound)?;
    let story: Story = serde_json::from_str(&value)?;
    if story.version != version {
        return Err(AppError::Internal(anyhow::anyhow!(
            "stored story version identity mismatch"
        )));
    }
    Ok(story)
}
