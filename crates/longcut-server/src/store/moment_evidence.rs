use std::collections::HashSet;

use futures_util::TryStreamExt;
use sqlx::{QueryBuilder, Row, Sqlite};

use crate::{
    error::{AppError, AppResult},
    model::{
        MAX_EVIDENCE_IDS, MAX_MOMENT_EVIDENCE_QUOTE_BYTES, MomentEvidence, MomentEvidenceOrigin,
        MomentEvidenceSelection, MomentTimingBasis,
    },
    transcript::{
        TranscriptOrigin, TranscriptPage, canonical_word_separator, checked_canonical_word_len,
    },
};

pub(super) struct EvidenceRequest<'a> {
    pub project_id: &'a str,
    pub source_id: &'a str,
    pub moment_start: &'a str,
    pub moment_end: &'a str,
    pub selection: &'a MomentEvidenceSelection,
}

struct SelectedItem {
    id: String,
    start: u64,
    end: u64,
    text: String,
}

pub(super) async fn resolve_evidence(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    request: EvidenceRequest<'_>,
) -> AppResult<MomentEvidence> {
    validate_selection(request.selection)?;
    let summary: String = sqlx::query_scalar(
        "SELECT summary FROM transcripts WHERE id=? AND project_id=? AND source_id=?",
    )
    .bind(&request.selection.transcript_id)
    .bind(request.project_id)
    .bind(request.source_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::Invalid("transcript evidence does not belong to this project and source".into())
    })?;
    let transcript: TranscriptPage = serde_json::from_str(&summary)?;
    let (items, basis) = if request.selection.word_ids.is_empty() {
        (
            selected_cues(
                tx,
                &request.selection.transcript_id,
                &request.selection.cue_ids,
            )
            .await?,
            MomentTimingBasis::Cues,
        )
    } else {
        (
            selected_words(
                tx,
                &request.selection.transcript_id,
                &request.selection.word_ids,
            )
            .await?,
            MomentTimingBasis::Words,
        )
    };
    let resolved = freeze(items, basis)?;
    if resolved.start < frame(request.moment_start)? || resolved.end > frame(request.moment_end)? {
        return Err(AppError::Invalid(
            "transcript evidence is outside the moment range".into(),
        ));
    }
    Ok(MomentEvidence {
        transcript_id: request.selection.transcript_id.clone(),
        cue_ids: resolved.cue_ids,
        word_ids: resolved.word_ids,
        quote: resolved.quote,
        origin: match transcript.origin {
            TranscriptOrigin::SubtitleImport => MomentEvidenceOrigin::SubtitleImport,
            TranscriptOrigin::LocalAsr => MomentEvidenceOrigin::LocalAsr,
        },
        start_frame: resolved.start.to_string(),
        end_frame: resolved.end.to_string(),
        timing_basis: basis,
    })
}

async fn selected_cues(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    transcript_id: &str,
    ids: &[String],
) -> AppResult<Vec<SelectedItem>> {
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT id,start_frame,end_frame,text FROM transcript_cues WHERE transcript_id=",
    );
    query.push_bind(transcript_id).push(" AND id IN (");
    push_ids(&mut query, ids);
    query.push(") ORDER BY CAST(start_frame AS INTEGER),ordinal");
    collect_selected(query, tx, ids.len(), MomentTimingBasis::Cues).await
}

async fn selected_words(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    transcript_id: &str,
    ids: &[String],
) -> AppResult<Vec<SelectedItem>> {
    let mut query = QueryBuilder::<Sqlite>::new(
        "SELECT word.word_id AS id,word.start_frame,word.end_frame,word.text FROM transcript_words word JOIN transcript_cues cue ON cue.id=word.cue_id WHERE word.transcript_id=",
    );
    query
        .push_bind(transcript_id)
        .push(" AND word.word_id IN (");
    push_ids(&mut query, ids);
    query.push(") ORDER BY CAST(cue.start_frame AS INTEGER),cue.ordinal,word.ordinal");
    collect_selected(query, tx, ids.len(), MomentTimingBasis::Words).await
}

fn push_ids<'a>(query: &mut QueryBuilder<'a, Sqlite>, ids: &'a [String]) {
    let mut separated = query.separated(",");
    for id in ids {
        separated.push_bind(id);
    }
}

