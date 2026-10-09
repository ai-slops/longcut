use crate::{
    error::{AppError, AppResult},
    model::{MAX_MOMENT_LABEL_BYTES, MAX_MOMENT_NOTE_BYTES, MomentEvidence, Project},
};

use super::moment::MomentFields;

pub(super) fn validate_fields(project: &Project, fields: &MomentFields) -> AppResult<()> {
    let source = super::scene::source(project, &fields.source_id)?;
    let start = super::scene::frame(&fields.start_frame)?;
    let end = super::scene::frame(&fields.end_frame)?;
    let duration = super::scene::frame(&source.duration_frames)?;
    if start >= end || end > duration {
        return Err(AppError::Invalid(
            "moment range must be non-empty and inside source".into(),
        ));
    }
    if fields.label.trim().is_empty() || fields.label.len() > MAX_MOMENT_LABEL_BYTES {
        return Err(AppError::Invalid(
            "a bounded moment label is required".into(),
        ));
    }
    if fields.note.len() > MAX_MOMENT_NOTE_BYTES {
        return Err(AppError::Invalid("moment note is too long".into()));
    }
    Ok(())
}

pub(super) fn validate_evidence(
    evidence: &[MomentEvidence],
    start: &str,
    end: &str,
) -> AppResult<()> {
    let start = super::scene::frame(start)?;
    let end = super::scene::frame(end)?;
    for item in evidence {
        let evidence_start = super::scene::frame(&item.start_frame)?;
        let evidence_end = super::scene::frame(&item.end_frame)?;
        if evidence_start < start || evidence_end > end || evidence_start >= evidence_end {
            return Err(AppError::Invalid(
                "transcript evidence must be inside the moment range".into(),
            ));
        }
    }
    Ok(())
}
