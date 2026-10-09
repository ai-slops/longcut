use crate::{
    error::{AppError, AppResult},
    model::{Command, MomentEvidenceSelection, Project},
};

use super::{ApplyContext, moment, review, scene};

pub(super) fn apply_edit(
    project: &mut Project,
    command: Command,
    context: &ApplyContext,
) -> AppResult<()> {
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
        } => {
            scene::insert(project, source_id, in_frame, out_frame, title)?;
        }
        Command::TrimScene {
            scene_id,
            in_frame,
            out_frame,
        } => {
            scene::trim(project, &scene_id, &in_frame, &out_frame, true)?;
        }
        Command::SplitScene {
            scene_id,
            offset_frame,
        } => scene::split(project, &scene_id, &offset_frame)?,
        Command::MoveScene {
            scene_id,
            before_scene_id,
        } => {
            scene::move_to(project, &scene_id, before_scene_id.as_deref())?;
        }
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
        Command::CreateMoment {
            source_id,
            start_frame,
            end_frame,
            label,
            note,
            narrative_role,
            evidence,
        } => {
            require_resolved_evidence(evidence.as_ref(), context)?;
            moment::create(
                project,
                moment::MomentFields {
                    source_id,
                    start_frame,
                    end_frame,
                    label,
                    note,
                    narrative_role,
                },
                context,
            )?;
        }
        Command::UpdateMoment {
            moment_id,
            expected_moment_version,
            source_id,
            start_frame,
            end_frame,
            label,
            note,
            narrative_role,
            evidence,
        } => {
            require_resolved_evidence(evidence.as_ref(), context)?;
            moment::update(
                project,
                mutation(moment_id, expected_moment_version, context),
                moment::MomentFields {
                    source_id,
                    start_frame,
                    end_frame,
                    label,
                    note,
                    narrative_role,
                },
            )?;
        }
        Command::ReviewMoment {
            moment_id,
            expected_moment_version,
            review_status,
        } => {
            moment::review(
                project,
                mutation(moment_id, expected_moment_version, context),
                review_status,
            )?;
        }
        Command::ArchiveMoment {
            moment_id,
            expected_moment_version,
        } => {
            moment::set_archived(
                project,
                mutation(moment_id, expected_moment_version, context),
                true,
            )?;
        }
        Command::RestoreMoment {
            moment_id,
            expected_moment_version,
        } => {
            moment::set_archived(
                project,
                mutation(moment_id, expected_moment_version, context),
                false,
            )?;
        }
        Command::InsertSceneFromMoment {
            moment_id,
            expected_moment_version,
            title,
        } => {
            moment::insert_scene(
                project,
                moment::MomentTarget {
                    moment_id,
                    expected_version: expected_moment_version,
                },
                title,
            )?;
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
        } => {
            review::create_change_set(project, title, intent, operations, evidence)?;
        }
        Command::ApplyChangeSet {
            change_set_id,
            operation_ids,
        } => {
            review::apply_change_set(project, &change_set_id, &operation_ids)?;
        }
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

fn mutation<'a>(
    moment_id: String,
    expected_version: String,
    context: &'a ApplyContext,
) -> moment::MomentMutation<'a> {
    moment::MomentMutation {
        target: moment::MomentTarget {
            moment_id,
            expected_version,
        },
        context,
    }
}

fn require_resolved_evidence(
    selection: Option<&MomentEvidenceSelection>,
    context: &ApplyContext,
) -> AppResult<()> {
    if selection.is_some() != context.evidence.is_some() {
        return Err(AppError::Invalid(
            "moment evidence was not resolved by the server".into(),
        ));
    }
    Ok(())
}
