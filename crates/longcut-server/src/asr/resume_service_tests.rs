#![cfg(unix)]

use std::{os::unix::fs::PermissionsExt, path::Path};

use sha2::{Digest, Sha256};

use crate::{
    AppState,
    model::{CreateProject, MediaStatus, Source, SourceRole},
};

use super::{AsrConfig, AsrJobStatus, AsrLanguage, ResumeSnapshot};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn executable(path: &Path, contents: &str) -> TestResult {
    std::fs::write(path, contents)?;
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

fn digest(path: &Path) -> Result<String, std::io::Error> {
    Ok(format!("{:x}", Sha256::digest(std::fs::read(path)?)))
}

struct Fixture {
    state: AppState,
    project_id: String,
    job_id: String,
}

async fn fixture(
    media_url: &str,
    valid_hashes: bool,
) -> Result<Fixture, Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?.keep();
    let cli = root.join("whisper");
    let model = root.join("model.gguf");
    let ffmpeg = root.join("ffmpeg");
    executable(&cli, "#!/bin/sh\nexit 1\n")?;
    executable(
        &ffmpeg,
        "#!/bin/sh\nif [ \"$1\" = \"-version\" ]; then exit 0; fi\nexit 1\n",
    )?;
    std::fs::write(&model, b"model")?;
    let state = AppState::initialize_with_asr_config(
        &root,
        AsrConfig::from_paths(cli.clone(), model.clone()).with_ffmpeg(ffmpeg),
    )
    .await?;
    let project = state
        .store
        .create(CreateProject {
            title: "resume service".into(),
            frame_rate: 30,
        })
        .await?;
    state
        .store
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
                media_url: media_url.into(),
                proxy_url: None,
                thumbnail_url: None,
                waveform: Vec::new(),
                status: MediaStatus::Ready,
                error: None,
            },
        )
        .await?;
    let source_path = state.artifacts.join("source.mp4");
    std::fs::write(&source_path, b"source")?;
    let snapshot = ResumeSnapshot {
        source_sha256: if valid_hashes {
            digest(&source_path)?
        } else {
            "old-source".into()
        },
        model_sha256: if valid_hashes {
            digest(&model)?
        } else {
            "old-model".into()
        },
        cli_sha256: if valid_hashes {
            digest(&cli)?
        } else {
            "old-cli".into()
        },
        source_media_url: media_url.into(),
        model_path: model.to_string_lossy().into_owned(),
        cli_path: cli.to_string_lossy().into_owned(),
        duration_frames: 900,
        frame_rate: 30,
        language: AsrLanguage::Ko,
        chunk_seconds: 600,
        overlap_seconds: 2,
        conversion_policy_version: 1,
        argument_policy_version: 1,
    };
    let (job, _) = state
        .store
        .enqueue_asr(
            &project.id,
            "source",
            AsrLanguage::Ko,
            &snapshot.model_path,
            &snapshot,
        )
        .await?;
    state.store.start_asr(&job.id, 1).await?;
    state
        .store
        .cancel_asr(&project.id, "source", &job.id)
        .await?;
    Ok(Fixture {
        state,
        project_id: project.id,
        job_id: job.id,
    })
}

#[tokio::test]
async fn aborting_resume_during_hash_does_not_leak_reservation() -> TestResult {
    // Given
    let fixture = fixture("/api/v1/media/source.mp4", false).await?;
    let gate = fixture.state.asr.install_resume_gate(true).await;
    let state = fixture.state.clone();
    let project_id = fixture.project_id.clone();
    let job_id = fixture.job_id.clone();
    let attempt = tokio::spawn(async move {
        state
            .asr
            .resume(&state, &project_id, "source", &job_id)
            .await
    });
    gate.entered.wait().await;
    // When
    attempt.abort();
    gate.release.wait().await;
    let retry = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let result = fixture
                .state
                .asr
                .resume(
                    &fixture.state,
                    &fixture.project_id,
                    "source",
                    &fixture.job_id,
                )
                .await;
            if !result
                .as_ref()
                .is_err_and(|error| error.to_string().contains("still stopping"))
            {
                break result;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    // Then
    assert!(!retry.is_err_and(|error| error.to_string().contains("still stopping")));
    Ok(())
}

#[tokio::test]
async fn invalid_media_url_failure_releases_reservation_for_retry() -> TestResult {
    // Given
    let fixture = fixture("invalid", false).await?;
    // When
    let first = fixture
        .state
        .asr
        .resume(
            &fixture.state,
            &fixture.project_id,
            "source",
            &fixture.job_id,
        )
        .await;
    let second = fixture
        .state
        .asr
        .resume(
            &fixture.state,
            &fixture.project_id,
            "source",
            &fixture.job_id,
        )
        .await;
    // Then
    assert!(first.is_err());
    assert!(!second.is_err_and(|error| error.to_string().contains("still stopping")));
    Ok(())
}

#[tokio::test]
async fn cancellation_before_activation_returns_cancelled_without_failed_worker() -> TestResult {
    // Given
    let fixture = fixture("/api/v1/media/source.mp4", true).await?;
    let gate = fixture.state.asr.install_resume_gate(false).await;
    let state = fixture.state.clone();
    let project_id = fixture.project_id.clone();
    let job_id = fixture.job_id.clone();
    let attempt = tokio::spawn(async move {
        state
            .asr
            .resume(&state, &project_id, "source", &job_id)
            .await
    });
    gate.entered.wait().await;
    fixture
        .state
        .store
        .cancel_asr(&fixture.project_id, "source", &fixture.job_id)
        .await?;
    fixture.state.asr.cancel(&fixture.job_id).await;
    // When
    gate.release.wait().await;
    let resumed = attempt.await??;
    // Then
    assert_eq!(resumed.status, AsrJobStatus::Cancelled);
    let persisted = fixture
        .state
        .store
        .get_asr(&fixture.project_id, "source", &fixture.job_id)
        .await?;
    assert_eq!(persisted.status, AsrJobStatus::Cancelled);
    Ok(())
}
