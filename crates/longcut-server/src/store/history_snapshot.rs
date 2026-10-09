mod versions;
mod wire;

use serde::{Deserialize, Serialize};

use self::wire::ProjectSnapshot;
use crate::{
    error::{AppError, AppResult},
    model::{Command, History, Project},
};

#[derive(Clone, Copy)]
pub(super) enum HistoryOperation {
    Edit,
    Undo,
    Redo,
}

impl HistoryOperation {
    pub(super) const fn from_command(command: &Command) -> Self {
        match command {
            Command::Undo => Self::Undo,
            Command::Redo => Self::Redo,
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
            | Command::CreateMoment { .. }
            | Command::UpdateMoment { .. }
            | Command::ReviewMoment { .. }
            | Command::ArchiveMoment { .. }
            | Command::RestoreMoment { .. }
            | Command::InsertSceneFromMoment { .. }
            | Command::CreateFeedback { .. }
            | Command::ResolveFeedback { .. }
            | Command::CreateChangeSet { .. }
            | Command::ApplyChangeSet { .. }
            | Command::RejectChangeSet { .. } => Self::Edit,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(untagged)]
enum StoredProject {
    Compact(ProjectSnapshot),
    Legacy(Project),
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct StoredHistory {
    undo: Vec<StoredProject>,
    redo: Vec<StoredProject>,
}

impl StoredHistory {
    pub(super) fn empty() -> Self {
        Self {
            undo: vec![],
            redo: vec![],
        }
    }

    pub(super) fn parse(value: &str) -> AppResult<Self> {
        Ok(serde_json::from_str(value)?)
    }

    pub(super) async fn domain_history(
        &mut self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        project_id: &str,
        operation: HistoryOperation,
    ) -> AppResult<History> {
        match operation {
            HistoryOperation::Edit => Ok(History {
                undo: vec![],
                redo: vec![],
            }),
            HistoryOperation::Undo => {
                let snapshot = self
                    .undo
                    .pop()
                    .ok_or_else(|| AppError::Invalid("nothing to undo".into()))?;
                Ok(History {
                    undo: vec![hydrate(tx, project_id, snapshot).await?],
                    redo: vec![],
                })
            }
            HistoryOperation::Redo => {
                let snapshot = self
                    .redo
                    .pop()
                    .ok_or_else(|| AppError::Invalid("nothing to redo".into()))?;
                Ok(History {
                    undo: vec![],
                    redo: vec![hydrate(tx, project_id, snapshot).await?],
                })
            }
        }
    }

    pub(super) fn merge(
        &mut self,
        history: &mut History,
        operation: HistoryOperation,
    ) -> AppResult<()> {
        match operation {
            HistoryOperation::Edit => {
                self.undo.push(StoredProject::Legacy(pop_required(
                    &mut history.undo,
                    "edit did not produce an undo snapshot",
                )?));
                self.redo.clear();
            }
            HistoryOperation::Undo => self.redo.push(StoredProject::Legacy(pop_required(
                &mut history.redo,
                "undo did not produce a redo snapshot",
            )?)),
            HistoryOperation::Redo => self.undo.push(StoredProject::Legacy(pop_required(
                &mut history.undo,
                "redo did not produce an undo snapshot",
            )?)),
        }
        Ok(())
    }

    pub(super) fn record_edit(&mut self, project: Project) {
        self.undo.push(StoredProject::Legacy(project));
        self.redo.clear();
    }

    pub(super) const fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(super) const fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub(super) async fn json(
        &mut self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        project_id: &str,
    ) -> AppResult<String> {
        let projects: Vec<&Project> = self
            .undo
            .iter()
            .chain(&self.redo)
            .filter_map(|snapshot| match snapshot {
                StoredProject::Compact(_) => None,
                StoredProject::Legacy(project) => Some(project),
            })
            .collect();
        versions::persist_missing(tx, project_id, &projects).await?;
        compact(&mut self.undo);
        compact(&mut self.redo);
        Ok(serde_json::to_string(self)?)
    }
}

pub(super) fn project_json(project: &Project) -> AppResult<String> {
    Ok(serde_json::to_string(&ProjectSnapshot::from_project(
        project.clone(),
    ))?)
}

pub(super) async fn project_from_json(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
    value: &str,
) -> AppResult<(Project, Option<String>)> {
    let stored: StoredProject = serde_json::from_str(value)?;
    match stored {
        StoredProject::Compact(snapshot) => {
            Ok((versions::hydrate(tx, project_id, snapshot).await?, None))
        }
        StoredProject::Legacy(project) => {
            if project.id != project_id {
                return Err(AppError::NotFound);
            }
            versions::persist_missing(tx, project_id, &[&project]).await?;
            let compact = project_json(&project)?;
            Ok((project, Some(compact)))
        }
    }
}

async fn hydrate(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    project_id: &str,
    stored: StoredProject,
) -> AppResult<Project> {
    match stored {
        StoredProject::Compact(snapshot) => versions::hydrate(tx, project_id, snapshot).await,
        StoredProject::Legacy(project) => {
            if project.id != project_id {
                return Err(AppError::NotFound);
            }
            versions::persist_missing(tx, project_id, &[&project]).await?;
            Ok(project)
        }
    }
}

fn compact(snapshots: &mut [StoredProject]) {
    for snapshot in snapshots {
        if let StoredProject::Legacy(project) = snapshot {
            *snapshot = StoredProject::Compact(ProjectSnapshot::from_project(project.clone()));
        }
    }
}

fn pop_required(projects: &mut Vec<Project>, message: &str) -> AppResult<Project> {
    projects
        .pop()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!(message.to_owned())))
}
