use super::{
    Store,
    moment_evidence::{EvidenceRequest, resolve_evidence},
};
use crate::{
    error::{AppError, AppResult},
    model::{Command, MomentEvidence, MomentVersionRecord, Project},
};

pub(crate) struct MomentVersionKey<'a> {
    pub project_id: &'a str,
    pub moment_id: &'a str,
    pub version: &'a str,
}

impl Store {
    pub(super) async fn evidence_for_command(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        project: &Project,
        command: &Command,
    ) -> AppResult<Option<Vec<MomentEvidence>>> {
        let fields = match command {
            Command::CreateMoment {
                source_id,
                start_frame,
                end_frame,
                evidence,
                ..
            } => evidence.as_ref().map(|selection| {
                (
                    source_id.as_str(),
                    start_frame.as_str(),
                    end_frame.as_str(),
                    selection,
                )
            }),
            Command::UpdateMoment {
                moment_id,
                expected_moment_version,
                source_id,
                start_frame,
                end_frame,
                evidence,
                ..
            } => {
                crate::domain::preflight_moment_update(
                    project,
                    moment_id,
                    expected_moment_version,
                )?;
                evidence.as_ref().map(|selection| {
                    (
                        source_id.as_str(),
                        start_frame.as_str(),
                        end_frame.as_str(),
                        selection,
                    )
                })
            }
            Command::SetBrief { .. }
            | Command::RenameProject { .. }
            | Command::SetSourceRole { .. }
            | Command::InsertScene { .. }
            | Command::TrimScene { .. }
            | Command::SplitScene { .. }
            | Command::MoveScene { .. }
            | Command::DeleteScene { .. }
            | Command::UpdateScene { .. }
            | Command::SetCamera { .. }
            | Command::LockScene { .. }
            | Command::ReviewMoment { .. }
            | Command::ArchiveMoment { .. }
            | Command::RestoreMoment { .. }
            | Command::InsertSceneFromMoment { .. }
            | Command::CreateFeedback { .. }
            | Command::ResolveFeedback { .. }
            | Command::CreateChangeSet { .. }
            | Command::ApplyChangeSet { .. }
            | Command::RejectChangeSet { .. }
            | Command::Undo
            | Command::Redo => None,
        };
        let Some((source_id, start, end, selection)) = fields else {
            return Ok(None);
        };
        resolve_evidence(
            tx,
            EvidenceRequest {
                project_id: &project.id,
                source_id,
                moment_start: start,
                moment_end: end,
                selection,
            },
        )
        .await
        .map(|evidence| Some(vec![evidence]))
    }

    pub(super) async fn persist_moment_heads(
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        project: &Project,
    ) -> AppResult<()> {
        for moment in project
            .moments
            .iter()
            .filter(|moment| moment.version == project.revision)
        {
            sqlx::query("INSERT INTO moment_versions(project_id,moment_id,version,snapshot) VALUES(?,?,?,?)")
                .bind(&project.id)
                .bind(&moment.id)
                .bind(&moment.version)
                .bind(serde_json::to_string(moment)?)
                .execute(&mut **tx)
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn moment_version(
        &self,
        key: MomentVersionKey<'_>,
    ) -> AppResult<MomentVersionRecord> {
        let snapshot: String = sqlx::query_scalar(
            "SELECT snapshot FROM moment_versions WHERE project_id=? AND moment_id=? AND version=?",
        )
        .bind(key.project_id)
        .bind(key.moment_id)
        .bind(key.version)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(AppError::NotFound)?;
        Ok(MomentVersionRecord {
            project_id: key.project_id.into(),
            moment_id: key.moment_id.into(),
            version: key.version.into(),
            snapshot: serde_json::from_str(&snapshot)?,
        })
    }
}
