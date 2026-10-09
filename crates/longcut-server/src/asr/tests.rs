use crate::{
    model::{CreateProject, MediaStatus, Source, SourceRole},
    store::Store,
    transcript::{Cue, ImportTranscript, Search, Word},
};

use super::{AsrLanguage, AsrTranscriptMetadata, ResumeSnapshot};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn resume_snapshot() -> ResumeSnapshot {
    ResumeSnapshot {
        source_sha256: "source-hash".into(),
        model_sha256: "model-hash".into(),
        cli_sha256: "cli-hash".into(),
        source_media_url: "/api/v1/media/source.mp4".into(),
        model_path: "model.gguf".into(),
        cli_path: "whisper-cli".into(),
        duration_frames: 300,
        frame_rate: 30,
        language: AsrLanguage::Ko,
        chunk_seconds: 600,
        overlap_seconds: 2,
        conversion_policy_version: 1,
        argument_policy_version: 1,
    }
}

#[test]
fn legacy_asr_metadata_deserializes_with_honest_defaults() -> TestResult {
    // Given
    let json = serde_json::json!({
        "engine": "whisper.cpp",
        "model": "legacy.bin",
        "requested_language": "ko",
        "result_language": "ko"
    });
    // When
    let metadata: AsrTranscriptMetadata = serde_json::from_value(json)?;
    // Then
    assert_eq!(metadata.chunk_seconds, 0);
    assert_eq!(metadata.overlap_seconds, 0);
    assert_eq!(
        metadata.ownership_policy,
        super::AsrOwnershipPolicy::LegacyChunkBounds
    );
    assert_eq!(
        metadata.timing_quality,
        super::AsrTimingQuality::ModelEstimated
    );
    Ok(())
}

async fn fixture(path: &std::path::Path) -> Result<(Store, String), Box<dyn std::error::Error>> {
    let store = Store::open(path).await?;
    let project = store
        .create(CreateProject {
            title: "ASR".into(),
            frame_rate: 30,
        })
        .await?;
    store
        .add_source(
            &project.id,
            Source {
                id: "source".into(),
                name: "source".into(),
                role: SourceRole::Program,
                duration_frames: "300".into(),
                width: 1920,
                height: 1080,
                has_audio: true,
                media_url: "/api/v1/media/source.mp4".into(),
                proxy_url: None,
                thumbnail_url: None,
                waveform: Vec::new(),
                status: MediaStatus::Ready,
                error: None,
            },
        )
        .await?;
    Ok((store, project.id))
}

#[tokio::test]
async fn one_active_job_is_allowed_per_source() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let (store, project_id) = fixture(&root.path().join("store.sqlite3")).await?;
    store
        .enqueue_asr(
            &project_id,
            "source",
            AsrLanguage::Ko,
            "model.gguf",
            &resume_snapshot(),
        )
        .await?;
    // When
    let duplicate = store
        .enqueue_asr(
            &project_id,
            "source",
            AsrLanguage::Ko,
            "model.gguf",
            &resume_snapshot(),
        )
        .await;
    // Then
    assert!(duplicate.is_err());
    Ok(())
}

#[tokio::test]
async fn interrupted_job_is_failed_when_store_reopens() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let path = root.path().join("store.sqlite3");
    let (store, project_id) = fixture(&path).await?;
    let (job, _) = store
        .enqueue_asr(
            &project_id,
            "source",
            AsrLanguage::Ko,
            "model.gguf",
            &resume_snapshot(),
        )
        .await?;
    drop(store);
    // When
    let reopened = Store::open(&path).await?;
    let recovered = reopened.get_asr(&project_id, "source", &job.id).await?;
    // Then
    assert_eq!(recovered.status, super::AsrJobStatus::Failed);
    assert!(
        recovered
            .error
            .as_deref()
            .is_some_and(|error| error.contains("restart"))
    );
    Ok(())
}

#[tokio::test]
async fn publication_rejects_a_newer_manual_transcript_without_partial_write() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let (store, project_id) = fixture(&root.path().join("store.sqlite3")).await?;
    let (job, _) = store
        .enqueue_asr(
            &project_id,
            "source",
            AsrLanguage::Ko,
            "model.gguf",
            &resume_snapshot(),
        )
        .await?;
    let job = store.start_asr(&job.id, 1).await?;
    let manual = store
        .import_transcript(
            &project_id,
            "source",
            ImportTranscript {
                filename: "manual.srt".into(),
                text: "1\n00:00:00,000 --> 00:00:01,000\nmanual".into(),
            },
        )
        .await?;
    // When
    let published = store
        .publish_asr(
            &job,
            &[Cue {
                id: uuid::Uuid::new_v4().to_string(),
                start_frame: "0".into(),
                end_frame: "30".into(),
                text: "asr".into(),
                words: None,
            }],
            AsrTranscriptMetadata {
                engine: "whisper.cpp".into(),
                model: "model.gguf".into(),
                requested_language: AsrLanguage::Ko,
                result_language: Some("ko".into()),
                chunk_seconds: 600,
                overlap_seconds: 2,
                ownership_policy: super::AsrOwnershipPolicy::HalfOpenMidpoint,
                timing_quality: super::AsrTimingQuality::ModelEstimated,
            },
        )
        .await;
    // Then
    assert!(published.is_err());
    let current = store
        .transcript(&project_id, "source", Search::default())
        .await?
        .ok_or("manual transcript")?;
    assert_eq!(current.id, manual.id);
    assert_eq!(current.cue_count, 1);
    Ok(())
}

#[tokio::test]
async fn asr_words_persist_through_query_and_store_reopen() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let path = root.path().join("store.sqlite3");
    let (store, project_id) = fixture(&path).await?;
    let (job, _) = store
        .enqueue_asr(
            &project_id,
            "source",
            AsrLanguage::Ko,
            "model.gguf",
            &resume_snapshot(),
        )
        .await?;
    let job = store.start_asr(&job.id, 1).await?;
    let word_id = uuid::Uuid::new_v4().to_string();
    store
        .publish_asr(
            &job,
            &[Cue {
                id: uuid::Uuid::new_v4().to_string(),
                start_frame: "3".into(),
                end_frame: "18".into(),
                text: "저장 단어".into(),
                words: Some(vec![Word {
                    id: word_id.clone(),
                    start_frame: "3".into(),
                    end_frame: "18".into(),
                    text: "저장".into(),
                }]),
            }],
            AsrTranscriptMetadata {
                engine: "whisper.cpp".into(),
                model: "model.gguf".into(),
                requested_language: AsrLanguage::Ko,
                result_language: Some("ko".into()),
                chunk_seconds: 600,
                overlap_seconds: 2,
                ownership_policy: super::AsrOwnershipPolicy::HalfOpenMidpoint,
                timing_quality: super::AsrTimingQuality::ModelEstimated,
            },
        )
        .await?;
    drop(store);
    // When
    let reopened = Store::open(&path).await?;
    let transcript = reopened
        .transcript(&project_id, "source", Search::default())
        .await?
        .ok_or("ASR transcript")?;
    // Then
    let words = transcript.cues[0].words.as_ref().ok_or("ASR words")?;
    assert_eq!(words[0].id, word_id);
    assert_eq!(
        (words[0].start_frame.as_str(), words[0].end_frame.as_str()),
        ("3", "18")
    );
    Ok(())
}
