use crate::{
    error::{AppError, AppResult},
    model::{Beat, MomentReviewStatus, Project, Story, StoryCommand},
};

mod order;
use order::{
    beat_index, beat_mut, chapter_mut, delete_chapter, insert_beat, insert_chapter, move_beat,
    move_chapter,
};

pub(super) fn apply_outline(
    story: &mut Story,
    project: &Project,
    command: StoryCommand,
) -> AppResult<()> {
    match command {
        StoryCommand::CreateChapter {
            title,
            purpose,
            before_chapter_id,
            ..
        } => insert_chapter(story, title, purpose, before_chapter_id.as_deref()),
        StoryCommand::UpdateChapter {
            chapter_id,
            title,
            purpose,
            ..
        } => {
            let chapter = chapter_mut(story, &chapter_id)?;
            chapter.title = title;
            chapter.purpose = purpose;
            Ok(())
        }
        StoryCommand::MoveChapter {
            chapter_id,
            before_chapter_id,
            ..
        } => move_chapter(story, &chapter_id, before_chapter_id.as_deref()),
        StoryCommand::DeleteChapter { chapter_id, .. } => delete_chapter(story, &chapter_id),
        StoryCommand::CreateBeat {
            chapter_id,
            title,
            purpose,
            target_frames,
            before_beat_id,
            ..
        } => insert_beat(
            story,
            &chapter_id,
            Beat {
                id: uuid::Uuid::new_v4().to_string(),
                title,
                purpose,
                target_frames,
                moment_refs: vec![],
            },
            before_beat_id.as_deref(),
        ),
        StoryCommand::UpdateBeat {
            beat_id,
            title,
            purpose,
            target_frames,
            ..
        } => {
            let beat = beat_mut(story, &beat_id)?;
            beat.title = title;
            beat.purpose = purpose;
            beat.target_frames = target_frames;
            Ok(())
        }
        StoryCommand::MoveBeat {
            beat_id,
            chapter_id,
            before_beat_id,
            ..
        } => move_beat(story, &beat_id, &chapter_id, before_beat_id.as_deref()),
        StoryCommand::DeleteBeat { beat_id, .. } => delete_beat(story, project, &beat_id),
        StoryCommand::SetBeatMomentRefs {
            beat_id,
            moment_refs,
            ..
        } => set_refs(story, project, &beat_id, moment_refs),
        StoryCommand::SetSceneBeat { .. } => Err(AppError::Internal(anyhow::anyhow!(
            "scene assignment reached outline dispatcher"
        ))),
    }
}

pub(super) fn assign_scene(
    project: &mut Project,
    scene_id: &str,
    beat_id: Option<String>,
) -> AppResult<()> {
    if let Some(id) = &beat_id {
        super::find_beat(&project.story, id)?;
    }
    let scene = crate::domain::scene::scene_mut(project, scene_id)?;
    if scene.beat_id == beat_id {
        return Ok(());
    }
    if scene.locked {
        return Err(AppError::Locked(scene_id.into()));
    }
    scene.beat_id = beat_id;
    Ok(())
}

fn delete_beat(story: &mut Story, project: &Project, id: &str) -> AppResult<()> {
    if project
        .scenes
        .iter()
        .any(|scene| scene.beat_id.as_deref() == Some(id))
    {
        return Err(AppError::Conflict(
            "beat is assigned to an active scene".into(),
        ));
    }
    let (chapter, beat) = beat_index(story, id)?;
    story.chapters[chapter].beats.remove(beat);
    Ok(())
}

fn set_refs(
    story: &mut Story,
    project: &Project,
    id: &str,
    refs: Vec<crate::model::MomentVersionRef>,
) -> AppResult<()> {
    let unique: std::collections::HashSet<&str> = refs
        .iter()
        .map(|reference| reference.moment_id.as_str())
        .collect();
    if unique.len() != refs.len() {
        return Err(AppError::Invalid("duplicate moment_id in beat".into()));
    }
    let existing = super::find_beat(story, id)?.moment_refs.clone();
    for reference in refs
        .iter()
        .filter(|reference| !existing.contains(reference))
    {
        let moment = project
            .moments
            .iter()
            .find(|moment| moment.id == reference.moment_id && moment.version == reference.version)
            .ok_or_else(|| {
                AppError::Conflict("new moment reference must use current head".into())
            })?;
        if moment.archived || moment.review_status != MomentReviewStatus::Reviewed {
            return Err(AppError::Conflict(
                "new moment reference must be reviewed and active".into(),
            ));
        }
    }
    beat_mut(story, id)?.moment_refs = refs;
    Ok(())
}
