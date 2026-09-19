use crate::{
    error::{AppError, AppResult},
    model::*,
};

pub(super) fn insert(
    project: &mut Project,
    source_id: String,
    in_frame: String,
    out_frame: String,
    title: String,
) -> AppResult<()> {
    validate_range(project, &source_id, &in_frame, &out_frame)?;
    project.scenes.push(Scene {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        purpose: String::new(),
        source_id,
        in_frame,
        out_frame,
        camera: None,
        caption: String::new(),
        locked: false,
        manual: true,
    });
    Ok(())
}

pub(super) fn trim(
    project: &mut Project,
    scene_id: &str,
    in_frame: &str,
    out_frame: &str,
    manual: bool,
) -> AppResult<()> {
    let scene = get(project, scene_id)?;
    let source_id = scene.source_id.clone();
    let camera = scene.camera.clone();
    validate_range(project, &source_id, in_frame, out_frame)?;
    let length = frame(out_frame)?
        .checked_sub(frame(in_frame)?)
        .ok_or_else(|| AppError::Invalid("scene frame range overflow".into()))?;
    if let Some(value) = &camera {
        validate_camera(project, value, length)?;
    }
    let target = editable(project, scene_id)?;
    target.in_frame = in_frame.into();
    target.out_frame = out_frame.into();
    target.manual |= manual;
    Ok(())
}

pub(super) fn split(project: &mut Project, scene_id: &str, offset: &str) -> AppResult<()> {
    let index = project
        .scenes
        .iter()
        .position(|item| item.id == scene_id)
        .ok_or(AppError::NotFound)?;
    if project.scenes[index].locked {
        return Err(AppError::Locked(scene_id.into()));
    }
    let start = frame(&project.scenes[index].in_frame)?;
    let end = frame(&project.scenes[index].out_frame)?;
    let offset = frame(offset)?;
    let split = start
        .checked_add(offset)
        .ok_or_else(|| AppError::Invalid("split frame overflow".into()))?;
    if offset == 0 || split >= end {
        return Err(AppError::Invalid(
            "split offset must be inside scene".into(),
        ));
    }
    let mut second = project.scenes[index].clone();
    project.scenes[index].out_frame = split.to_string();
    project.scenes[index].manual = true;
    second.id = uuid::Uuid::new_v4().to_string();
    second.in_frame = split.to_string();
    if let Some(camera) = &mut second.camera {
        camera.in_frame = frame(&camera.in_frame)?
            .checked_add(offset)
            .ok_or_else(|| AppError::Invalid("camera frame overflow".into()))?
            .to_string();
    }
    second.manual = true;
    let second_id = second.id.clone();
    project.scenes.insert(index + 1, second);
    let mut continued = Vec::new();
    for feedback in project
        .feedback
        .iter_mut()
        .filter(|item| item.scene_id == scene_id && item.render_id.is_none())
    {
        let feedback_start = frame(&feedback.start_frame)?;
        let feedback_end = frame(&feedback.end_frame)?;
        if feedback_start >= offset {
            feedback.scene_id = second_id.clone();
            feedback.start_frame = (feedback_start - offset).to_string();
            feedback.end_frame = (feedback_end - offset).to_string();
        } else if feedback_end > offset {
            let mut continuation = feedback.clone();
            continuation.id = uuid::Uuid::new_v4().to_string();
            continuation.scene_id = second_id.clone();
            continuation.start_frame = "0".into();
            continuation.end_frame = (feedback_end - offset).to_string();
            feedback.end_frame = offset.to_string();
            continued.push(continuation);
        }
    }
    project.feedback.extend(continued);
    Ok(())
}

pub(super) fn move_to(
    project: &mut Project,
    scene_id: &str,
    before: Option<&str>,
) -> AppResult<()> {
    let index = project
        .scenes
        .iter()
        .position(|item| item.id == scene_id)
        .ok_or(AppError::NotFound)?;
    if project.scenes[index].locked {
        return Err(AppError::Locked(scene_id.into()));
    }
    if before == Some(scene_id) {
        return Err(AppError::Invalid(
            "scene cannot be moved before itself".into(),
        ));
    }
    let mut moved = project.scenes.remove(index);
    moved.manual = true;
    let target = match before {
        Some(id) => project
            .scenes
            .iter()
            .position(|item| item.id == id)
            .ok_or(AppError::NotFound)?,
        None => project.scenes.len(),
    };
    project.scenes.insert(target, moved);
    Ok(())
}

pub(super) fn delete(project: &mut Project, scene_id: &str) -> AppResult<()> {
    let index = project
        .scenes
        .iter()
        .position(|item| item.id == scene_id)
        .ok_or(AppError::NotFound)?;
    if project.scenes[index].locked {
        return Err(AppError::Locked(scene_id.into()));
    }
    project.scenes.remove(index);
    Ok(())
}

pub(super) fn set_camera(
    project: &mut Project,
    scene_id: &str,
    camera: Option<Camera>,
    manual: bool,
) -> AppResult<()> {
    if let Some(value) = &camera {
        validate_camera(project, value, scene_len(get(project, scene_id)?)?)?;
        let finite = [value.x, value.y, value.width, value.height, value.opacity]
            .iter()
            .all(|component| component.is_finite());
        if !finite
            || value.x < 0.0
            || value.y < 0.0
            || value.width <= 0.0
            || value.height <= 0.0
            || value.x + value.width > 1.0
            || value.y + value.height > 1.0
            || !(0.0..=1.0).contains(&value.opacity)
        {
            return Err(AppError::Invalid("invalid camera geometry".into()));
        }
    }
    let target = editable(project, scene_id)?;
    target.camera = camera;
    target.manual |= manual;
    Ok(())
}

pub(super) fn validate_range(
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

pub(super) fn frame(value: &str) -> AppResult<u64> {
    value
        .parse()
        .map_err(|_| AppError::Invalid("frame must be a decimal string".into()))
}
pub(super) fn scene_len(value: &Scene) -> AppResult<u64> {
    frame(&value.out_frame)?
        .checked_sub(frame(&value.in_frame)?)
        .ok_or_else(|| AppError::Invalid("invalid persisted scene range".into()))
}

fn validate_camera(project: &Project, camera: &Camera, scene_length: u64) -> AppResult<()> {
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
pub(super) fn source<'a>(project: &'a Project, id: &str) -> AppResult<&'a Source> {
    project
        .sources
        .iter()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)
}
pub(super) fn source_mut<'a>(project: &'a mut Project, id: &str) -> AppResult<&'a mut Source> {
    project
        .sources
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)
}
pub(super) fn get<'a>(project: &'a Project, id: &str) -> AppResult<&'a Scene> {
    project
        .scenes
        .iter()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)
}
pub(super) fn scene_mut<'a>(project: &'a mut Project, id: &str) -> AppResult<&'a mut Scene> {
    project
        .scenes
        .iter_mut()
        .find(|item| item.id == id)
        .ok_or(AppError::NotFound)
}
pub(super) fn editable<'a>(project: &'a mut Project, id: &str) -> AppResult<&'a mut Scene> {
    let target = scene_mut(project, id)?;
    if target.locked {
        return Err(AppError::Locked(id.into()));
    }
    Ok(target)
}
