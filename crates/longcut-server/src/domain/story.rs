mod mutation;

use std::collections::HashSet;

use crate::{
    error::{AppError, AppResult},
    model::{
        Beat, MAX_BEATS_PER_CHAPTER, MAX_CHAPTERS, MAX_PURPOSE_BYTES, MAX_REFS_PER_BEAT,
        MAX_STORY_BYTES, MAX_TARGET_FRAMES, MAX_TITLE_BYTES, MAX_TOTAL_BEATS, MAX_TOTAL_REFS,
        Project, Story, StoryCommand,
    },
};

use super::ApplyContext;

pub(super) fn preflight(project: &Project, command: &StoryCommand) -> AppResult<()> {
    if command.expected_story_version() != &project.story.version {
        return Err(AppError::Conflict("story version changed".into()));
    }
    match command {
        StoryCommand::CreateChapter { title, purpose, .. }
        | StoryCommand::UpdateChapter { title, purpose, .. } => validate_text(title, purpose)?,
        StoryCommand::CreateBeat {
            title,
            purpose,
            target_frames,
            ..
        }
        | StoryCommand::UpdateBeat {
            title,
            purpose,
            target_frames,
            ..
        } => {
            validate_text(title, purpose)?;
            validate_target(target_frames.as_deref())?;
        }
        StoryCommand::SetBeatMomentRefs { moment_refs, .. } => {
            validate_refs(moment_refs)?;
            for reference in new_moment_refs(project, command)? {
                let moment = project
                    .moments
                    .iter()
                    .find(|moment| {
                        moment.id == reference.moment_id && moment.version == reference.version
                    })
                    .ok_or_else(|| {
                        AppError::Conflict("new moment reference must use current head".into())
                    })?;
                if moment.archived
                    || moment.review_status != crate::model::MomentReviewStatus::Reviewed
                {
                    return Err(AppError::Conflict(
                        "new moment reference must be reviewed and active".into(),
                    ));
                }
            }
        }
        StoryCommand::MoveChapter { .. }
        | StoryCommand::DeleteChapter { .. }
        | StoryCommand::MoveBeat { .. }
        | StoryCommand::DeleteBeat { .. }
        | StoryCommand::SetSceneBeat { .. } => {}
    }
    Ok(())
}

pub(super) fn apply(
    project: &mut Project,
    command: StoryCommand,
    context: &ApplyContext,
) -> AppResult<()> {
    preflight(project, &command)?;
    match command {
        StoryCommand::SetSceneBeat {
            scene_id, beat_id, ..
        } => mutation::assign_scene(project, &scene_id, beat_id),
        outline => {
            let mut story = project.story.clone();
            mutation::apply_outline(&mut story, project, outline)?;
            story.version.clone_from(&context.revision);
            validate(&story)?;
            project.story = story;
            Ok(())
        }
    }
}

pub(super) fn new_moment_refs<'a>(
    project: &'a Project,
    command: &'a StoryCommand,
) -> AppResult<Vec<&'a crate::model::MomentVersionRef>> {
    let StoryCommand::SetBeatMomentRefs {
        beat_id,
        moment_refs,
        ..
    } = command
    else {
        return Ok(vec![]);
    };
    let beat = find_beat(&project.story, beat_id)?;
    Ok(moment_refs
        .iter()
        .filter(|reference| !beat.moment_refs.contains(reference))
        .collect())
}

pub(super) fn find_beat<'a>(story: &'a Story, beat_id: &str) -> AppResult<&'a Beat> {
    story
        .chapters
        .iter()
        .flat_map(|chapter| &chapter.beats)
        .find(|beat| beat.id == beat_id)
        .ok_or(AppError::NotFound)
}

fn validate(story: &Story) -> AppResult<()> {
    if story.chapters.len() > MAX_CHAPTERS {
        return Err(AppError::Invalid("story exceeds chapter limit".into()));
    }
    let mut beat_count = 0usize;
    let mut ref_count = 0usize;
    let mut ids = HashSet::new();
    for chapter in &story.chapters {
        validate_text(&chapter.title, &chapter.purpose)?;
        if !ids.insert(chapter.id.as_str()) {
            return Err(AppError::Invalid("duplicate story id".into()));
        }
        if chapter.beats.len() > MAX_BEATS_PER_CHAPTER {
            return Err(AppError::Invalid("chapter exceeds beat limit".into()));
        }
        beat_count = beat_count
            .checked_add(chapter.beats.len())
            .ok_or_else(|| AppError::Invalid("story beat count overflow".into()))?;
        for beat in &chapter.beats {
            validate_beat(beat)?;
            if !ids.insert(beat.id.as_str()) {
                return Err(AppError::Invalid("duplicate story id".into()));
            }
            ref_count = ref_count
                .checked_add(beat.moment_refs.len())
                .ok_or_else(|| AppError::Invalid("story reference count overflow".into()))?;
        }
    }
    if beat_count > MAX_TOTAL_BEATS || ref_count > MAX_TOTAL_REFS {
        return Err(AppError::Invalid("story exceeds aggregate limits".into()));
    }
    if serde_json::to_vec(story)?.len() > MAX_STORY_BYTES {
        return Err(AppError::Invalid("serialized story exceeds 4 MiB".into()));
    }
    Ok(())
}

fn validate_beat(beat: &Beat) -> AppResult<()> {
    validate_text(&beat.title, &beat.purpose)?;
    validate_target(beat.target_frames.as_deref())?;
    validate_refs(&beat.moment_refs)
}

fn validate_target(target_frames: Option<&str>) -> AppResult<()> {
    if let Some(value) = target_frames {
        let frames = value
            .parse::<u32>()
            .map_err(|_| AppError::Invalid("target_frames must be a decimal string".into()))?;
        if frames == 0 || frames > MAX_TARGET_FRAMES {
            return Err(AppError::Invalid(
                "target_frames must be between 1 and 2147483647".into(),
            ));
        }
    }
    Ok(())
}

fn validate_refs(refs: &[crate::model::MomentVersionRef]) -> AppResult<()> {
    if refs.len() > MAX_REFS_PER_BEAT {
        return Err(AppError::Invalid(
            "beat exceeds moment reference limit".into(),
        ));
    }
    let unique: HashSet<&str> = refs
        .iter()
        .map(|reference| reference.moment_id.as_str())
        .collect();
    if unique.len() != refs.len() {
        return Err(AppError::Invalid("duplicate moment_id in beat".into()));
    }
    Ok(())
}

fn validate_text(title: &str, purpose: &str) -> AppResult<()> {
    if title.trim().is_empty() || title.len() > MAX_TITLE_BYTES {
        return Err(AppError::Invalid(
            "title must contain 1 to 200 UTF-8 bytes".into(),
        ));
    }
    if purpose.len() > MAX_PURPOSE_BYTES {
        return Err(AppError::Invalid("purpose exceeds 2000 UTF-8 bytes".into()));
    }
    Ok(())
}
