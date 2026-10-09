use super::{
    AsrJobStatus, AsrLanguage,
    checkpoint::{Checkpoint, MAX_CHECKPOINT_BYTES, MAX_JOB_CHECKPOINT_BYTES},
    resume_test_support::{TestResult, cancelled_material, fixture, resume_job, snapshot},
};

#[tokio::test]
async fn checkpoint_byte_caps_reject_before_progress_mutation() -> TestResult {
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
    store.start_asr(&job.id, 3).await?;
    let oversized = Checkpoint {
        index: 0,
        inference_start: 0,
        inference_end: 360,
        owner_start: 0,
        owner_end: 300,
        payload: "x".repeat(MAX_CHECKPOINT_BYTES + 1),
    };
    // When
    let per_checkpoint = store.checkpoint_asr(&job.id, &oversized).await;
    sqlx::query("INSERT INTO asr_checkpoints(job_id,chunk_index,inference_start,inference_end,owner_start,owner_end,payload) VALUES(?,99,'0','0','0','0',zeroblob(?))")
        .bind(&job.id).bind(i64::try_from(MAX_JOB_CHECKPOINT_BYTES)?).execute(&store.pool).await?;
    let per_job = store
        .checkpoint_asr(
            &job.id,
            &Checkpoint {
                payload: "{}".into(),
                ..oversized
            },
        )
        .await;
    // Then
    assert!(per_checkpoint.is_err());
    assert!(per_job.is_err());
    assert_eq!(
        store
            .get_asr(&project_id, "source", &job.id)
            .await?
            .progress
            .completed_chunks,
        0
    );
    Ok(())
}

#[tokio::test]
async fn persisted_oversized_payload_is_rejected_by_preflight_without_mutation() -> TestResult {
    // Given
    let (store, project_id) = fixture().await?;
    let material = cancelled_material(&store, &project_id).await?;
    sqlx::query("INSERT INTO asr_checkpoints(job_id,chunk_index,inference_start,inference_end,owner_start,owner_end,payload) VALUES(?,0,'0','360','0','300',zeroblob(?))")
        .bind(&material.job.id).bind(i64::try_from(MAX_CHECKPOINT_BYTES + 1)?).execute(&store.pool).await?;
    // When
    let result = resume_job(&store, &material, &snapshot()).await;
    // Then
    assert!(result.is_err_and(|error| error.to_string().contains("checkpoint")));
    assert_eq!(
        store
            .get_asr(&project_id, "source", &material.job.id)
            .await?
            .status,
        AsrJobStatus::Cancelled
    );
    Ok(())
}
