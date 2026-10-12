use super::{
    AsrJobStatus, AsrLanguage, AsrOwnershipPolicy, AsrTimingQuality, AsrTranscriptMetadata,
    checkpoint::Checkpoint,
    resume_test_support::{TestResult, fixture, fixture_at, snapshot},
};

#[tokio::test]
async fn successful_publication_deletes_job_checkpoints_atomically() -> TestResult {
    // Given
    let (store, project_id) = fixture().await?;
    let (job, _) = store
        .enqueue_asr(
            &project_id,
            "source",
            AsrLanguage::Ko,
            "model.gguf",
            &snapshot(),
        )
        .await?;
    let job = store.start_asr(&job.id, 3).await?;
    store
        .checkpoint_asr(
            &job.id,
            &Checkpoint {
                index: 0,
                inference_start: 0,
                inference_end: 360,
                owner_start: 0,
                owner_end: 300,
                payload: "{\"transcription\":[],\"result\":{\"language\":null}}".into(),
            },
        )
        .await?;
    // When
    store
        .publish_asr(
            &job,
            &[],
            AsrTranscriptMetadata {
                engine: "whisper.cpp".into(),
                model: "model.gguf".into(),
                requested_language: AsrLanguage::Ko,
                result_language: None,
                chunk_seconds: 10,
                overlap_seconds: 2,
                ownership_policy: AsrOwnershipPolicy::HalfOpenMidpoint,
                timing_quality: AsrTimingQuality::ModelEstimated,
            },
        )
        .await?;
    // Then
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM asr_checkpoints WHERE job_id=?")
        .bind(&job.id)
        .fetch_one(&store.pool)
        .await?;
    assert_eq!(count, 0);
    Ok(())
}

#[tokio::test]
async fn store_reopen_preserves_checkpoint_and_marks_job_resume_eligible() -> TestResult {
    // Given
    let root = tempfile::tempdir()?;
    let path = root.path().join("store.sqlite3");
    let (store, project_id) = fixture_at(&path).await?;
    let (job, _) = store
        .enqueue_asr(
            &project_id,
            "source",
            AsrLanguage::Ko,
            "model.gguf",
            &snapshot(),
        )
        .await?;
    store.start_asr(&job.id, 3).await?;
    store
        .checkpoint_asr(
            &job.id,
            &Checkpoint {
                index: 0,
                inference_start: 0,
                inference_end: 360,
                owner_start: 0,
                owner_end: 300,
                payload: "{\"transcription\":[],\"result\":{\"language\":null}}".into(),
            },
        )
        .await?;
    drop(store);
    // When
    let reopened = crate::store::Store::open(&path).await?;
    let recovered = reopened.get_asr(&project_id, "source", &job.id).await?;
    // Then
    let version: i64 = sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&reopened.pool)
        .await?;
    assert_eq!(version, 9);
    assert_eq!(recovered.status, AsrJobStatus::Failed);
    assert!(recovered.resume_eligible);
    assert_eq!(recovered.progress.completed_chunks, 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM asr_checkpoints WHERE job_id=?")
        .bind(&job.id)
        .fetch_one(&reopened.pool)
        .await?;
    assert_eq!(count, 1);
    Ok(())
}
