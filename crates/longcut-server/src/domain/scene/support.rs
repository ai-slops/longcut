use crate::{
    error::{AppError, AppResult},
    model::{Camera, Project, Scene, Source},
};

pub(in crate::domain) fn validate_range(
    project: &Project,
    source_id: &str,
    start: &str,
    end: &str,
) -> AppResult<()> {
    let start = frame(start)?;
    let end = frame(end)?;
    let source = source(project, source_id)?;
    if source.width == 0 || source.height == 0 {
        return Err(AppError::Invalid("visual source must contain video".into()));
    }
    let duration = frame(&source.duration_frames)?;
    if start >= end || end > duration {
        return Err(AppError::Invalid(
            "range must be non-empty and inside source".into(),
        ));
    }
    Ok(())
}

pub(in crate::domain) fn frame(value: &str) -> AppResult<u64> {
    value
        .parse()
        .map_err(|_| AppError::Invalid("frame must be a decimal string".into()))
}

pub(in crate::domain) fn scene_len(value: &Scene) -> AppResult<u64> {
    frame(&value.out_frame)?
        .checked_sub(frame(&value.in_frame)?)
        .ok_or_else(|| AppError::Invalid("invalid persisted scene range".into()))
}

pub(in crate::domain) fn validate_camera(
    project: &Project,
    camera: &Camera,
    scene_length: u64,
) -> AppResult<()> {
    let start = frame(&camera.in_frame)?;
    let end = start
        .checked_add(scene_length)
        .ok_or_else(|| AppError::Invalid("camera frame overflow".into()))?;
    validate_range(
        project,
        &camera.source_id,
        &camera.in_frame,
        &end.to_string(),
    )
}

pub(in crate::domain) fn source<'a>(project: &'a Project, id: &str) -> AppResult<&'a Source> {
    project
        .sources
        .iter()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)
}

pub(in crate::domain) fn source_mut<'a>(
    project: &'a mut Project,
    id: &str,
) -> AppResult<&'a mut Source> {
    project
        .sources
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)
}

pub(in crate::domain) fn get<'a>(project: &'a Project, id: &str) -> AppResult<&'a Scene> {
    project
        .scenes
        .iter()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)
}

pub(in crate::domain) fn scene_mut<'a>(
    project: &'a mut Project,
    id: &str,
) -> AppResult<&'a mut Scene> {
    project
        .scenes
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)
}

pub(in crate::domain) fn editable<'a>(
    project: &'a mut Project,
    id: &str,
) -> AppResult<&'a mut Scene> {
    let target = scene_mut(project, id)?;
    if target.locked {
        return Err(AppError::Locked(id.into()));
    }
    Ok(target)
}
