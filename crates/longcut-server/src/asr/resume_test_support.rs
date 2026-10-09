use crate::{
    model::{CreateProject, MediaStatus, Source, SourceRole},
    store::Store,
};

use super::{AsrJob, AsrLanguage, ResumeSnapshot, resume_store::ResumeMaterial};

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) fn snapshot() -> ResumeSnapshot {
    ResumeSnapshot {
        source_sha256: "source-hash".into(),
        model_sha256: "model-hash".into(),
        cli_sha256: "cli-hash".into(),
        source_media_url: "/api/v1/media/source.mp4".into(),
        model_path: "model.gguf".into(),
        cli_path: "whisper-cli".into(),
        duration_frames: 900,
        frame_rate: 30,
        language: AsrLanguage::Ko,
        chunk_seconds: 10,
        overlap_seconds: 2,
        conversion_policy_version: 1,
        argument_policy_version: 1,
    }
}

pub(super) async fn fixture() -> Result<(Store, String), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    fixture_at(&root.keep().join("store.sqlite3")).await
}

pub(super) async fn fixture_at(
    path: &std::path::Path,
) -> Result<(Store, String), Box<dyn std::error::Error>> {
    let store = Store::open(path).await?;
    let project = store
        .create(CreateProject {
            title: "resume".into(),
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
                duration_frames: "900".into(),
                width: 1,
                height: 1,
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

pub(super) async fn cancelled_material(
    store: &Store,
    project_id: &str,
) -> Result<ResumeMaterial, Box<dyn std::error::Error>> {
    let (job, _) = store
        .enqueue_asr(
            project_id,
            "source",
            AsrLanguage::Ko,
            "model.gguf",
            &snapshot(),
        )
        .await?;
    store.start_asr(&job.id, 3).await?;
    store.cancel_asr(project_id, "source", &job.id).await?;
    Ok(store.resume_material(project_id, "source", &job.id).await?)
}

pub(super) async fn replace_progress(
    store: &Store,
    job_id: &str,
    completed_chunks: u32,
    total_chunks: u32,
) -> TestResult {
    let raw: String = sqlx::query_scalar("SELECT job FROM asr_jobs WHERE id=?")
        .bind(job_id)
        .fetch_one(&store.pool)
        .await?;
    let mut job: AsrJob = serde_json::from_str(&raw)?;
    job.progress.completed_chunks = completed_chunks;
    job.progress.total_chunks = total_chunks;
    sqlx::query("UPDATE asr_jobs SET job=? WHERE id=?")
        .bind(serde_json::to_string(&job)?)
        .bind(job_id)
        .execute(&store.pool)
        .await?;
    Ok(())
}

pub(super) async fn resume_job(
    store: &Store,
    material: &ResumeMaterial,
    current: &ResumeSnapshot,
) -> crate::error::AppResult<AsrJob> {
    let validated = store.validate_resume_checkpoints(material).await?;
    store.resume_asr(material, current, &validated).await
}
