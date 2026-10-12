use crate::{
    error::{AppError, AppResult},
    model::{Beat, Chapter, Story},
};

pub(super) fn insert_chapter(
    story: &mut Story,
    title: String,
    purpose: String,
    before: Option<&str>,
) -> AppResult<()> {
    let target = chapter_target(story, before)?;
    story.chapters.insert(
        target,
        Chapter {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            purpose,
            beats: vec![],
        },
    );
    Ok(())
}

pub(super) fn move_chapter(story: &mut Story, id: &str, before: Option<&str>) -> AppResult<()> {
    if before == Some(id) {
        return Err(AppError::Invalid(
            "chapter cannot be moved before itself".into(),
        ));
    }
    let source = chapter_index(story, id)?;
    let chapter = story.chapters.remove(source);
    let target = chapter_target(story, before)?;
    story.chapters.insert(target, chapter);
    Ok(())
}

pub(super) fn delete_chapter(story: &mut Story, id: &str) -> AppResult<()> {
    let index = chapter_index(story, id)?;
    if !story.chapters[index].beats.is_empty() {
        return Err(AppError::Conflict(
            "chapter must be empty before deletion".into(),
        ));
    }
    story.chapters.remove(index);
    Ok(())
}

pub(super) fn insert_beat(
    story: &mut Story,
    chapter_id: &str,
    beat: Beat,
    before: Option<&str>,
) -> AppResult<()> {
    let chapter = chapter_mut(story, chapter_id)?;
    let target = beat_target(chapter, before)?;
    chapter.beats.insert(target, beat);
    Ok(())
}

pub(super) fn move_beat(
    story: &mut Story,
    id: &str,
    chapter_id: &str,
    before: Option<&str>,
) -> AppResult<()> {
    if before == Some(id) {
        return Err(AppError::Invalid(
            "beat cannot be moved before itself".into(),
        ));
    }
    chapter_index(story, chapter_id)?;
    if let Some(target) = before {
        let target_chapter = story
            .chapters
            .iter()
            .find(|chapter| chapter.beats.iter().any(|beat| beat.id == target))
            .ok_or(AppError::NotFound)?;
        if target_chapter.id != chapter_id {
            return Err(AppError::Invalid(
                "before beat is outside target chapter".into(),
            ));
        }
    }
    let (source_chapter, source_beat) = beat_index(story, id)?;
    let beat = story.chapters[source_chapter].beats.remove(source_beat);
    insert_beat(story, chapter_id, beat, before)
}

pub(super) fn chapter_mut<'a>(story: &'a mut Story, id: &str) -> AppResult<&'a mut Chapter> {
    let index = chapter_index(story, id)?;
    Ok(&mut story.chapters[index])
}

pub(super) fn beat_index(story: &Story, id: &str) -> AppResult<(usize, usize)> {
    story
        .chapters
        .iter()
        .enumerate()
        .find_map(|(chapter_index, chapter)| {
            chapter
                .beats
                .iter()
                .position(|beat| beat.id == id)
                .map(|beat_index| (chapter_index, beat_index))
        })
        .ok_or(AppError::NotFound)
}

pub(super) fn beat_mut<'a>(story: &'a mut Story, id: &str) -> AppResult<&'a mut Beat> {
    let (chapter, beat) = beat_index(story, id)?;
    Ok(&mut story.chapters[chapter].beats[beat])
}

fn chapter_index(story: &Story, id: &str) -> AppResult<usize> {
    story
        .chapters
        .iter()
        .position(|chapter| chapter.id == id)
        .ok_or(AppError::NotFound)
}

fn chapter_target(story: &Story, before: Option<&str>) -> AppResult<usize> {
    before.map_or(Ok(story.chapters.len()), |id| chapter_index(story, id))
}

fn beat_target(chapter: &Chapter, before: Option<&str>) -> AppResult<usize> {
    before.map_or(Ok(chapter.beats.len()), |id| {
        chapter
            .beats
            .iter()
            .position(|beat| beat.id == id)
            .ok_or(AppError::NotFound)
    })
}
