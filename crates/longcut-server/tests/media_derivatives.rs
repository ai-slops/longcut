use std::{path::Path, process::Stdio, time::Duration};

use longcut_server::{
    media::{artifact_name, derivatives, derivatives_with_cancel},
    model::{MediaStatus, Source, SourceRole},
};
use tempfile::TempDir;
use tokio::{fs, process::Command, time::timeout};
use tokio_util::sync::CancellationToken;

async fn generate_video(path: &Path, duration: u32) {
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000",
            "-t",
            &duration.to_string(),
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-threads",
            "1",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-y",
        ])
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .await
        .expect("start fixture ffmpeg");
    assert!(
        status.success(),
        "fixture ffmpeg must be installed and succeed"
    );
}

async fn generate_audio(path: &Path) {
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=997:sample_rate=48000:duration=2",
            "-c:a",
            "pcm_s16le",
            "-threads",
            "1",
            "-y",
        ])
        .arg(path)
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .await
        .expect("start fixture ffmpeg");
    assert!(
        status.success(),
        "fixture ffmpeg must be installed and succeed"
    );
}

fn source(id: &str, artifact: &str, has_video: bool) -> Source {
    Source {
        id: id.into(),
        name: artifact.into(),
        role: SourceRole::Program,
        duration_frames: "60".into(),
        width: if has_video { 1280 } else { 0 },
        height: if has_video { 720 } else { 0 },
        has_audio: true,
        media_url: format!("/api/v1/media/{artifact}"),
        proxy_url: None,
        thumbnail_url: None,
        waveform: vec![],
        status: MediaStatus::Processing,
        error: None,
    }
}

#[tokio::test]
async fn derivatives_publish_real_video_artifacts_when_ffmpeg_succeeds() {
    // Given
    let artifacts = TempDir::new().expect("create artifacts directory");
    generate_video(&artifacts.path().join("video-original.mp4"), 2).await;
    let source = source("video", "video-original.mp4", true);

    // When
    let ready = derivatives(artifacts.path(), &source)
        .await
        .expect("generate derivatives");

    // Then
    assert_eq!(ready.status, MediaStatus::Ready);
    assert_eq!(
        ready.proxy_url.as_deref(),
        Some("/api/v1/media/video-proxy.mp4")
    );
    assert_eq!(
        ready.thumbnail_url.as_deref(),
        Some("/api/v1/media/video-thumbnail.jpg")
    );
    assert!(!ready.waveform.is_empty());
    assert!(ready.waveform.len() <= 512);
    for url in [ready.proxy_url, ready.thumbnail_url].into_iter().flatten() {
        let path = artifacts
            .path()
            .join(artifact_name(&url).expect("artifact URL"));
        assert!(fs::metadata(path).await.expect("published artifact").len() > 0);
    }
}

#[tokio::test]
async fn derivatives_publish_audio_preview_without_video_thumbnail() {
    // Given
    let artifacts = TempDir::new().expect("create artifacts directory");
    generate_audio(&artifacts.path().join("audio-original.wav")).await;
    let source = source("audio", "audio-original.wav", false);

    // When
    let ready = derivatives(artifacts.path(), &source)
        .await
        .expect("generate audio derivatives");

    // Then
    assert_eq!(ready.status, MediaStatus::Ready);
    assert_eq!(
        ready.proxy_url.as_deref(),
        Some("/api/v1/media/audio-proxy.mp4")
    );
    assert_eq!(ready.thumbnail_url, None);
    assert!(!ready.waveform.is_empty());
    assert!(ready.waveform.len() <= 512);
    assert!(ready.waveform.iter().all(|peak| (0.0..=1.0).contains(peak)));
    assert!(
        fs::metadata(artifacts.path().join("audio-proxy.mp4"))
            .await
            .is_ok()
    );
    assert!(
        fs::metadata(artifacts.path().join("audio-thumbnail.jpg"))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn derivatives_kill_reap_and_clean_temporary_files_when_cancelled() {
    // Given
    let artifacts = TempDir::new().expect("create artifacts directory");
    generate_video(&artifacts.path().join("cancel-original.mp4"), 2).await;
    let source = source("cancel", "cancel-original.mp4", true);
    let cancel = CancellationToken::new();
    cancel.cancel();

    // When
    let result = timeout(
        Duration::from_secs(5),
        derivatives_with_cancel(artifacts.path(), &source, &cancel),
    )
    .await
    .expect("cancellation completes promptly");

    // Then
    assert!(result.is_err());
    for name in [
        ".cancel-proxy.tmp.mp4",
        ".cancel-thumbnail.tmp.jpg",
        "cancel-proxy.mp4",
        "cancel-thumbnail.jpg",
    ] {
        assert!(
            fs::metadata(artifacts.path().join(name)).await.is_err(),
            "left {name}"
        );
    }
}
