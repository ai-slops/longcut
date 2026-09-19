use std::path::Path;

use serde::Serialize;
use tokio::fs;

use crate::{
    error::{AppError, AppResult},
    model::Project,
};

#[derive(Serialize)]
struct Lineage<'a> {
    project_id: &'a str,
    revision: &'a str,
    scenes: Vec<LineageScene<'a>>,
}
#[derive(Serialize)]
struct LineageScene<'a> {
    scene_id: &'a str,
    source_id: &'a str,
    source_in_frame: &'a str,
    source_out_frame: &'a str,
    output_in_frame: String,
    output_out_frame: String,
    camera_source_id: Option<&'a str>,
    camera_in_frame: Option<&'a str>,
}

pub(super) struct Sidecars {
    pub srt: String,
    pub vtt: String,
    pub lineage: String,
}

pub(super) async fn write(
    artifacts: &Path,
    project: &Project,
    render_id: &str,
) -> AppResult<Sidecars> {
    let (srt, vtt, lineage) = build(project)?;
    let srt_name = format!("{render_id}.srt");
    let vtt_name = format!("{render_id}.vtt");
    let lineage_name = format!("{render_id}-lineage.json");
    atomic_write(artifacts, &srt_name, srt.as_bytes()).await?;
    atomic_write(artifacts, &vtt_name, vtt.as_bytes()).await?;
    atomic_write(
        artifacts,
        &lineage_name,
        &serde_json::to_vec_pretty(&lineage)?,
    )
    .await?;
    Ok(Sidecars {
        srt: srt_name,
        vtt: vtt_name,
        lineage: lineage_name,
    })
}

fn build(project: &Project) -> AppResult<(String, String, Lineage<'_>)> {
    let mut srt = String::new();
    let mut vtt = String::from("WEBVTT\n\n");
    let mut output_frame = 0_u64;
    let mut scenes = Vec::new();
    let mut index = 1;
    for scene in &project.scenes {
        let length = frame(&scene.out_frame)?
            .checked_sub(frame(&scene.in_frame)?)
            .ok_or_else(|| AppError::Invalid("invalid persisted scene range".into()))?;
        let output_end = output_frame
            .checked_add(length)
            .ok_or_else(|| AppError::Invalid("output frame overflow".into()))?;
        if !scene.caption.trim().is_empty() {
            let start_ms = output_frame
                .checked_mul(1000)
                .ok_or_else(|| AppError::Invalid("caption timestamp overflow".into()))?
                / u64::from(project.frame_rate);
            let end_ms = output_end
                .checked_mul(1000)
                .ok_or_else(|| AppError::Invalid("caption timestamp overflow".into()))?
                / u64::from(project.frame_rate);
            srt.push_str(&format!(
                "{index}\n{} --> {}\n{}\n\n",
                timestamp(start_ms, ','),
                timestamp(end_ms, ','),
                scene.caption
            ));
            vtt.push_str(&format!(
                "{} --> {}\n{}\n\n",
                timestamp(start_ms, '.'),
                timestamp(end_ms, '.'),
                scene.caption
            ));
            index += 1;
        }
        scenes.push(LineageScene {
            scene_id: &scene.id,
            source_id: &scene.source_id,
            source_in_frame: &scene.in_frame,
            source_out_frame: &scene.out_frame,
            output_in_frame: output_frame.to_string(),
            output_out_frame: output_end.to_string(),
            camera_source_id: scene
                .camera
                .as_ref()
                .map(|camera| camera.source_id.as_str()),
            camera_in_frame: scene.camera.as_ref().map(|camera| camera.in_frame.as_str()),
        });
        output_frame = output_end;
    }
    Ok((
        srt,
        vtt,
        Lineage {
            project_id: &project.id,
            revision: &project.revision,
            scenes,
        },
    ))
}

async fn atomic_write(dir: &Path, name: &str, bytes: &[u8]) -> AppResult<()> {
    let temp = dir.join(format!(".{name}.tmp"));
    fs::write(&temp, bytes).await?;
    fs::rename(temp, dir.join(name)).await?;
    Ok(())
}
fn frame(value: &str) -> AppResult<u64> {
    value
        .parse()
        .map_err(|_| AppError::Invalid("invalid persisted frame".into()))
}
fn timestamp(ms: u64, separator: char) -> String {
    format!(
        "{:02}:{:02}:{:02}{separator}{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}
