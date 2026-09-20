use crate::{
    error::{AppError, AppResult},
    model::{Operation, Project, Scene},
};

use super::scene;

pub(super) fn insert(project: &mut Project, operation: &Operation) -> AppResult<()> {
    let source_id = required(&operation.source_id, "source_id")?;
    let in_frame = required(&operation.in_frame, "in_frame")?;
    let out_frame = required(&operation.out_frame, "out_frame")?;
    let title = required(&operation.title, "title")?;
    let scene_id = operation.scene_id.as_str();
    if project.scenes.iter().any(|item| item.id == scene_id) {
        return Err(AppError::Conflict(
            "proposed scene id already exists".into(),
        ));
    }
    scene::validate_range(project, source_id, in_frame, out_frame)?;
    project.scenes.push(Scene {
        id: scene_id.into(),
        title: title.into(),
        purpose: String::new(),
        source_id: source_id.into(),
        in_frame: in_frame.into(),
        out_frame: out_frame.into(),
        camera: None,
        caption: String::new(),
        locked: false,
        manual: false,
    });
    Ok(())
}

pub(super) fn update_metadata(project: &mut Project, operation: &Operation) -> AppResult<()> {
    let target = scene::editable(project, &operation.scene_id)?;
    target.title = required(&operation.title, "title")?.into();
    target.purpose = required(&operation.purpose, "purpose")?.into();
    target.caption = required(&operation.caption, "caption")?.into();
    Ok(())
}

fn required<'a>(value: &'a Option<String>, field: &str) -> AppResult<&'a str> {
    value
        .as_deref()
        .ok_or_else(|| AppError::Invalid(format!("{field} required")))
}
