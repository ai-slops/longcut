use std::collections::HashSet;

use super::{AiRequest, AiScope};
use crate::{
    error::{AppError, AppResult},
    model::{Project, Source},
};

pub fn validate_request(project: &Project, request: &AiRequest) -> AppResult<()> {
    if !request.consent {
        return Err(AppError::Invalid(
            "AI context sharing consent is required".into(),
        ));
    }
    let instruction_length = request.instruction.chars().count();
    if !(1..=8000).contains(&instruction_length) {
        return Err(AppError::Invalid(
            "instruction must contain 1 to 8000 characters".into(),
        ));
    }
    if project.revision != request.base_revision {
        return Err(AppError::Stale);
    }
    match &request.scope {
        AiScope::Source {
            source_id,
            start_frame,
            end_frame,
        } => validate_source_scope(project, source_id, start_frame, end_frame),
        AiScope::Scenes { scene_ids } => validate_scene_scope(project, scene_ids),
    }
}

fn validate_source_scope(
    project: &Project,
    source_id: &str,
    start_frame: &str,
    end_frame: &str,
) -> AppResult<()> {
    let source = source(project, source_id)?;
    let start = frame(start_frame)?;
    let end = frame(end_frame)?;
    let duration = frame(&source.duration_frames)?;
    let maximum = u64::from(project.frame_rate) * 6 * 60 * 60;
    if start >= end || end > duration || end - start > maximum {
        return Err(AppError::Invalid(
            "source scope must be non-empty, in bounds, and at most six hours".into(),
        ));
    }
    visual(source)
}

fn validate_scene_scope(project: &Project, scene_ids: &[String]) -> AppResult<()> {
    if scene_ids.is_empty() || scene_ids.len() > 12 {
        return Err(AppError::Invalid(
            "scene scope requires 1 to 12 scenes".into(),
        ));
    }
    let mut unique = HashSet::with_capacity(scene_ids.len());
    for id in scene_ids {
        if !unique.insert(id) {
            return Err(AppError::Invalid("scene scope contains duplicates".into()));
        }
        let scene = project
            .scenes
            .iter()
            .find(|scene| &scene.id == id)
            .ok_or(AppError::NotFound)?;
        visual(source(project, &scene.source_id)?)?;
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
        .find(|source| source.id == id)
        .ok_or(AppError::NotFound)
}

fn frame(value: &str) -> AppResult<u64> {
    value
        .parse()
        .map_err(|_| AppError::Invalid("frame must be a decimal string".into()))
}
