use std::process::Stdio;

use longcut_server::{
    AppState,
    ai::{AiScope, sampling},
    model::{CreateProject, MediaStatus, Source, SourceRole},
};
use tempfile::TempDir;
use tokio::process::Command;

#[tokio::test]
async fn real_ffmpeg_sampling_reads_managed_original_and_returns_jpeg() {
    // Given
    if !longcut_server::media::tool_available("ffmpeg").await {
        return;
    }
    let data = TempDir::new().expect("create temp directory");
    let state = AppState::initialize(data.path())
        .await
        .expect("initialize state");
    let project = state
        .store
        .create(CreateProject {
            title: "Sampling".into(),
            frame_rate: 30,
        })
        .await
        .expect("create project");
    let project = state
        .store
        .add_source(
            &project.id,
            Source {
                id: "source".into(),
                name: "source.mp4".into(),
                role: SourceRole::Program,
                duration_frames: "180".into(),
                width: 1280,
                height: 720,
                has_audio: false,
                media_url: "/api/v1/media/source-original.mp4".into(),
                proxy_url: None,
                thumbnail_url: None,
                waveform: vec![],
                status: MediaStatus::Ready,
                error: None,
            },
        )
        .await
        .expect("add source");
    let media_path = state.artifacts.join("source-original.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1280x720:rate=30",
            "-t",
            "6",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&media_path)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .await
        .expect("run ffmpeg fixture");
    assert!(status.success());
    // When
    let samples = sampling::sample(
        &state.artifacts,
        &project,
        &AiScope::Source {
            source_id: "source".into(),
            start_frame: "0".into(),
            end_frame: "180".into(),
        },
    )
    .await
    .expect("sample media");
    // Then
    assert_eq!(samples.len(), 12);
    assert_eq!(samples[0].evidence_id, "frame_0001");
    assert_eq!(&samples[0].jpeg[..2], &[0xff, 0xd8]);
    assert!(samples.iter().all(|sample| sample.jpeg.len() < 2_000_000));
}
