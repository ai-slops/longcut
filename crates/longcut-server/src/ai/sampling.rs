use std::{path::Path, process::Stdio, time::Duration};

use tokio::{process::Command, time::timeout};
use tokio_util::sync::CancellationToken;

use super::{AiScope, EvidenceSample};
use crate::{
    error::{AppError, AppResult},
    media,
    model::{Project, Scene, Source},
};

const MAX_SAMPLES: usize = 12;
const FFMPEG_INPUT_POLICY: [&str; 2] = ["-protocol_whitelist", "file,pipe"];
const FFMPEG_METADATA_POLICY: [&str; 2] = ["-map_metadata", "-1"];

pub async fn sample(
    artifacts: &Path,
    project: &Project,
    scope: &AiScope,
) -> AppResult<Vec<EvidenceSample>> {
    sample_cancellable(artifacts, project, scope, &CancellationToken::new()).await
}

pub(crate) async fn sample_cancellable(
    artifacts: &Path,
    project: &Project,
    scope: &AiScope,
    cancellation: &CancellationToken,
) -> AppResult<Vec<EvidenceSample>> {
    let positions = positions(project, scope)?;
    let mut samples = Vec::with_capacity(positions.len());
    for (index, (source, frame)) in positions.into_iter().enumerate() {
        let path = artifacts.join(media::artifact_name(&source.media_url)?);
        let width = if source.width >= 1024 { "1024" } else { "768" };
        let seconds = frame as f64 / f64::from(project.frame_rate);
        let ffmpeg = Command::new("ffmpeg")
            .kill_on_drop(true)
            .args(["-hide_banner", "-loglevel", "error"])
            .args(FFMPEG_INPUT_POLICY)
            .arg("-ss")
            .arg(format!("{seconds:.6}"))
            .arg("-i")
            .arg(path)
            .args(FFMPEG_METADATA_POLICY)
            .args([
                "-frames:v",
                "1",
                "-vf",
                &format!("scale={width}:-2"),
                "-f",
                "image2pipe",
                "-vcodec",
                "mjpeg",
                "pipe:1",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();
        let output = tokio::select! {
            biased;
            () = cancellation.cancelled() => {
                return Err(AppError::Conflict("AI run was cancelled".into()));
            }
            result = timeout(Duration::from_secs(30), ffmpeg) => {
                result.map_err(|_| AppError::Internal(anyhow::anyhow!("AI frame sampling timed out")))??
            }
        };
        if !output.status.success() || !output.stdout.starts_with(&[0xff, 0xd8]) {
            return Err(AppError::Internal(anyhow::anyhow!(
                "AI frame sampling failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        samples.push(EvidenceSample {
            evidence_id: format!("frame_{:04}", index + 1),
            source_id: source.id.clone(),
            frame,
            jpeg: output.stdout,
        });
    }
    Ok(samples)
}

fn positions<'a>(project: &'a Project, scope: &AiScope) -> AppResult<Vec<(&'a Source, u64)>> {
    match scope {
        AiScope::Source {
            source_id,
            start_frame,
            end_frame,
        } => {
            let source = source(project, source_id)?;
            let start = frame(start_frame)?;
            let end = frame(end_frame)?;
            Ok(spread(source, start, end, MAX_SAMPLES))
        }
        AiScope::Scenes { scene_ids } => scene_positions(project, scene_ids),
    }
}

fn scene_positions<'a>(
    project: &'a Project,
    scene_ids: &[String],
) -> AppResult<Vec<(&'a Source, u64)>> {
    let scenes = scene_ids
        .iter()
        .map(|id| {
            project
                .scenes
                .iter()
                .find(|scene| &scene.id == id)
                .ok_or(AppError::NotFound)
        })
        .collect::<AppResult<Vec<_>>>()?;
    let mut result: Vec<(&Source, u64)> = Vec::with_capacity(MAX_SAMPLES);
    for slot in 0..MAX_SAMPLES {
        let scene = scenes[slot % scenes.len()];
        let pass = slot / scenes.len();
        let passes = MAX_SAMPLES.div_ceil(scenes.len());
        let source = source(project, &scene.source_id)?;
        let (start, end) = scene_range(scene)?;
        let length = end - start;
        let frame = start + (length * (2 * pass + 1) as u64) / (2 * passes as u64);
        if !result
            .iter()
            .any(|(existing, existing_frame)| existing.id == source.id && *existing_frame == frame)
        {
            result.push((source, frame));
        }
    }
    Ok(result)
}

fn spread(source: &Source, start: u64, end: u64, limit: usize) -> Vec<(&Source, u64)> {
    let length = end - start;
    let count = usize::try_from(length.min(limit as u64)).unwrap_or(limit);
    (0..count)
        .map(|index| {
            let frame = start + (length * (2 * index + 1) as u64) / (2 * count as u64);
            (source, frame)
        })
        .collect()
}

fn scene_range(scene: &Scene) -> AppResult<(u64, u64)> {
    Ok((frame(&scene.in_frame)?, frame(&scene.out_frame)?))
}

fn source<'a>(project: &'a Project, id: &str) -> AppResult<&'a Source> {
    project
        .sources
        .iter()
        .find(|source| source.id == id)
        .ok_or(AppError::NotFound)
}

fn frame(value: &str) -> AppResult<u64> {
    value
        .parse()
        .map_err(|_| AppError::Invalid("frame must be a decimal string".into()))
}

#[cfg(test)]
mod tests {
    use super::{FFMPEG_INPUT_POLICY, FFMPEG_METADATA_POLICY};

    #[test]
    fn ffmpeg_restricts_protocols_and_drops_metadata() {
        assert_eq!(FFMPEG_INPUT_POLICY, ["-protocol_whitelist", "file,pipe"]);
        assert_eq!(FFMPEG_METADATA_POLICY, ["-map_metadata", "-1"]);
    }
}
