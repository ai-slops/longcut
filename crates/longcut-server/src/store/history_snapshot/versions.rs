use std::collections::HashMap;

use anyhow::Context;

use super::wire::ProjectSnapshot;
use crate::{
    error::{AppError, AppResult},
    model::{Moment, Project},
};

const LOOKUP_SQL: &str =
    "SELECT snapshot FROM moment_versions WHERE project_id=? AND moment_id=? AND version=?";

pub(super) async fn hydrate(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
    snapshot: ProjectSnapshot,
) -> AppResult<Project> {
    let mut moments = Vec::with_capacity(snapshot.refs().len());
    for reference in snapshot.refs() {
        let value: String = sqlx::query_scalar(LOOKUP_SQL)
            .bind(project_id)
            .bind(&reference.moment_id)
            .bind(&reference.version)
            .fetch_optional(&mut **tx)
            .await?
            .ok_or(AppError::NotFound)?;
        let moment: Moment = serde_json::from_str(&value)?;
        if moment.id != reference.moment_id || moment.version != reference.version {
            return Err(AppError::Internal(anyhow::anyhow!(
                "stored moment version identity mismatch"
            )));
        }
        moments.push(moment);
    }
    let project = snapshot.into_project(moments);
    if project.id != project_id {
        return Err(AppError::NotFound);
    }
    Ok(project)
}

pub(super) async fn persist_missing(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
    projects: &[&Project],
) -> AppResult<()> {
    let mut pending: HashMap<(&str, &str), &Moment> = HashMap::new();
    for project in projects {
        if project.id != project_id {
            return Err(AppError::Internal(anyhow::anyhow!(
                "history snapshot belongs to a different project"
            )));
        }
        for moment in &project.moments {
            let key = (moment.id.as_str(), moment.version.as_str());
            if let Some(existing) = pending.insert(key, moment)
                && existing != moment
            {
                return Err(version_conflict(&moment.id, &moment.version));
            }
        }
    }
    if pending.is_empty() {
        return Ok(());
    }
    for ((moment_id, version), moment) in pending {
        let existing: Option<String> = sqlx::query_scalar(LOOKUP_SQL)
            .bind(project_id)
            .bind(moment_id)
            .bind(version)
            .fetch_optional(&mut **tx)
            .await?;
        if let Some(value) = existing {
            let stored: Moment = serde_json::from_str(&value)?;
            if stored != *moment {
                return Err(version_conflict(moment_id, version));
            }
            continue;
        }
        sqlx::query(
            "INSERT INTO moment_versions(project_id,moment_id,version,snapshot) VALUES(?,?,?,?)",
        )
        .bind(project_id)
        .bind(moment_id)
        .bind(version)
        .bind(serde_json::to_string(moment)?)
        .execute(&mut **tx)
        .await
        .with_context(|| format!("persist moment version {moment_id}:{version}"))?;
    }
    Ok(())
}

fn version_conflict(moment_id: &str, version: &str) -> AppError {
    AppError::Conflict(format!(
        "immutable moment version payload conflicts for {moment_id}:{version}"
    ))
}

#[cfg(test)]
mod tests {
    use sqlx::Row;

    use super::LOOKUP_SQL;

    #[tokio::test]
    async fn exact_lookup_uses_the_full_primary_key_index() {
        // Given
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("open database");
        sqlx::query("CREATE TABLE moment_versions(project_id TEXT NOT NULL,moment_id TEXT NOT NULL,version TEXT NOT NULL,snapshot TEXT NOT NULL,PRIMARY KEY(project_id,moment_id,version))")
            .execute(&pool)
            .await
            .expect("create table");
        let explain = format!("EXPLAIN QUERY PLAN {LOOKUP_SQL}");

        // When
        let row = sqlx::query(&explain)
            .bind("project")
            .bind("moment")
            .bind("1")
            .fetch_one(&pool)
            .await
            .expect("query plan");
        let detail: String = row.try_get("detail").expect("plan detail");

        // Then
        assert!(detail.contains("project_id=? AND moment_id=? AND version=?"));
    }
}
