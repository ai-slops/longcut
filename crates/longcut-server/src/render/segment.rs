use std::{path::Path, process::Stdio, time::Duration};

use tokio::{process::Command, time::timeout};

use crate::{
    error::{AppError, AppResult},
    model::{Project, Scene, Source},
};

pub(super) async fn render(
    artifacts: &Path,
    project: &Project,
    scene: &Scene,
    output: &Path,
) -> AppResult<()> {
    let source = find_source(project, &scene.source_id)?;
    let fps = f64::from(project.frame_rate);
    let start_frame = parse_frame(&scene.in_frame)?;
    let duration_frames = parse_frame(&scene.out_frame)?
        .checked_sub(start_frame)
        .ok_or_else(|| AppError::Invalid("invalid persisted scene range".into()))?;
    let start = start_frame as f64 / fps;
    let duration = duration_frames as f64 / fps;
    let mut command = Command::new("ffmpeg");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-ss",
            &start.to_string(),
            "-t",
            &duration.to_string(),
            "-i",
        ])
        .arg(artifacts.join(crate::media::artifact_name(&source.media_url)?));
    let mut silence_index = 1;
    if let Some(camera) = &scene.camera {
        let camera_source = find_source(project, &camera.source_id)?;
        silence_index = 2;
        command
            .args([
                "-ss",
                &(parse_frame(&camera.in_frame)? as f64 / fps).to_string(),
                "-t",
                &duration.to_string(),
                "-i",
            ])
            .arg(artifacts.join(crate::media::artifact_name(&camera_source.media_url)?));
    }
    command.args([
        "-f",
        "lavfi",
        "-t",
        &duration.to_string(),
        "-i",
        "anullsrc=channel_layout=stereo:sample_rate=48000",
    ]);
    let video_filter = if let Some(camera) = &scene.camera {
        let width = pixels(camera.width, 1920.0);
        let height = pixels(camera.height, 1080.0);
        let x = pixels(camera.x, 1920.0);
        let y = pixels(camera.y, 1080.0);
        format!(
            "[0:v]scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={}[base];[1:v]scale={width}:{height},format=rgba,colorchannelmixer=aa={}[cam];[base][cam]overlay={x}:{y}:shortest=1[v]",
            project.frame_rate, camera.opacity
        )
    } else {
        format!(
            "[0:v]scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={}[v]",
            project.frame_rate
        )
    };
    command.args(["-filter_complex", &video_filter, "-map", "[v]"]);
    if source.has_audio {
        command.args(["-map", "0:a:0"]);
    } else {
        command.args(["-map", &format!("{silence_index}:a:0")]);
    }
    command
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "20",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-b:a",
            "192k",
            "-shortest",
            "-y",
        ])
        .arg(output);
    run(&mut command, "scene render").await
}

pub(super) async fn run(command: &mut Command, label: &str) -> AppResult<()> {
    command.kill_on_drop(true);
    let output = timeout(
        Duration::from_secs(600),
        command
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .map_err(|_| AppError::Internal(anyhow::anyhow!("{label} timed out")))??;
    if !output.status.success() {
        return Err(AppError::Internal(anyhow::anyhow!(
            "{label} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(())
}

fn find_source<'a>(project: &'a Project, id: &str) -> AppResult<&'a Source> {
    project
        .sources
        .iter()
        .find(|source| source.id == id)
        .ok_or(AppError::NotFound)
}
fn parse_frame(value: &str) -> AppResult<u64> {
    value
        .parse()
        .map_err(|_| AppError::Invalid("invalid persisted frame".into()))
}
fn pixels(value: f32, total: f32) -> i32 {
    if value <= 1.0 {
        (value * total).round() as i32
    } else {
        value.round() as i32
    }
}
