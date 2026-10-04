use std::{fs, path::Path};

use longcut_session_contract::model::Registry;
use sha2::{Digest, Sha256};
use tokio::process::Command;

pub type TestResult = Result<(), Box<dyn std::error::Error>>;

pub async fn prepare(root: &Path) -> TestResult {
    if fs::read_dir(root)?.next().is_some() {
        return Err("fixture directory must be empty".into());
    }
    let fixture =
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
            fixture.join(format!("{name}.json")),
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
            "10",
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
    let checksum = format!("sha256:{:x}", Sha256::digest(&bytes));
    let mut registry: Registry = serde_json::from_slice(&fs::read(root.join("sources.json"))?)?;
    for asset in &mut registry.assets {
        asset.size_bytes = Some(longcut_session_contract::model::Integer::new(
            i64::try_from(bytes.len())?,
        ));
        asset.content_hash = Some(checksum.clone());
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
