use sqlx::Row;

use super::{
    AsrLanguage,
    checkpoint::Checkpoint,
    resume_test_support::{TestResult, fixture_at, snapshot},
};

enum FingerprintShape {
    Absent,
    Empty,
}

#[tokio::test]
async fn v5_without_fingerprint_column_backfills_populated_job() -> TestResult {
    migrated_v5_job_passes_resume_preflight(FingerprintShape::Absent).await
}

#[tokio::test]
async fn v5_with_empty_fingerprint_backfills_populated_job() -> TestResult {
    migrated_v5_job_passes_resume_preflight(FingerprintShape::Empty).await
}

#[tokio::test]
async fn v4_job_without_resume_snapshot_remains_ineligible() -> TestResult {
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
    sqlx::query("DROP TABLE transcript_words; DROP TABLE asr_checkpoints; ALTER TABLE asr_jobs DROP COLUMN checkpoint_fingerprint; ALTER TABLE asr_jobs DROP COLUMN resume_snapshot; PRAGMA user_version=4;")
        .execute(&store.pool)
        .await?;
    drop(store);
    // When
    let reopened = crate::store::Store::open(&path).await?;
    // Then
    assert_eq!(schema_version(&reopened).await?, 9);
    assert!(
        !reopened
            .get_asr(&project_id, "source", &job.id)
            .await?
            .resume_eligible
    );
    Ok(())
}

async fn migrated_v5_job_passes_resume_preflight(shape: FingerprintShape) -> TestResult {
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
    match shape {
        FingerprintShape::Absent => {
            sqlx::query("ALTER TABLE asr_jobs DROP COLUMN checkpoint_fingerprint")
                .execute(&store.pool)
                .await?;
        }
        FingerprintShape::Empty => {
            sqlx::query("UPDATE asr_jobs SET checkpoint_fingerprint='' WHERE id=?")
                .bind(&job.id)
                .execute(&store.pool)
                .await?;
        }
    }
    sqlx::query("DROP TABLE transcript_words; PRAGMA user_version=5")
        .execute(&store.pool)
        .await?;
    drop(store);
    // When
    let reopened = crate::store::Store::open(&path).await?;
    let material = reopened
        .resume_material(&project_id, "source", &job.id)
        .await?;
    let validated = reopened.validate_resume_checkpoints(&material).await;
    // Then
    assert_eq!(schema_version(&reopened).await?, 9);
    assert!(validated.is_ok());
    let fingerprint: String = sqlx::query("SELECT checkpoint_fingerprint FROM asr_jobs WHERE id=?")
        .bind(&job.id)
        .fetch_one(&reopened.pool)
        .await?
        .try_get("checkpoint_fingerprint")?;
    assert!(!fingerprint.is_empty());
    Ok(())
}

async fn schema_version(store: &crate::store::Store) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("PRAGMA user_version")
        .fetch_one(&store.pool)
        .await
}
