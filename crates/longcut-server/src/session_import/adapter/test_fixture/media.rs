use std::{fs, path::Path, process::Stdio};

use longcut_session_contract::model::{Integer, Registry, TimeMaps};
use serde_json::Value;
use tokio::process::Command;

use super::TestResult;

pub async fn shift_program(root: &Path, seconds: i64) -> TestResult {
    let shifted = root.join("media/shifted.mp4");
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-nostdin",
            "-itsoffset",
            &seconds.to_string(),
            "-i",
        ])
        .arg(root.join("media/program.mp4"))
        .args([
            "-map",
            "0",
            "-c",
            "copy",
            "-avoid_negative_ts",
            "disabled",
            "-y",
        ])
        .arg(&shifted)
        .stderr(Stdio::piped())
        .output()
        .await?;
    require_success(&output, "fixture timestamp shift")?;
    let sources_path = root.join("sources.json");
    let mut registry: Registry = serde_json::from_slice(&fs::read(&sources_path)?)?;
    registry.assets[0].uri = "media/shifted.mp4".into();
    registry.assets[0].size_bytes = None;
    registry.assets[0].content_hash = None;
    fs::write(sources_path, serde_json::to_vec_pretty(&registry)?)?;

    let maps_path = root.join("time-maps.json");
    let mut maps: TimeMaps = serde_json::from_slice(&fs::read(&maps_path)?)?;
    let video_offset = seconds * 30;
    maps.segments[0].media_range.start.ticks = Integer::new(video_offset);
    maps.segments[0].media_range.end.ticks = Integer::new(video_offset + 300);
    let audio_offset = seconds * 48_000;
    maps.segments[1].media_range.start.ticks = Integer::new(audio_offset);
    maps.segments[1].media_range.end.ticks = Integer::new(audio_offset + 480_000);
    fs::write(maps_path, serde_json::to_vec_pretty(&maps)?)?;
    Ok(())
}

pub async fn video_packet_bounds(path: &Path) -> TestResult<(i64, i64, String)> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=time_base:packet=pts,duration",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .await?;
    require_success(&output, "adapter packet probe")?;
    let value: Value = serde_json::from_slice(&output.stdout)?;
    let packets = value["packets"].as_array().ok_or("missing packet probe")?;
    let first = packets
        .first()
        .and_then(|packet| packet["pts"].as_i64())
        .ok_or("missing first packet PTS")?;
    let end = packets
        .iter()
        .filter_map(|packet| Some(packet["pts"].as_i64()? + packet["duration"].as_i64()?))
        .max()
        .ok_or("empty packet probe")?;
    let time_base = value["streams"][0]["time_base"]
        .as_str()
        .ok_or("missing video time base")?
        .to_owned();
    Ok((first, end, time_base))
}

fn require_success(output: &std::process::Output, action: &str) -> TestResult {
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{action} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    )
    .into())
}
