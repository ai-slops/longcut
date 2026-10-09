mod plan;
mod probe;
#[cfg(test)]
mod tests;
mod workspace;

use longcut_session_contract::model::Source as PackageSource;
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command};
use tokio_util::sync::CancellationToken;

use crate::{
    error::{AppError, AppResult},
    media,
    model::{EditorSourceMapping, Source},
};
use probe::{verify_origin, verify_source_start};
use workspace::AttemptWorkspace;

struct Adaptation<'a> {
    context: &'a super::prepare::Context<'a>,
    source: &'a PackageSource,
    plan: plan::Plan,
}

pub(super) async fn build(
    context: &super::prepare::Context<'_>,
    source: &PackageSource,
) -> AppResult<(Source, EditorSourceMapping)> {
    let plan = plan::build(context.package, source, context.frame_rate)?;
    let video = plan
        .bindings
        .first()
        .ok_or_else(|| AppError::Invalid("adapter plan has no video binding".into()))?;
    let asset = context
        .package
        .registry
        .assets
        .iter()
        .find(|asset| asset.asset_id == video.asset_id)
        .ok_or(AppError::NotFound)?;
    let video_mapping = plan
        .mappings
        .first()
        .ok_or_else(|| AppError::Invalid("adapter plan has no video mapping".into()))?;
    verify_source_start(
        &context.root.join(&asset.uri),
        video.stream_index,
        &video_mapping.media_start,
    )
    .await?;
    let attempt = AttemptWorkspace::create(context.work).await?;
    let result = Adaptation {
        context,
        source,
        plan,
    }
    .build_in(attempt.path())
    .await;
    match result {
        Ok(value) => {
            attempt.publish(context.work, &value.0).await?;
            Ok(value)
        }
        Err(error) => {
            attempt.cleanup().await?;
            Err(error)
        }
    }
}

impl Adaptation<'_> {
    async fn build_in(self, work: &Path) -> AppResult<(Source, EditorSourceMapping)> {
        let name = format!("{}-session-source.mp4", uuid::Uuid::new_v4());
        let path = work.join(&name);
        let mut command = Command::new("ffmpeg");
        command.args(["-v", "error", "-nostdin", "-copyts"]);
        for binding in &self.plan.bindings {
            let asset = self
                .context
                .package
                .registry
                .assets
                .iter()
                .find(|asset| asset.asset_id == binding.asset_id)
                .ok_or(AppError::NotFound)?;
            command
                .args([
                    "-itsoffset",
                    &self.plan.input_offset,
                    "-protocol_whitelist",
                    "file",
                    "-format_whitelist",
                    "mov,matroska,webm,avi,wav,flac,mp3,ogg,aac,mpegts",
                    "-i",
                ])
                .arg(self.context.root.join(&asset.uri));
        }
        for (index, binding) in self.plan.bindings.iter().enumerate() {
            command.args(["-map", &format!("{index}:{}", binding.stream_index)]);
        }
        command
            .args([
                "-t",
                &self.plan.duration,
                "-c",
                "copy",
                "-avoid_negative_ts",
                "disabled",
                "-map_metadata",
                "-1",
                "-movflags",
                "+faststart",
                "-y",
            ])
            .arg(&path);
        run(command, self.context.token).await?;
        verify_origin(&path, self.plan.bindings.len()).await?;
        let mut editor = media::probe_source(
            &path,
            &self.source.display_name,
            self.plan.role,
            self.context.frame_rate,
            &name,
        )
        .await?;
        editor.duration_frames = self.plan.frames;
        editor = media::derivatives_with_cancel(work, &editor, self.context.token).await?;
        let mapping = EditorSourceMapping {
            editor_source_id: editor.id.clone(),
            package_source_id: self.source.source_id.0.clone(),
            bindings: self.plan.mappings,
            session_range: self.plan.session_range,
        };
        Ok((editor, mapping))
    }
}

async fn run(mut command: Command, token: &CancellationToken) -> AppResult<()> {
    let mut child = command
        .kill_on_drop(true)
        .env_remove("FFREPORT")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("adapter stderr unavailable")))?;
    let work = async {
        let mut bytes = Vec::new();
        let read = async {
            stderr.take(64 * 1024 + 1).read_to_end(&mut bytes).await?;
            if bytes.len() > 64 * 1024 {
                return Err(std::io::Error::other("adapter output exceeded limit"));
            }
            Ok::<(), std::io::Error>(())
        };
        let (_, status) = tokio::try_join!(read, child.wait())?;
        if !status.success() {
            return Err(AppError::Invalid(format!(
                "lossless editor adapter unavailable: {}",
                String::from_utf8_lossy(&bytes)
            )));
        }
        Ok::<(), AppError>(())
    };
    let result = tokio::select! {
        _ = token.cancelled() => Err(AppError::Conflict("import cancelled".into())),
        result = tokio::time::timeout(Duration::from_secs(1800), work) => match result { Ok(result) => result, Err(_) => Err(AppError::Invalid("adapter timed out".into())) },
    };
    if result.is_err() {
        child.kill().await?;
    }
    result
}
