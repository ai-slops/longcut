mod operations;

use std::collections::HashSet;

use crate::{
    error::{AppError, AppResult},
    model::*,
};

use super::scene;

pub(super) fn create_feedback(
    project: &mut Project,
    scene_id: String,
    start: String,
    end: String,
    body: String,
    pin: Option<Pin>,
    render_id: Option<String>,
) -> AppResult<()> {
    let length = scene::scene_len(scene::get(project, &scene_id)?)?;
    let from = scene::frame(&start)?;
    let to = scene::frame(&end)?;
    if body.trim().is_empty() || from >= to || to > length {
        return Err(AppError::Invalid("invalid feedback range or body".into()));
    }
    if let Some(value) = &pin
        && (!(0.0..=1.0).contains(&value.x) || !(0.0..=1.0).contains(&value.y))
    {
        return Err(AppError::Invalid("pin must be normalized".into()));
    }
    if let Some(id) = &render_id
        && !project.renders.iter().any(|item| &item.id == id)
    {
        return Err(AppError::NotFound);
    }
    project.feedback.push(Feedback {
        id: uuid::Uuid::new_v4().to_string(),
        body,
        scene_id,
        start_frame: start,
        end_frame: end,
        revision: super::next_revision(&project.revision)?,
        render_id,
        pin,
        resolved: false,
    });
    Ok(())
}

pub(super) fn create_change_set(
    project: &mut Project,
    title: String,
    intent: String,
    operations: Vec<Operation>,
    evidence: Vec<String>,
) -> AppResult<()> {
    if operations.is_empty() || operations.len() > 24 {
        return Err(AppError::Invalid(
            "change set requires between 1 and 24 operations".into(),
        ));
    }
    let mut operation_ids = HashSet::new();
    let mut properties = HashSet::new();
    let mut preconditions = Vec::with_capacity(operations.len());
    for operation in &operations {
        if operation.id.is_empty() || !operation_ids.insert(&operation.id) {
            return Err(AppError::Invalid("duplicate or empty operation id".into()));
        }
        operations::reserve_properties(operation, &mut properties)?;
        operations::validate_shape(operation)?;
        preconditions.push(operations::capture_precondition(project, operation)?);
    }
    let mut candidate = project.clone();
    for operation in &operations {
        operations::apply(&mut candidate, operation)?;
    }
    project.change_sets.push(ChangeSet {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        intent,
        base_revision: project.revision.clone(),
        operations,
        preconditions,
        status: ChangeSetStatus::Ready,
        applied_operation_ids: vec![],
        evidence,
    });
    Ok(())
}

pub(super) fn apply_change_set(
    project: &mut Project,
    id: &str,
    requested: &[String],
) -> AppResult<()> {
    let requested_ids = requested.iter().collect::<HashSet<_>>();
    if requested.is_empty() || requested_ids.len() != requested.len() {
        return Err(AppError::Invalid(
            "operation_ids must be non-empty and unique".into(),
        ));
    }
    let change_set = project
        .change_sets
        .iter()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)?;
    if change_set.status == ChangeSetStatus::Rejected {
        return Err(AppError::Invalid("change set rejected".into()));
    }
    for operation_id in requested {
        if change_set.applied_operation_ids.contains(operation_id) {
            return Err(AppError::Invalid("operation already applied".into()));
        }
        if !change_set
            .operations
            .iter()
            .any(|operation| &operation.id == operation_id)
        {
            return Err(AppError::NotFound);
        }
    }
    let selected = change_set
        .operations
        .iter()
        .filter(|operation| requested_ids.contains(&operation.id))
        .map(|operation| {
            let precondition = change_set
                .preconditions
                .iter()
                .find(|value| value.operation_id() == operation.id)
                .ok_or_else(|| {
                    AppError::Conflict("operation has no captured precondition".into())
                })?;
            Ok((operation.clone(), precondition.clone()))
        })
        .collect::<AppResult<Vec<_>>>()?;
    for (_, precondition) in &selected {
        operations::validate_precondition(project, precondition)?;
    }
    let mut candidate = project.clone();
    for (operation, _) in &selected {
        operations::apply(&mut candidate, operation)?;
    }
    let change_set = candidate
        .change_sets
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)?;
    change_set
        .applied_operation_ids
        .extend(selected.iter().map(|(operation, _)| operation.id.clone()));
    change_set.status = if change_set.applied_operation_ids.len() == change_set.operations.len() {
        ChangeSetStatus::Applied
    } else {
        ChangeSetStatus::PartiallyApplied
    };
    *project = candidate;
    Ok(())
}

pub(super) fn reject_change_set(project: &mut Project, id: &str) -> AppResult<()> {
    let change_set = project
        .change_sets
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)?;
    if change_set.status == ChangeSetStatus::Applied {
        return Err(AppError::Invalid(
            "applied change set cannot be rejected".into(),
        ));
    }
    change_set.status = ChangeSetStatus::Rejected;
    Ok(())
}
