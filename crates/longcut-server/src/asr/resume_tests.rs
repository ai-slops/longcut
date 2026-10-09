use crate::transcript::ImportTranscript;

use super::{
    AsrJobStatus,
    resume_test_support::{TestResult, cancelled_material, fixture, resume_job, snapshot},
};

#[tokio::test]
async fn resume_rejects_each_changed_fingerprint_or_policy_without_mutation() -> TestResult {
    // Given
    let (store, project_id) = fixture().await?;
    let material = cancelled_material(&store, &project_id).await?;
    let mut changed = Vec::new();
    enum ChangedField {
        Source,
        Model,
        Cli,
        ChunkPolicy,
    }
    for field in [
        ChangedField::Source,
        ChangedField::Model,
        ChangedField::Cli,
        ChangedField::ChunkPolicy,
    ] {
        let mut value = snapshot();
        match field {
            ChangedField::Source => value.source_sha256.push('x'),
            ChangedField::Model => value.model_sha256.push('x'),
            ChangedField::Cli => value.cli_sha256.push('x'),
            ChangedField::ChunkPolicy => value.chunk_seconds += 1,
        }
        changed.push(value);
    }
    // When / Then
    for current in changed {
        assert!(resume_job(&store, &material, &current).await.is_err());
        assert_eq!(
            store
                .get_asr(&project_id, "source", &material.job.id)
                .await?
                .status,
            AsrJobStatus::Cancelled
        );
    }
    Ok(())
}

#[tokio::test]
async fn resume_rejects_newer_manual_transcript_without_mutation() -> TestResult {
    // Given
    let (store, project_id) = fixture().await?;
    let material = cancelled_material(&store, &project_id).await?;
    store
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
    let result = resume_job(&store, &material, &snapshot()).await;
    // Then
    assert!(result.is_err());
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
async fn concurrent_resume_allows_exactly_one_transition() -> TestResult {
    // Given
    let (store, project_id) = fixture().await?;
    let material = cancelled_material(&store, &project_id).await?;
    let first_snapshot = snapshot();
    let second_snapshot = snapshot();
    // When
    let (first, second) = tokio::join!(
        resume_job(&store, &material, &first_snapshot),
        resume_job(&store, &material, &second_snapshot),
    );
    // Then
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    Ok(())
}
