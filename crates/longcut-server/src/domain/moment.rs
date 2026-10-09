use crate::{
    error::{AppError, AppResult},
    model::{
        MAX_MOMENT_LABEL_BYTES, MAX_MOMENTS, Moment, MomentNarrativeRole, MomentOrigin,
        MomentProvenance, MomentReviewStatus, MomentVersionRef, Project, Scene,
    },
};

use super::ApplyContext;

pub(super) struct MomentFields {
    pub source_id: String,
    pub start_frame: String,
    pub end_frame: String,
    pub label: String,
    pub note: String,
    pub narrative_role: MomentNarrativeRole,
}

pub(super) struct MomentTarget {
    pub moment_id: String,
    pub expected_version: String,
}

pub(super) struct MomentMutation<'a> {
    pub target: MomentTarget,
    pub context: &'a ApplyContext,
}

pub(super) fn create(
    project: &mut Project,
    fields: MomentFields,
    context: &ApplyContext,
) -> AppResult<()> {
    if project.moments.len() >= MAX_MOMENTS {
        return Err(AppError::Invalid("moment limit reached".into()));
    }
    super::moment_validation::validate_fields(project, &fields)?;
    let evidence = context.evidence.clone().unwrap_or_default();
    super::moment_validation::validate_evidence(&evidence, &fields.start_frame, &fields.end_frame)?;
    project.moments.push(Moment {
        id: uuid::Uuid::new_v4().to_string(),
        version: context.revision.clone(),
        source_id: fields.source_id,
        start_frame: fields.start_frame,
        end_frame: fields.end_frame,
        label: fields.label,
        note: fields.note,
        narrative_role: fields.narrative_role,
        origin: MomentOrigin::Manual,
        review_status: MomentReviewStatus::Draft,
        archived: false,
        evidence,
        provenance: MomentProvenance {
            created_command_id: context.command_id.clone(),
            updated_command_id: context.command_id.clone(),
            created_revision: context.revision.clone(),
            updated_revision: context.revision.clone(),
            created_at: context.timestamp.clone(),
            updated_at: context.timestamp.clone(),
        },
    });
    Ok(())
}

pub(super) fn update(
    project: &mut Project,
    mutation: MomentMutation<'_>,
    fields: MomentFields,
) -> AppResult<()> {
    preflight_update(
        project,
        &mutation.target.moment_id,
        &mutation.target.expected_version,
    )?;
    super::moment_validation::validate_fields(project, &fields)?;
    let evidence = mutation.context.evidence.clone().unwrap_or_default();
    super::moment_validation::validate_evidence(&evidence, &fields.start_frame, &fields.end_frame)?;
    let target = find_mut(
        project,
        &mutation.target.moment_id,
        &mutation.target.expected_version,
    )?;
    target.source_id = fields.source_id;
    target.start_frame = fields.start_frame;
    target.end_frame = fields.end_frame;
    target.label = fields.label;
    target.note = fields.note;
    target.narrative_role = fields.narrative_role;
    target.evidence = evidence;
    target.review_status = MomentReviewStatus::Draft;
    touch(target, mutation.context);
    Ok(())
}

pub(super) fn preflight_update(project: &Project, id: &str, version: &str) -> AppResult<()> {
    let moment = find(project, id, version)?;
    if moment.archived {
        return Err(AppError::Conflict(
            "archived moment must be restored before update".into(),
        ));
    }
    Ok(())
}

pub(super) fn review(
    project: &mut Project,
    mutation: MomentMutation<'_>,
    status: MomentReviewStatus,
) -> AppResult<()> {
    let target = find_mut(
        project,
        &mutation.target.moment_id,
        &mutation.target.expected_version,
    )?;
    if target.archived {
        return Err(AppError::Conflict(
            "archived moment cannot be reviewed".into(),
        ));
    }
    target.review_status = status;
    touch(target, mutation.context);
    Ok(())
}

pub(super) fn set_archived(
    project: &mut Project,
    mutation: MomentMutation<'_>,
    archived: bool,
) -> AppResult<()> {
    let target = find_mut(
        project,
        &mutation.target.moment_id,
        &mutation.target.expected_version,
    )?;
    if target.archived == archived {
        return Err(AppError::Conflict(
            "moment archive state is unchanged".into(),
        ));
    }
    target.archived = archived;
    touch(target, mutation.context);
    Ok(())
}

pub(super) fn insert_scene(
    project: &mut Project,
    target: MomentTarget,
    title: String,
) -> AppResult<()> {
    if title.trim().is_empty() || title.len() > MAX_MOMENT_LABEL_BYTES {
        return Err(AppError::Invalid(
            "a bounded scene title is required".into(),
        ));
    }
    let moment = find(project, &target.moment_id, &target.expected_version)?;
    if moment.archived || moment.review_status != MomentReviewStatus::Reviewed {
        return Err(AppError::Conflict(
            "only reviewed, non-archived moments can become scenes".into(),
        ));
    }
    super::scene::validate_range(
        project,
        &moment.source_id,
        &moment.start_frame,
        &moment.end_frame,
    )?;
    project.scenes.push(Scene {
        id: uuid::Uuid::new_v4().to_string(),
        title: title.trim().into(),
        purpose: String::new(),
        source_id: moment.source_id.clone(),
        in_frame: moment.start_frame.clone(),
        out_frame: moment.end_frame.clone(),
        camera: None,
        caption: String::new(),
        locked: false,
        manual: true,
        moment: Some(MomentVersionRef {
            moment_id: moment.id.clone(),
            version: moment.version.clone(),
        }),
    });
    Ok(())
}

fn find<'a>(project: &'a Project, id: &str, version: &str) -> AppResult<&'a Moment> {
    let moment = project
        .moments
        .iter()
        .find(|moment| moment.id == id)
        .ok_or(AppError::NotFound)?;
    require_version(moment, version)?;
    Ok(moment)
}

fn find_mut<'a>(project: &'a mut Project, id: &str, version: &str) -> AppResult<&'a mut Moment> {
    let moment = project
        .moments
        .iter_mut()
        .find(|moment| moment.id == id)
        .ok_or(AppError::NotFound)?;
    require_version(moment, version)?;
    Ok(moment)
}

fn require_version(moment: &Moment, version: &str) -> AppResult<()> {
    if moment.version != version {
        return Err(AppError::Conflict("moment version changed".into()));
    }
    Ok(())
}

fn touch(moment: &mut Moment, context: &ApplyContext) {
    moment.version.clone_from(&context.revision);
    moment
        .provenance
        .updated_command_id
        .clone_from(&context.command_id);
    moment
        .provenance
        .updated_revision
        .clone_from(&context.revision);
    moment.provenance.updated_at.clone_from(&context.timestamp);
}
