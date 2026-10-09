mod command;
mod moment;
mod moment_validation;
mod review;
mod scene;
mod scene_proposal;

use crate::{
    error::{AppError, AppResult},
    model::*,
};

pub fn next_revision(revision: &str) -> AppResult<String> {
    let value = revision
        .parse::<u64>()
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid persisted revision")))?;
    value
        .checked_add(1)
        .map(|next| next.to_string())
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("persisted revision overflow")))
}

pub fn apply(project: &mut Project, history: &mut History, command: Command) -> AppResult<()> {
    let context = ApplyContext {
        command_id: "domain".into(),
        revision: next_revision(&project.revision)?,
        timestamp: String::new(),
        evidence: None,
    };
    apply_with_context(project, history, command, &context)
}

pub struct ApplyContext {
    pub command_id: String,
    pub revision: String,
    pub timestamp: String,
    pub evidence: Option<Vec<MomentEvidence>>,
}

pub(crate) fn preflight_moment_update(
    project: &Project,
    moment_id: &str,
    expected_version: &str,
) -> AppResult<()> {
    moment::preflight_update(project, moment_id, expected_version)
}

pub fn apply_with_context(
    project: &mut Project,
    history: &mut History,
    command: Command,
    context: &ApplyContext,
) -> AppResult<()> {
    match command {
        Command::Undo => undo(project, history),
        Command::Redo => redo(project, history),
        other => {
            let before = project.clone();
            let revision = next_revision(&before.revision)?;
            if context.revision != revision {
                return Err(AppError::Internal(anyhow::anyhow!(
                    "command context revision mismatch"
                )));
            }
            command::apply_edit(project, other, context)?;
            project.revision = revision;
            history.undo.push(before);
            history.redo.clear();
            Ok(())
        }
    }
}

fn undo(project: &mut Project, history: &mut History) -> AppResult<()> {
    let mut restored = history
        .undo
        .pop()
        .ok_or_else(|| AppError::Invalid("nothing to undo".into()))?;
    let revision = next_revision(&project.revision)?;
    preserve_operational_state(project, &mut restored);
    history.redo.push(project.clone());
    restored.revision = revision;
    *project = restored;
    Ok(())
}

fn redo(project: &mut Project, history: &mut History) -> AppResult<()> {
    let mut restored = history
        .redo
        .pop()
        .ok_or_else(|| AppError::Invalid("nothing to redo".into()))?;
    let revision = next_revision(&project.revision)?;
    preserve_operational_state(project, &mut restored);
    history.undo.push(project.clone());
    restored.revision = revision;
    *project = restored;
    Ok(())
}

fn preserve_operational_state(current: &Project, restored: &mut Project) {
    for source in &mut restored.sources {
        let Some(active) = current.sources.iter().find(|item| item.id == source.id) else {
            continue;
        };
        source.media_url.clone_from(&active.media_url);
        source.proxy_url.clone_from(&active.proxy_url);
        source.thumbnail_url.clone_from(&active.thumbnail_url);
        source.waveform.clone_from(&active.waveform);
        source.status = active.status;
        source.error.clone_from(&active.error);
    }
    restored.renders.clone_from(&current.renders);
}

#[cfg(test)]
mod tests {
    use super::next_revision;

    #[test]
    fn next_revision_rejects_u64_overflow() {
        // Given
        let revision = u64::MAX.to_string();

        // When
        let result = next_revision(&revision);

        // Then
        assert!(result.is_err());
    }
}
