mod review;
mod scene;
mod scene_proposal;

use crate::{
    error::{AppError, AppResult},
    model::*,
};

pub fn next_revision(revision: &str) -> AppResult<String> {
    revision
        .parse::<u64>()
        .map(|value| (value + 1).to_string())
        .map_err(|_| AppError::Internal(anyhow::anyhow!("invalid persisted revision")))
}

pub fn apply(project: &mut Project, history: &mut History, command: Command) -> AppResult<()> {
    match command {
        Command::Undo => undo(project, history),
        Command::Redo => redo(project, history),
        other => {
            let before = project.clone();
            apply_edit(project, other)?;
            project.revision = next_revision(&before.revision)?;
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

fn apply_edit(project: &mut Project, command: Command) -> AppResult<()> {
    match command {
        Command::SetBrief { brief } => project.brief = brief,
        Command::RenameProject { title } => {
            if title.trim().is_empty() {
                return Err(AppError::Invalid("title is required".into()));
            }
            project.title = title.trim().into();
        }
        Command::SetSourceRole { source_id, role } => {
            scene::source_mut(project, &source_id)?.role = role
        }
        Command::InsertScene {
            source_id,
            in_frame,
            out_frame,
            title,
        } => scene::insert(project, source_id, in_frame, out_frame, title)?,
        Command::TrimScene {
            scene_id,
            in_frame,
            out_frame,
        } => scene::trim(project, &scene_id, &in_frame, &out_frame, true)?,
        Command::SplitScene {
            scene_id,
            offset_frame,
        } => scene::split(project, &scene_id, &offset_frame)?,
        Command::MoveScene {
            scene_id,
            before_scene_id,
        } => scene::move_to(project, &scene_id, before_scene_id.as_deref())?,
        Command::DeleteScene { scene_id } => scene::delete(project, &scene_id)?,
        Command::UpdateScene {
            scene_id,
            title,
            purpose,
            caption,
        } => {
            let target = scene::editable(project, &scene_id)?;
            target.title = title;
            target.purpose = purpose;
            target.caption = caption;
            target.manual = true;
        }
        Command::SetCamera { scene_id, camera } => {
            scene::set_camera(project, &scene_id, camera, true)?
        }
        Command::LockScene { scene_id, locked } => {
            scene::scene_mut(project, &scene_id)?.locked = locked
        }
        Command::CreateFeedback {
            scene_id,
            start_frame,
            end_frame,
            body,
            pin,
            render_id,
        } => {
            review::create_feedback(
                project,
                scene_id,
                start_frame,
                end_frame,
                body,
                pin,
                render_id,
            )?;
        }
        Command::ResolveFeedback {
            feedback_id,
            resolved,
        } => {
            project
                .feedback
                .iter_mut()
                .find(|item| item.id == feedback_id)
                .ok_or(AppError::NotFound)?
                .resolved = resolved;
        }
        Command::CreateChangeSet {
            title,
            intent,
            operations,
            evidence,
        } => review::create_change_set(project, title, intent, operations, evidence)?,
        Command::ApplyChangeSet {
            change_set_id,
            operation_ids,
        } => review::apply_change_set(project, &change_set_id, &operation_ids)?,
        Command::RejectChangeSet { change_set_id } => {
            review::reject_change_set(project, &change_set_id)?
        }
        Command::Undo | Command::Redo => {
            return Err(AppError::Internal(anyhow::anyhow!(
                "history command reached edit dispatcher"
            )));
        }
    }
    Ok(())
}
