use std::{path::Path, sync::Arc};

use tokio::sync::OwnedSemaphorePermit;
use tokio_util::sync::CancellationToken;

use crate::{
    AppState,
    error::{AppError, AppResult},
    media,
    transcript::Cue,
};

use super::{
    AsrJob, AsrTranscriptMetadata, Service, SourceSnapshot, audio,
    conversion::{CueAccumulator, CueWindow, merge_language},
    whisper,
};

pub(super) struct WorkerInput {
    pub state: AppState,
    pub service: Arc<Service>,
    pub job: AsrJob,
    pub source: SourceSnapshot,
    pub cancellation: CancellationToken,
}

struct WorkContext<'a> {
    state: &'a AppState,
    service: &'a Arc<Service>,
    job: &'a AsrJob,
    source: &'a SourceSnapshot,
    cancellation: &'a CancellationToken,
}

struct ChunkContext<'a> {
    work: WorkContext<'a>,
    source_path: &'a Path,
    work_path: &'a Path,
    chunk_frames: u64,
    total_chunks: u32,
}

pub(super) async fn execute(input: WorkerInput) {
    let context = WorkContext {
        state: &input.state,
        service: &input.service,
        job: &input.job,
        source: &input.source,
        cancellation: &input.cancellation,
    };
    let result = run(&context).await;
    if let Err(error) = result {
        let message = match &error {
            AppError::Conflict(message) => message.clone(),
            AppError::Invalid(message) => message.clone(),
            AppError::NotFound => "source or ASR job no longer exists".into(),
            AppError::Stale => "ASR publication became stale".into(),
            AppError::Locked(message) => message.clone(),
            AppError::Internal(_) | AppError::Database(_) => "local ASR processing failed".into(),
        };
        if let Err(store_error) = input.state.store.fail_asr(&input.job.id, &message).await {
            tracing::error!(job_id = input.job.id, error = ?store_error, "asr.failure_persist");
        }
    }
    input.service.finish(&input.job.id).await;
}

async fn run(context: &WorkContext<'_>) -> AppResult<()> {
    let _permit = acquire(context.service, context.cancellation).await?;
    let chunk_frames = u64::from(context.service.config.chunk_seconds)
        .checked_mul(u64::from(context.source.frame_rate))
        .ok_or_else(|| AppError::Invalid("ASR chunk size overflow".into()))?;
    let total_chunks_u64 = context.source.duration_frames.div_ceil(chunk_frames);
    let total_chunks = u32::try_from(total_chunks_u64)
        .map_err(|_| AppError::Invalid("source has too many ASR chunks".into()))?;
    context
        .state
        .store
        .start_asr(&context.job.id, total_chunks)
        .await?;
    let work = context.service.work_root.join(&context.job.id);
    tokio::fs::create_dir(&work).await?;
    let source_path = context
        .state
        .artifacts
        .join(media::artifact_name(&context.source.media_url)?);
    let processed = process_chunks(ChunkContext {
        work: WorkContext {
            state: context.state,
            service: context.service,
            job: context.job,
            source: context.source,
            cancellation: context.cancellation,
        },
        source_path: &source_path,
        work_path: &work,
        chunk_frames,
        total_chunks,
    })
    .await;
    if let Err(error) = tokio::fs::remove_dir_all(&work).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(job_id = context.job.id, path = %work.display(), error = ?error, "asr.cleanup_failed");
        if processed.is_ok() {
            return Err(error.into());
        }
    }
    let (cues, result_language) = processed?;
    context
        .state
        .store
        .publish_asr(
            context.job,
            &cues,
            AsrTranscriptMetadata {
                engine: context.job.engine.clone(),
                model: context.job.model.clone(),
                requested_language: context.job.language,
                result_language,
            },
        )
        .await?;
    Ok(())
}

async fn process_chunks(context: ChunkContext<'_>) -> AppResult<(Vec<Cue>, Option<String>)> {
    let mut cues = CueAccumulator::new();
    let mut observed_language = None;
    for index in 0..context.total_chunks {
        let start_frame = u64::from(index) * context.chunk_frames;
        let end_frame = context
            .work
            .source
            .duration_frames
            .min(start_frame.saturating_add(context.chunk_frames));
        let wav = context.work_path.join(format!("chunk-{index}.wav"));
        let prefix = context.work_path.join(format!("chunk-{index}"));
        audio::extract_chunk(
            &context.work.service.config,
            audio::AudioChunk {
                source: context.source_path,
                output: &wav,
                start_frame,
                end_frame,
                frame_rate: context.work.source.frame_rate,
            },
            context.work.cancellation,
        )
        .await?;
        let output = whisper::transcribe(
            &context.work.service.config,
            whisper::WhisperRequest {
                wav: &wav,
                prefix: &prefix,
                language: context.work.job.language,
            },
            context.work.cancellation,
        )
        .await?;
        observed_language = merge_language(observed_language, output.result.language.clone());
        cues.append(
            output,
            CueWindow {
                start: start_frame,
                end: end_frame,
                frame_rate: context.work.source.frame_rate,
            },
        )?;
        cleanup_chunk(&wav, &prefix).await?;
        context
            .work
            .state
            .store
            .progress_asr(&context.work.job.id, index + 1)
            .await?;
    }
    Ok((cues.into_cues(), observed_language))
}

async fn cleanup_chunk(wav: &Path, prefix: &Path) -> AppResult<()> {
    for path in [wav.to_path_buf(), prefix.with_extension("json")] {
        match tokio::fs::remove_file(path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

async fn acquire(
    service: &Arc<Service>,
    cancellation: &CancellationToken,
) -> AppResult<OwnedSemaphorePermit> {
    tokio::select! {
        biased;
        () = cancellation.cancelled() => Err(AppError::Conflict("ASR job cancelled".into())),
        permit = service.slots.clone().acquire_owned() => permit
            .map_err(|error| AppError::Internal(anyhow::anyhow!("ASR limiter closed: {error}"))),
    }
}
