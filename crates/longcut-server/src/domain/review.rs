use std::collections::HashSet;

use super::scene;
use crate::{
    error::{AppError, AppResult},
    model::*,
};

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
    if operations.is_empty() {
        return Err(AppError::Invalid("change set requires operations".into()));
    }
    let mut ids = HashSet::new();
    let mut preconditions = Vec::with_capacity(operations.len());
    for operation in &operations {
        if !ids.insert(&operation.id) {
            return Err(AppError::Invalid("duplicate operation id".into()));
        }
        validate_operation(project, operation)?;
        preconditions.push(capture_precondition(project, operation)?);
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
    if requested.is_empty() {
        return Err(AppError::Invalid("operation_ids is required".into()));
    }
    if requested.iter().collect::<HashSet<_>>().len() != requested.len() {
        return Err(AppError::Invalid("duplicate operation id".into()));
    }
    let change_set = project
        .change_sets
        .iter()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)?;
    if change_set.status == ChangeSetStatus::Rejected {
        return Err(AppError::Invalid("change set rejected".into()));
    }
    let mut selected = Vec::new();
    for operation_id in requested {
        if change_set.applied_operation_ids.contains(operation_id) {
            return Err(AppError::Invalid("operation already applied".into()));
        }
        let operation = change_set
            .operations
            .iter()
            .find(|item| &item.id == operation_id)
            .ok_or(AppError::NotFound)?
            .clone();
        let precondition = change_set
            .preconditions
            .iter()
            .find(|item| item.operation_id() == operation_id)
            .ok_or_else(|| AppError::Conflict("operation has no captured precondition".into()))?
            .clone();
        selected.push((operation, precondition));
    }
    for (operation, precondition) in &selected {
        validate_precondition(project, precondition)?;
        validate_operation(project, operation)?;
    }
    let mut candidate = project.clone();
    for (operation, _) in &selected {
        match operation.kind {
            OperationType::SetCamera => scene::set_camera(
                &mut candidate,
                &operation.scene_id,
                operation.camera.clone(),
                false,
            )?,
            OperationType::TrimScene => scene::trim(
                &mut candidate,
                &operation.scene_id,
                operation
                    .in_frame
                    .as_deref()
                    .ok_or_else(|| AppError::Invalid("in_frame required".into()))?,
                operation
                    .out_frame
                    .as_deref()
                    .ok_or_else(|| AppError::Invalid("out_frame required".into()))?,
                false,
            )?,
        }
    }
    let change_set = candidate
        .change_sets
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)?;
    change_set
        .applied_operation_ids
        .extend(requested.iter().cloned());
    change_set.status = if change_set.applied_operation_ids.len() == change_set.operations.len() {
        ChangeSetStatus::Applied
    } else {
        ChangeSetStatus::PartiallyApplied
    };
    *project = candidate;
    Ok(())
}

fn capture_precondition(
    project: &Project,
    operation: &Operation,
) -> AppResult<OperationPrecondition> {
    let target = scene::get(project, &operation.scene_id)?;
    Ok(match operation.kind {
        OperationType::SetCamera => OperationPrecondition::SetCamera {
            operation_id: operation.id.clone(),
            scene_id: operation.scene_id.clone(),
            camera: target.camera.clone(),
        },
        OperationType::TrimScene => OperationPrecondition::TrimScene {
            operation_id: operation.id.clone(),
            scene_id: operation.scene_id.clone(),
            in_frame: target.in_frame.clone(),
            out_frame: target.out_frame.clone(),
        },
    })
}

fn validate_precondition(project: &Project, value: &OperationPrecondition) -> AppResult<()> {
    let matches = match value {
        OperationPrecondition::SetCamera {
            scene_id, camera, ..
        } => scene::get(project, scene_id)?.camera == *camera,
        OperationPrecondition::TrimScene {
            scene_id,
            in_frame,
            out_frame,
            ..
        } => {
            let target = scene::get(project, scene_id)?;
            target.in_frame == *in_frame && target.out_frame == *out_frame
        }
    };
    if !matches {
        return Err(AppError::Conflict(
            "operation target changed after proposal creation".into(),
        ));
    }
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

fn validate_operation(project: &Project, operation: &Operation) -> AppResult<()> {
    let target = scene::get(project, &operation.scene_id)?;
    if target.locked {
        return Err(AppError::Locked(operation.scene_id.clone()));
    }
    match operation.kind {
        OperationType::SetCamera => {
            if let Some(camera) = &operation.camera {
                scene::source(project, &camera.source_id)?;
            }
        }
        OperationType::TrimScene => scene::validate_range(
            project,
            &target.source_id,
            operation
                .in_frame
                .as_deref()
                .ok_or_else(|| AppError::Invalid("in_frame required".into()))?,
            operation
                .out_frame
                .as_deref()
                .ok_or_else(|| AppError::Invalid("out_frame required".into()))?,
        )?,
    }
    Ok(())
}
