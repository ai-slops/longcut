use std::path::Path as FsPath;

use axum::{
    Json,
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::Response,
};
use serde::Deserialize;
use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
};
use tokio_util::io::ReaderStream;

use crate::{
    AppState,
    error::{AppError, AppResult},
    media,
    model::{CreateRender, MediaStatus, Project, Render, RenderStatus, SourceRole},
    render,
};

#[derive(Deserialize)]
pub struct ImportQuery {
    role: Option<SourceRole>,
}

pub async fn import(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ImportQuery>,
    mut multipart: Multipart,
) -> AppResult<Json<Project>> {
    let mut upload = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|error| AppError::Invalid(error.to_string()))?
    {
        if field.name() != Some("file") {
            continue;
        }
        let original_name = field.file_name().unwrap_or("upload.bin").to_string();
        let extension = FsPath::new(&original_name)
            .extension()
            .and_then(|value| value.to_str())
            .filter(|value| value.chars().all(|ch| ch.is_ascii_alphanumeric()))
            .unwrap_or("bin");
        let artifact = format!("{}-original.{extension}", uuid::Uuid::new_v4());
        let temp = state.artifacts.join(format!(".{artifact}.upload"));
        let mut file = File::create(&temp).await?;
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|error| AppError::Invalid(error.to_string()))?
        {
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        drop(file);
        let final_path = state.artifacts.join(&artifact);
        fs::rename(&temp, &final_path).await?;
        upload = Some((original_name, artifact, final_path));
        break;
    }
    let (name, artifact, path) =
        upload.ok_or_else(|| AppError::Invalid("multipart field 'file' is required".into()))?;
    let project = state.store.get(&id).await?;
    let source = match media::probe_source(
        &path,
        &name,
        query.role.unwrap_or(SourceRole::Program),
        project.frame_rate,
        &artifact,
    )
    .await
    {
        Ok(source) => source,
        Err(error) => {
            let _ = fs::remove_file(path).await;
            return Err(error);
        }
    };
    let response = state.store.add_source(&id, source.clone()).await?;
    let worker_state = state.clone();
    tokio::spawn(async move {
        let updated = match media::derivatives(&worker_state.artifacts, &source).await {
            Ok(value) => value,
            Err(error) => {
                let mut failed = source;
                failed.status = MediaStatus::Error;
                failed.error = Some(error.to_string());
                failed
            }
        };
        if let Err(error) = worker_state.store.update_source(&id, updated).await {
            tracing::error!(error = ?error, "source.persist_failed");
        }
    });
    Ok(Json(response))
}

pub async fn demo(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<Project>> {
    let project = state.store.get(&id).await?;
    let (path, artifact) = media::generate_demo(&state.artifacts).await?;
    let source = media::probe_source(
        &path,
        "SYNTHETIC DEMO.mp4",
        SourceRole::Program,
        project.frame_rate,
        &artifact,
    )
    .await?;
    let response = state.store.add_source(&id, source.clone()).await?;
    let worker_state = state.clone();
    tokio::spawn(async move {
        let updated = match media::derivatives(&worker_state.artifacts, &source).await {
            Ok(value) => value,
            Err(error) => {
                let mut failed = source;
                failed.status = MediaStatus::Error;
                failed.error = Some(error.to_string());
                failed
            }
        };
        if let Err(error) = worker_state.store.update_source(&id, updated).await {
            tracing::error!(error = ?error, "source.persist_failed");
        }
    });
    Ok(Json(response))
}

pub async fn create_render(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<CreateRender>,
) -> AppResult<(StatusCode, Json<Render>)> {
    let project = state.store.get(&id).await?;
    if project.revision != input.base_revision {
        return Err(AppError::Stale);
    }
    if project.scenes.is_empty() {
        return Err(AppError::Invalid(
            "render requires at least one scene".into(),
        ));
    }
    let render = Render {
        id: uuid::Uuid::new_v4().to_string(),
        revision: project.revision.clone(),
        kind: input.kind,
        status: RenderStatus::Queued,
        progress: 0.0,
        media_url: None,
        srt_url: None,
        vtt_url: None,
        lineage_url: None,
        error: None,
    };
    let (queued, snapshot) = state
        .store
        .enqueue_render(&id, render, &input.base_revision)
        .await?;
    let worker_state = state.clone();
    let worker_render = queued.clone();
    tokio::spawn(async move {
        render::execute(
            worker_state.store,
            worker_state.artifacts.as_ref().clone(),
            snapshot,
            worker_render,
        )
        .await;
    });
    Ok((StatusCode::ACCEPTED, Json(queued)))
}

pub async fn serve_artifact(
    State(state): State<AppState>,
    Path(artifact): Path<String>,
    headers: HeaderMap,
) -> AppResult<Response> {
    if artifact.is_empty()
        || artifact.starts_with('.')
        || artifact.contains('/')
        || artifact.contains("..")
    {
        return Err(AppError::NotFound);
    }
    let path = state.artifacts.join(&artifact);
    let mut file = File::open(&path).await.map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            AppError::NotFound
        } else {
            error.into()
        }
    })?;
    let size = file.metadata().await?.len();
    let requested = headers.get(header::RANGE);
    let (start, end, status) = match requested {
        Some(value) => match value
            .to_str()
            .ok()
            .and_then(|value| parse_range(value, size))
        {
            Some((start, end)) => (start, end, StatusCode::PARTIAL_CONTENT),
            None => {
                return Response::builder()
                    .status(StatusCode::RANGE_NOT_SATISFIABLE)
                    .header(header::CONTENT_RANGE, format!("bytes */{size}"))
                    .header(header::CONTENT_LENGTH, "0")
                    .body(Body::empty())
                    .map_err(|error| AppError::Internal(error.into()));
            }
        },
        None => (0, size.saturating_sub(1), StatusCode::OK),
    };
    let length = if size == 0 {
        0
    } else {
        end.checked_sub(start)
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| AppError::Invalid("artifact range overflow".into()))?
    };
    file.seek(std::io::SeekFrom::Start(start)).await?;
    let stream = ReaderStream::new(file.take(length));
    let mut response = Response::builder()
        .status(status)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, length.to_string())
        .header(
            header::CONTENT_TYPE,
            mime_guess::from_path(&path)
                .first_or_octet_stream()
                .as_ref(),
        );
    if status == StatusCode::PARTIAL_CONTENT {
        response = response.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{size}"));
    }
    response
        .body(Body::from_stream(stream))
        .map_err(|error| AppError::Internal(error.into()))
}

fn parse_range(value: &str, size: u64) -> Option<(u64, u64)> {
    let range = value.strip_prefix("bytes=")?;
    if range.contains(',') || size == 0 {
        return None;
    }
    let (start, end) = range.split_once('-')?;
    if start.is_empty() {
        let suffix = end.parse::<u64>().ok()?;
        if suffix == 0 {
            return None;
        }
        return Some((size.saturating_sub(suffix), size - 1));
    }
    let start = start.parse::<u64>().ok()?;
    let end = if end.is_empty() {
        size - 1
    } else {
        end.parse::<u64>().ok()?.min(size - 1)
    };
    if start >= size || start > end {
        return None;
    }
    Some((start, end))
}
