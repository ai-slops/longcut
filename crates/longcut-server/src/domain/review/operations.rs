use std::collections::HashSet;

use crate::{
    error::{AppError, AppResult},
    model::{Operation, OperationPrecondition, OperationType, Project},
};

use super::super::{scene, scene_proposal};

pub(super) fn reserve_properties(
    operation: &Operation,
    reserved: &mut HashSet<(String, &'static str)>,
) -> AppResult<()> {
    let properties: &[&str] = match operation.kind {
        OperationType::InsertScene => &["insert"],
        OperationType::SetCamera => &["camera"],
        OperationType::TrimScene => &["range"],
        OperationType::UpdateScene => &["metadata"],
        OperationType::DeleteScene => &["camera", "range", "metadata"],
    };
    for &property in properties {
        if !reserved.insert((operation.scene_id.clone(), property)) {
            return Err(AppError::Invalid(
                "multiple operations target the same scene property".into(),
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_shape(operation: &Operation) -> AppResult<()> {
    let valid = match operation.kind {
        OperationType::InsertScene => {
            operation.camera.is_none()
                && operation.source_id.is_some()
                && operation.in_frame.is_some()
                && operation.out_frame.is_some()
                && operation.title.is_some()
                && operation.purpose.is_none()
                && operation.caption.is_none()
        }
        OperationType::SetCamera => {
            operation.source_id.is_none()
                && operation.in_frame.is_none()
                && operation.out_frame.is_none()
                && operation.title.is_none()
                && operation.purpose.is_none()
                && operation.caption.is_none()
        }
        OperationType::TrimScene => {
            operation.camera.is_none()
                && operation.source_id.is_none()
                && operation.in_frame.is_some()
                && operation.out_frame.is_some()
                && operation.title.is_none()
                && operation.purpose.is_none()
                && operation.caption.is_none()
        }
        OperationType::UpdateScene => {
            operation.camera.is_none()
                && operation.source_id.is_none()
                && operation.in_frame.is_none()
                && operation.out_frame.is_none()
                && operation.title.is_some()
                && operation.purpose.is_some()
                && operation.caption.is_some()
        }
        OperationType::DeleteScene => {
            operation.camera.is_none()
                && operation.source_id.is_none()
                && operation.in_frame.is_none()
                && operation.out_frame.is_none()
                && operation.title.is_none()
                && operation.purpose.is_none()
                && operation.caption.is_none()
        }
    };
    if operation.scene_id.is_empty() || !valid {
        return Err(AppError::Invalid(
            "operation fields do not match operation type".into(),
        ));
    }
    Ok(())
}

pub(super) fn capture_precondition(
    project: &Project,
    operation: &Operation,
) -> AppResult<OperationPrecondition> {
    Ok(match operation.kind {
        OperationType::InsertScene => {
            if project
                .scenes
                .iter()
                .any(|scene| scene.id == operation.scene_id)
            {
                return Err(AppError::Conflict(
                    "proposed scene id already exists".into(),
                ));
            }
            let source_id = required(&operation.source_id, "source_id")?;
            let in_frame = required(&operation.in_frame, "in_frame")?;
            let out_frame = required(&operation.out_frame, "out_frame")?;
            scene::validate_range(project, source_id, in_frame, out_frame)?;
            OperationPrecondition::InsertScene {
                operation_id: operation.id.clone(),
                scene_id: operation.scene_id.clone(),
                source_id: source_id.into(),
                in_frame: in_frame.into(),
                out_frame: out_frame.into(),
            }
        }
        OperationType::SetCamera => OperationPrecondition::SetCamera {
            operation_id: operation.id.clone(),
            scene_id: operation.scene_id.clone(),
            camera: scene::get(project, &operation.scene_id)?.camera.clone(),
        },
        OperationType::TrimScene => {
            let target = scene::get(project, &operation.scene_id)?;
            OperationPrecondition::TrimScene {
                operation_id: operation.id.clone(),
                scene_id: operation.scene_id.clone(),
                in_frame: target.in_frame.clone(),
                out_frame: target.out_frame.clone(),
            }
        }
        OperationType::UpdateScene => {
            let target = scene::get(project, &operation.scene_id)?;
            OperationPrecondition::UpdateScene {
                operation_id: operation.id.clone(),
                scene_id: operation.scene_id.clone(),
                title: target.title.clone(),
                purpose: target.purpose.clone(),
                caption: target.caption.clone(),
            }
        }
        OperationType::DeleteScene => OperationPrecondition::DeleteScene {
            operation_id: operation.id.clone(),
            scene: Box::new(scene::get(project, &operation.scene_id)?.clone()),
        },
    })
}

pub(super) fn validate_precondition(
    project: &Project,
    value: &OperationPrecondition,
) -> AppResult<()> {
    let matches = match value {
        OperationPrecondition::InsertScene {
            scene_id,
            source_id,
            in_frame,
            out_frame,
            ..
        } => {
            if project.scenes.iter().any(|scene| scene.id == *scene_id) {
                false
            } else {
                scene::validate_range(project, source_id, in_frame, out_frame)?;
                true
            }
        }
        OperationPrecondition::SetCamera {
            scene_id, camera, ..
        } => find_target(project, scene_id)?.camera == *camera,
        OperationPrecondition::TrimScene {
            scene_id,
            in_frame,
            out_frame,
            ..
        } => {
            let target = find_target(project, scene_id)?;
            target.in_frame == *in_frame && target.out_frame == *out_frame
        }
        OperationPrecondition::UpdateScene {
            scene_id,
            title,
            purpose,
            caption,
            ..
        } => {
            let target = find_target(project, scene_id)?;
            target.title == *title && target.purpose == *purpose && target.caption == *caption
        }
        OperationPrecondition::DeleteScene {
            scene: expected, ..
        } => find_target(project, &expected.id)? == expected.as_ref(),
    };
    if !matches {
        return Err(AppError::Conflict(
            "operation target changed after proposal creation".into(),
        ));
    }
    Ok(())
}

pub(super) fn apply(project: &mut Project, operation: &Operation) -> AppResult<()> {
    validate_shape(operation)?;
    match operation.kind {
        OperationType::InsertScene => scene_proposal::insert(project, operation),
        OperationType::SetCamera => scene::set_camera(
            project,
            &operation.scene_id,
            operation.camera.clone(),
            false,
        ),
        OperationType::TrimScene => scene::trim(
            project,
            &operation.scene_id,
            required(&operation.in_frame, "in_frame")?,
            required(&operation.out_frame, "out_frame")?,
            false,
        ),
        OperationType::UpdateScene => scene_proposal::update_metadata(project, operation),
        OperationType::DeleteScene => scene::delete(project, &operation.scene_id),
    }
}

fn required<'a>(value: &'a Option<String>, field: &str) -> AppResult<&'a str> {
    value
        .as_deref()
        .ok_or_else(|| AppError::Invalid(format!("{field} required")))
}

fn find_target<'a>(project: &'a Project, scene_id: &str) -> AppResult<&'a crate::model::Scene> {
    project
        .scenes
        .iter()
        .find(|scene| scene.id == scene_id)
        .ok_or_else(|| AppError::Conflict("operation target no longer exists".into()))
}