async fn collect_selected(
    mut query: QueryBuilder<'_, Sqlite>,
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    expected: usize,
    basis: MomentTimingBasis,
) -> AppResult<Vec<SelectedItem>> {
    let mut rows = query.build().fetch(&mut **tx);
    let mut selected = Vec::with_capacity(expected);
    let mut quote_bytes = 0_usize;
    while let Some(row) = rows.try_next().await? {
        let text: String = row.try_get("text")?;
        quote_bytes = match basis {
            MomentTimingBasis::Cues => quote_bytes
                .checked_add(usize::from(!selected.is_empty()))
                .and_then(|bytes| bytes.checked_add(text.len())),
            MomentTimingBasis::Words => {
                checked_canonical_word_len(quote_bytes, !selected.is_empty(), &text)
            }
        }
        .ok_or_else(|| AppError::Invalid("evidence quote byte count overflow".into()))?;
        if quote_bytes > MAX_MOMENT_EVIDENCE_QUOTE_BYTES {
            return Err(AppError::Invalid("evidence quote exceeds 16 KiB".into()));
        }
        selected.push(SelectedItem {
            id: row.try_get("id")?,
            start: frame(&row.try_get::<String, _>("start_frame")?)?,
            end: frame(&row.try_get::<String, _>("end_frame")?)?,
            text,
        });
    }
    if selected.len() != expected {
        return Err(AppError::Invalid(
            "evidence item was not found uniquely in transcript".into(),
        ));
    }
    Ok(selected)
}

struct FrozenSelection {
    cue_ids: Vec<String>,
    word_ids: Vec<String>,
    quote: String,
    start: u64,
    end: u64,
}

fn freeze(items: Vec<SelectedItem>, basis: MomentTimingBasis) -> AppResult<FrozenSelection> {
    let start = items
        .iter()
        .map(|item| item.start)
        .min()
        .ok_or_else(|| AppError::Invalid("empty evidence".into()))?;
    let end = items
        .iter()
        .map(|item| item.end)
        .max()
        .ok_or_else(|| AppError::Invalid("empty evidence".into()))?;
    let mut quote_bytes = 0_usize;
    for (index, item) in items.iter().enumerate() {
        quote_bytes = match basis {
            MomentTimingBasis::Cues => quote_bytes
                .checked_add(usize::from(index > 0))
                .and_then(|bytes| bytes.checked_add(item.text.len())),
            MomentTimingBasis::Words => {
                checked_canonical_word_len(quote_bytes, index > 0, &item.text)
            }
        }
        .ok_or_else(|| AppError::Invalid("evidence quote byte count overflow".into()))?;
        if quote_bytes > MAX_MOMENT_EVIDENCE_QUOTE_BYTES {
            return Err(AppError::Invalid("evidence quote exceeds 16 KiB".into()));
        }
    }
    let mut quote = String::with_capacity(quote_bytes);
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            quote.push_str(match basis {
                MomentTimingBasis::Cues => "\n",
                MomentTimingBasis::Words => canonical_word_separator(&item.text),
            });
        }
        quote.push_str(&item.text);
    }
    let ids = items.into_iter().map(|item| item.id).collect();
    let (cue_ids, word_ids) = match basis {
        MomentTimingBasis::Cues => (ids, vec![]),
        MomentTimingBasis::Words => (vec![], ids),
    };
    Ok(FrozenSelection {
        cue_ids,
        word_ids,
        quote,
        start,
        end,
    })
}

fn validate_selection(selection: &MomentEvidenceSelection) -> AppResult<()> {
    let id_count = selection.cue_ids.len() + selection.word_ids.len();
    if id_count == 0 || id_count > MAX_EVIDENCE_IDS {
        return Err(AppError::Invalid(
            "evidence selection is empty or too large".into(),
        ));
    }
    if !selection.cue_ids.is_empty() && !selection.word_ids.is_empty() {
        return Err(AppError::Invalid(
            "select cue IDs or word IDs, not both".into(),
        ));
    }
    let ids = if selection.cue_ids.is_empty() {
        &selection.word_ids
    } else {
        &selection.cue_ids
    };
    let unique: HashSet<&str> = ids.iter().map(String::as_str).collect();
    if unique.len() != ids.len() {
        return Err(AppError::Invalid("evidence IDs must be unique".into()));
    }
    Ok(())
}

fn frame(value: &str) -> AppResult<u64> {
    value
        .parse()
        .map_err(|_| AppError::Invalid("evidence frame is invalid".into()))
}
