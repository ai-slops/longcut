use super::{
    AsrJobStatus, AsrLanguage,
    checkpoint::Checkpoint,
    resume_test_support::{
        TestResult, cancelled_material, fixture, replace_progress, resume_job, snapshot,
    },
};

#[tokio::test]
async fn gapped_and_corrupted_checkpoints_reject_resume_without_status_mutation() -> TestResult {
    // Given
    let (store, project_id) = fixture().await?;
    let material = cancelled_material(&store, &project_id).await?;
    sqlx::query("INSERT INTO asr_checkpoints(job_id,chunk_index,inference_start,inference_end,owner_start,owner_end,payload) VALUES(?,1,'240','660','300','600','{\"transcription\":[],\"result\":{\"language\":null}}')")
        .bind(&material.job.id).execute(&store.pool).await?;
    // When
    let gap_result = resume_job(&store, &material, &snapshot()).await;
    sqlx::query("DELETE FROM asr_checkpoints WHERE job_id=?")
        .bind(&material.job.id)
        .execute(&store.pool)
        .await?;
    sqlx::query("INSERT INTO asr_checkpoints(job_id,chunk_index,inference_start,inference_end,owner_start,owner_end,payload) VALUES(?,0,'0','360','0','300','{')")
        .bind(&material.job.id).execute(&store.pool).await?;
    let corruption_result = resume_job(&store, &material, &snapshot()).await;
    // Then
    assert!(gap_result.is_err());
    assert!(corruption_result.is_err());
    assert_eq!(
        store
            .get_asr(&project_id, "source", &material.job.id)
            .await?
            .status,
        AsrJobStatus::Cancelled
    );
    Ok(())
}

#[tokio::test]
async fn resume_rejects_checkpoint_progress_or_total_mismatch_without_mutation() -> TestResult {
    // Given
    let (progress_store, progress_project) = fixture().await?;
    let progress_material = cancelled_material(&progress_store, &progress_project).await?;
    replace_progress(&progress_store, &progress_material.job.id, 1, 3).await?;
    let (total_store, total_project) = fixture().await?;
    let total_material = cancelled_material(&total_store, &total_project).await?;
    replace_progress(&total_store, &total_material.job.id, 0, 2).await?;
    // When
    let progress_result = resume_job(&progress_store, &progress_material, &snapshot()).await;
    let total_result = resume_job(&total_store, &total_material, &snapshot()).await;
    // Then
    assert!(progress_result.is_err());
    assert!(total_result.is_err());
    assert_eq!(
        progress_store
            .get_asr(&progress_project, "source", &progress_material.job.id)
            .await?
            .status,
        AsrJobStatus::Cancelled
    );
    assert_eq!(
        total_store
            .get_asr(&total_project, "source", &total_material.job.id)
            .await?
            .status,
        AsrJobStatus::Cancelled
    );
    Ok(())
}

#[tokio::test]
async fn checkpoint_insert_rejects_index_at_total_chunks() -> TestResult {
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
    store.start_asr(&job.id, 1).await?;
    store
        .checkpoint_asr(
            &job.id,
            &Checkpoint {
                index: 0,
                inference_start: 0,
                inference_end: 900,
                owner_start: 0,
                owner_end: 900,
                payload: "{\"transcription\":[],\"result\":{\"language\":null}}".into(),
            },
        )
        .await?;
    // When
    let result = store
        .checkpoint_asr(
            &job.id,
            &Checkpoint {
                index: 1,
                inference_start: 0,
                inference_end: 0,
                owner_start: 0,
                owner_end: 0,
                payload: "{\"transcription\":[],\"result\":{\"language\":null}}".into(),
            },
        )
        .await;
    // Then
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn activation_rejects_checkpoint_aggregate_changed_after_validation() -> TestResult {
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
    store.cancel_asr(&project_id, "source", &job.id).await?;
    let material = store
        .resume_material(&project_id, "source", &job.id)
        .await?;
    let validated = store.validate_resume_checkpoints(&material).await?;
    sqlx::query("INSERT INTO asr_checkpoints(job_id,chunk_index,inference_start,inference_end,owner_start,owner_end,payload) VALUES(?,1,'240','660','300','600','{\"transcription\":[],\"result\":{\"language\":null}}')")
        .bind(&job.id).execute(&store.pool).await?;
    // When
    let result = store.resume_asr(&material, &snapshot(), &validated).await;
    // Then
    assert!(result.is_err());
    assert_eq!(
        store.get_asr(&project_id, "source", &job.id).await?.status,
        AsrJobStatus::Cancelled
    );
    Ok(())
}
