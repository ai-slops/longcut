use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    fs::{self, File},
    io::AsyncReadExt,
};

#[derive(Debug, Error)]
pub(crate) enum FileError {
    #[error("asset inspection exceeded its deadline")]
    Timeout,
    #[error("asset inspection was cancelled")]
    Cancelled,
    #[error("media must be a regular file inside the package, without symlinks")]
    Unsafe,
    #[error("media changed during verification; inspect an immutable completed package")]
    Changed,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl FileError {
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Timeout => "ASSET_TIMEOUT",
            Self::Cancelled => "ASSET_CANCELLED",
            Self::Unsafe => "UNSAFE_MEDIA_PATH",
            Self::Changed => "MEDIA_CHANGED",
            Self::Io(error) if error.kind() == std::io::ErrorKind::NotFound => "MEDIA_MISSING",
            Self::Io(_) => "MEDIA_IO_ERROR",
        }
    }
}

pub(crate) async fn bounded<T>(
    operation: impl std::future::Future<Output = Result<T, FileError>>,
    deadline: tokio::time::Instant,
    cancellation: &tokio_util::sync::CancellationToken,
) -> Result<T, FileError> {
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => Err(FileError::Cancelled),
        result = tokio::time::timeout_at(deadline, operation) => result.map_err(|_| FileError::Timeout)?,
    }
}

pub(crate) struct Integrity {
    pub path: PathBuf,
    pub size: u64,
    pub sha256: String,
    pub modified: SystemTime,
}

pub(crate) async fn integrity(root: &Path, relative: &str) -> Result<Integrity, FileError> {
    let mut path = fs::canonicalize(root).await?;
    for part in relative.split('/') {
        if part.is_empty()
            || matches!(part, "." | "..")
            || part.contains(['\\', ':', '%', '?', '#'])
            || part.chars().any(char::is_control)
        {
            return Err(FileError::Unsafe);
        }
        path.push(part);
        if fs::symlink_metadata(&path).await?.file_type().is_symlink() {
            return Err(FileError::Unsafe);
        }
    }
    if !fs::metadata(&path).await?.is_file() {
        return Err(FileError::Unsafe);
    }
    let mut file = File::open(&path).await?;
    let metadata = file.metadata().await?;
    let modified = metadata.modified()?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0; 256 * 1024];
    let mut size = 0_u64;
    loop {
        let count = file.read(&mut buffer).await?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(u64::try_from(count).map_err(|_| FileError::Changed)?)
            .ok_or(FileError::Changed)?;
        if size > metadata.len() {
            return Err(FileError::Changed);
        }
        digest.update(&buffer[..count]);
    }
    let result = Integrity {
        path,
        size,
        modified,
        sha256: format!("{:x}", digest.finalize()),
    };
    if size != metadata.len() {
        return Err(FileError::Changed);
    }
    unchanged(&result).await?;
    Ok(result)
}

pub(crate) async fn unchanged(integrity: &Integrity) -> Result<(), FileError> {
    let metadata = fs::symlink_metadata(&integrity.path).await?;
    if !metadata.is_file()
        || metadata.len() != integrity.size
        || metadata.modified()? != integrity.modified
    {
        return Err(FileError::Changed);
    }
    Ok(())
}
