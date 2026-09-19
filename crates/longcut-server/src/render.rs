mod segment;
mod sidecar;

use std::path::{Path, PathBuf};

use tokio::{fs, process::Command};

use crate::{
    error::{AppError, AppResult},
    model::{Project, Render, RenderKind, RenderStatus},
    store::Store,
};

pub async fn execute(store: Store, artifacts: PathBuf, project: Project, mut render: Render) {
    render.status = RenderStatus::Running;
    render.progress = 0.05;
    if let Err(error) = store.update_render(&project.id, render.clone()).await {
        tracing::error!(render_id = %render.id, error = ?error, "render.state_failed");
        return;
    }
    match render_project(&artifacts, &project, &render).await {
        Ok(urls) => {
            render.status = RenderStatus::Ready;
            render.progress = 1.0;
            render.media_url = Some(urls.media);
            render.srt_url = Some(urls.srt);
            render.vtt_url = Some(urls.vtt);
            render.lineage_url = Some(urls.lineage);
            tracing::info!(render_id = %render.id, revision = %render.revision, "render.ready");
        }
        Err(error) => {
            tracing::error!(render_id = %render.id, error = ?error, "render.failed");
            render.status = RenderStatus::Error;
            render.error = Some(error.to_string());
        }
    }
    if let Err(error) = store.update_render(&project.id, render).await {
        tracing::error!(error = ?error, "render.persist_failed");
    }
}

struct Urls {
    media: String,
    srt: String,
    vtt: String,
    lineage: String,
}

async fn render_project(artifacts: &Path, project: &Project, render: &Render) -> AppResult<Urls> {
    if project.scenes.is_empty() {
        return Err(AppError::Invalid(
            "render requires at least one scene".into(),
        ));
    }
    let work = artifacts.join(format!(".render-{}", render.id));
    fs::create_dir_all(&work).await?;
    let result = render_inner(artifacts, &work, project, render).await;
    if let Err(error) = fs::remove_dir_all(&work).await {
        tracing::warn!(error = ?error, "render.cleanup_failed");
    }
    result
}

async fn render_inner(
    artifacts: &Path,
    work: &Path,
    project: &Project,
    render: &Render,
) -> AppResult<Urls> {
    let mut segments = Vec::with_capacity(project.scenes.len());
    for (index, scene) in project.scenes.iter().enumerate() {
        let path = work.join(format!("segment-{index:04}.mp4"));
        segment::render(artifacts, project, scene, &path).await?;
        segments.push(path);
    }
    let list = work.join("concat.txt");
    fs::write(
        &list,
        segments
            .iter()
            .map(|path| format!("file '{}'\n", path.display()))
            .collect::<String>(),
    )
    .await?;
    let suffix = match render.kind {
        RenderKind::Preview => "preview",
        RenderKind::Final => "final",
    };
    let media_name = format!("{}-{suffix}.mp4", render.id);
    let temp = artifacts.join(format!(".{media_name}.tmp.mp4"));
    segment::run(
        Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "concat",
                "-safe",
                "0",
                "-i",
            ])
            .arg(&list)
            .args(["-c", "copy", "-movflags", "+faststart", "-y"])
            .arg(&temp),
        "concat",
    )
    .await?;
    fs::rename(&temp, artifacts.join(&media_name)).await?;
    let sidecars = sidecar::write(artifacts, project, &render.id).await?;
    Ok(Urls {
        media: url(&media_name),
        srt: url(&sidecars.srt),
        vtt: url(&sidecars.vtt),
        lineage: url(&sidecars.lineage),
    })
}

fn url(name: &str) -> String {
    format!("/api/v1/media/{name}")
}
