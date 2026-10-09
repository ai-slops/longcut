use std::path::{Path, PathBuf};

use tokio::fs;

use crate::{
    error::{AppError, AppResult},
    media,
    model::Source,
};

pub(super) struct AttemptWorkspace {
    path: PathBuf,
}

impl AttemptWorkspace {
    pub(super) async fn create(parent: &Path) -> AppResult<Self> {
        let path = parent.join(format!(".adapter-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).await?;
        Ok(Self { path })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) async fn cleanup(self) -> AppResult<()> {
        remove_directory(&self.path).await
    }

    pub(super) async fn publish(self, destination: &Path, source: &Source) -> AppResult<()> {
        let mut names = vec![media::artifact_name(&source.media_url)?];
        names.extend(
            [source.proxy_url.as_deref(), source.thumbnail_url.as_deref()]
                .into_iter()
                .flatten()
                .map(media::artifact_name)
                .collect::<AppResult<Vec<_>>>()?,
        );
        let mut moved = Vec::with_capacity(names.len());
        for name in names {
            let target = destination.join(name);
            if let Err(error) = fs::rename(self.path.join(name), &target).await {
                rollback(&moved, &self.path).await?;
                return Err(error.into());
            }
            moved.push(target);
        }
        if let Err(error) = fs::remove_dir(&self.path).await {
            rollback(&moved, &self.path).await?;
            return Err(error.into());
        }
        Ok(())
    }
}

async fn rollback(moved: &[PathBuf], attempt: &Path) -> AppResult<()> {
    for path in moved {
        remove_file(path).await?;
    }
    remove_directory(attempt).await
}

async fn remove_file(path: &Path) -> AppResult<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

async fn remove_directory(path: &Path) -> AppResult<()> {
    match fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::Internal(error.into())),
    }
}
