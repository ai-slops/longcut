use std::collections::HashSet;

use super::{AiOperation, AiProposal, AiScope, EvidenceSample, context::scoped_sources};
use crate::{
    error::{AppError, AppResult},
    model::{Project, Scene, Source},
};

pub fn validate_proposal(
    project: &Project,
    scope: &AiScope,
    samples: &[EvidenceSample],
    proposal: &AiProposal,
) -> AppResult<()> {
    if proposal.operations.len() > 24 {
        return Err(AppError::Invalid(
            "AI proposal exceeds 24 operations".into(),
        ));
    }
    let evidence = evidence_ids(project, scope, samples)?;
    let mut properties = HashSet::with_capacity(proposal.operations.len());
    let mut deleted_scenes = HashSet::new();
    for operation in &proposal.operations {
        if operation.evidence_ids().is_empty() {
            return Err(AppError::Invalid(
                "AI operations require at least one evidence ID".into(),
            ));
        }
        if operation
            .evidence_ids()
            .iter()
            .any(|id| !evidence.contains(id))
        {
            return Err(AppError::Invalid(
                "AI operation referenced evidence outside frozen context".into(),
            ));
        }
        validate_operation(
            project,
            scope,
            operation,
            &mut properties,
            &mut deleted_scenes,
        )?;
    }
    Ok(())
}

fn validate_operation(
    project: &Project,
    scope: &AiScope,
    operation: &AiOperation,
    properties: &mut HashSet<String>,
    deleted_scenes: &mut HashSet<String>,
) -> AppResult<()> {
    match (scope, operation) {
        (
            AiScope::Source {
                source_id,
                start_frame,
                end_frame,
            },
            AiOperation::InsertScene {
                source_id: operation_source,
                in_frame,
                out_frame,
                ..
            },
        ) if operation_source == source_id => {
            let selected_start = frame(start_frame)?;
            let selected_end = frame(end_frame)?;
            let start = frame(in_frame)?;
            let end = frame(out_frame)?;
            if start < selected_start || start >= end || end > selected_end {
                return Err(AppError::Invalid(
                    "insert is outside selected source scope".into(),
                ));
            }
        }
        (AiScope::Source { .. }, _) => {
            return Err(AppError::Invalid(
                "source scope only permits insert_scene operations".into(),
            ));
        }
        (AiScope::Scenes { .. }, AiOperation::InsertScene { .. }) => {
            return Err(AppError::Invalid("scene scope cannot insert scenes".into()));
        }
        (AiScope::Scenes { scene_ids }, operation) => {
            let id = existing_scene_id(operation).ok_or_else(|| {
                AppError::Invalid("scene operation requires an existing scene".into())
            })?;
            if !scene_ids.iter().any(|selected| selected == id) {
                return Err(AppError::Invalid(
                    "operation is outside selected scenes".into(),
                ));
            }
            let target = scene(project, id)?;
            if target.locked {
                return Err(AppError::Locked(id.into()));
            }
            if deleted_scenes.contains(id) {
                return Err(AppError::Invalid(
                    "deleted scene has additional operations".into(),
                ));
            }
            let property = operation_property(operation);
            if !properties.insert(format!("{id}:{property}")) {
                return Err(AppError::Invalid(
                    "multiple operations target the same scene property".into(),
                ));
            }
            match operation {
                AiOperation::TrimScene {
                    in_frame,
                    out_frame,
                    ..
                } => validate_range(project, &target.source_id, in_frame, out_frame)?,
                AiOperation::SetCamera {
                    camera: Some(value),
                    ..
                } => {
                    if !scoped_sources(project, scope)
                        .iter()
                        .any(|source| source.id == value.source_id)
                    {
                        return Err(AppError::Invalid(
                            "camera source is outside frozen context".into(),
                        ));
                    }
                    let camera_source = source(project, &value.source_id)?;
                    visual(camera_source)?;
                }
                AiOperation::DeleteScene { .. } => {
                    if properties.iter().any(|key| {
                        key.starts_with(&format!("{id}:")) && key != &format!("{id}:delete")
                    }) {
                        return Err(AppError::Invalid(
                            "deleted scene has additional operations".into(),
                        ));
                    }
                    deleted_scenes.insert(id.into());
                }
                AiOperation::SetCamera { camera: None, .. } | AiOperation::UpdateScene { .. } => {}
                AiOperation::InsertScene { .. } => {
                    return Err(AppError::Invalid("scene scope cannot insert scenes".into()));
                }
            }
        }
    }
    Ok(())
}

fn evidence_ids(
    project: &Project,
    scope: &AiScope,
    samples: &[EvidenceSample],
) -> AppResult<HashSet<String>> {
    let mut evidence = samples
        .iter()
        .map(|sample| sample.evidence_id.clone())
        .collect::<HashSet<_>>();
    match scope {
        AiScope::Source { source_id, .. } => {
            source(project, source_id)?;
            evidence.insert(format!("source_{source_id}"));
        }
        AiScope::Scenes { scene_ids } => {
            for id in scene_ids {
                scene(project, id)?;
                evidence.insert(format!("scene_{id}"));
            }
            for source in scoped_sources(project, scope) {
                evidence.insert(format!("source_{}", source.id));
            }
        }
    }
    Ok(evidence)
}

fn existing_scene_id(operation: &AiOperation) -> Option<&str> {
    match operation {
        AiOperation::InsertScene { .. } => None,
        AiOperation::TrimScene { scene_id, .. }
        | AiOperation::SetCamera { scene_id, .. }
        | AiOperation::UpdateScene { scene_id, .. }
        | AiOperation::DeleteScene { scene_id, .. } => Some(scene_id),
    }
}

const fn operation_property(operation: &AiOperation) -> &'static str {
    match operation {
        AiOperation::InsertScene { .. } => "insert",
        AiOperation::TrimScene { .. } => "trim",
        AiOperation::SetCamera { .. } => "camera",
        AiOperation::UpdateScene { .. } => "metadata",
        AiOperation::DeleteScene { .. } => "delete",
    }
}

fn validate_range(project: &Project, source_id: &str, start: &str, end: &str) -> AppResult<()> {
    let start = frame(start)?;
    let end = frame(end)?;
    let duration = frame(&source(project, source_id)?.duration_frames)?;
    if start >= end || end > duration {
        return Err(AppError::Invalid(
            "operation range is outside source".into(),
        ));
    }
    Ok(())
}

fn visual(source: &Source) -> AppResult<()> {
    if source.width == 0 || source.height == 0 {
        return Err(AppError::Invalid("AI scope requires visual media".into()));
    }
    Ok(())
}

fn source<'a>(project: &'a Project, id: &str) -> AppResult<&'a Source> {
    project
        .sources
        .iter()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)
}

fn scene<'a>(project: &'a Project, id: &str) -> AppResult<&'a Scene> {
    project
        .scenes
        .iter()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)
}

fn frame(value: &str) -> AppResult<u64> {
    value
        .parse()
        .map_err(|_| AppError::Invalid("frame must be a decimal string".into()))
}
