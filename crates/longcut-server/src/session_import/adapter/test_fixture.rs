#[path = "test_fixture/media.rs"]
mod media;

use std::{fs, path::Path, time::Duration};

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use longcut_server::{
    AppState, app,
    model::{ImportStatus, SessionImportJob},
};
use longcut_session_contract::model::{Integer, Registry};
use sha2::{Digest, Sha256};
use tokio::process::Command;
use tower::ServiceExt;

pub use media::{shift_program, video_packet_bounds};

pub type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub async fn prepare_fixture(root: &Path) -> TestResult {
    prepare_fixture_duration(root, 10).await
}

pub async fn prepare_fixture_duration(root: &Path, seconds: u32) -> TestResult {
    let source =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/edit-session-packages/complete");
    for name in [
        "manifest",
        "sources",
        "time-maps",
        "visual-composition",
        "audio-mix",
        "media-health",
    ] {
        fs::copy(
            source.join(format!("{name}.json")),
            root.join(format!("{name}.json")),
        )?;
    }
    fs::create_dir(root.join("media"))?;
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x90:rate=30",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000",
            "-t",
            &seconds.to_string(),
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-threads",
            "1",
            "-c:a",
            "aac",
            "-ac",
            "2",
            "-map_metadata",
            "-1",
        ])
        .arg(root.join("media/program.mp4"))
        .output()
        .await?;
    if !output.status.success() {
        return Err(format!(
            "fixture ffmpeg failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    fs::copy(root.join("media/program.mp4"), root.join("media/clean.mp4"))?;
    let bytes = fs::read(root.join("media/program.mp4"))?;
    let mut registry: Registry = serde_json::from_slice(&fs::read(root.join("sources.json"))?)?;
    for asset in &mut registry.assets {
        asset.size_bytes = Some(Integer::new(i64::try_from(bytes.len())?));
        asset.content_hash = Some(format!("sha256:{:x}", Sha256::digest(&bytes)));
        asset.streams[0].width = Some(160);
        asset.streams[0].height = Some(90);
        asset.streams[1].codec = "aac".into();
    }
    registry.assets[1].uri = "media/clean.mp4".into();
    fs::write(
        root.join("sources.json"),
        serde_json::to_vec_pretty(&registry)?,
    )?;
    Ok(())
}

fn multipart(root: &Path) -> TestResult<Vec<u8>> {
    let mut bytes = Vec::new();
    for name in [
        "manifest.json",
        "sources.json",
        "time-maps.json",
        "visual-composition.json",
        "audio-mix.json",
        "media-health.json",
        "media/shifted.mp4",
        "media/clean.mp4",
    ] {
        bytes.extend_from_slice(format!("--boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/octet-stream\r\n\r\n").as_bytes());
        bytes.extend(fs::read(root.join(name))?);
        bytes.extend_from_slice(b"\r\n");
    }
    bytes.extend_from_slice(b"--boundary--\r\n");
    Ok(bytes)
}

pub async fn prepare_import(
    state: &AppState,
    project_id: &str,
    package: &Path,
) -> TestResult<SessionImportJob> {
    let response = app(state.clone())
        .oneshot(
            Request::post(format!("/api/v1/projects/{project_id}/session-imports"))
                .header("content-type", "multipart/form-data; boundary=boundary")
                .body(Body::from(multipart(package)?))?,
        )
        .await?;
    if response.status() != StatusCode::ACCEPTED {
        return Err(format!("upload failed with {}", response.status()).into());
    }
    let job: SessionImportJob =
        serde_json::from_slice(&response.into_body().collect().await?.to_bytes())?;
    tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            let current = state.store.get_import(project_id, &job.id).await?;
            if current.status != ImportStatus::Validating {
                return Ok::<_, longcut_server::error::AppError>(current);
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await?
    .map_err(Into::into)
}
