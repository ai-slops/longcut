mod adapter;
mod http;
mod prepare;
mod publication;

use crate::{error::AppResult, model::ImportStatus, store::Store};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::sync::{Mutex, Semaphore};
use tokio_util::sync::CancellationToken;

pub use http::{asset, cancel, confirm, get, list, render_session, session, upload};
pub(crate) use publication::confirm as confirm_prepared;

pub struct Service {
    pub root: PathBuf,
    pub packages: PathBuf,
    pub tokens: Mutex<HashMap<String, CancellationToken>>,
    pub capacity: Semaphore,
}

impl Service {
    pub async fn initialize(data: &Path, store: &Store, artifacts: &Path) -> AppResult<Self> {
        let service = Self {
            root: data.join("session-imports"),
            packages: data.join("source-sessions"),
            tokens: Mutex::new(HashMap::new()),
            capacity: Semaphore::new(2),
        };
        tokio::fs::create_dir_all(&service.root).await?;
        tokio::fs::create_dir_all(&service.packages).await?;
        for job in store.recovery_imports().await? {
            match job.status {
                ImportStatus::Prepared => {}
                ImportStatus::Confirmed => {
                    publication::remove_directory(&service.root.join(&job.id)).await?
                }
                ImportStatus::Cancelled | ImportStatus::Failed => {
                    publication::cleanup(&service, artifacts, store, &job).await?
                }
                ImportStatus::Validating | ImportStatus::Publishing => {
                    publication::cleanup(&service, artifacts, store, &job).await?;
                    store
                        .transition_import(
                            &job.project_id,
                            &job.id,
                            ImportStatus::Failed,
                            Some("import was interrupted; upload the package again".into()),
                        )
                        .await?;
                }
            }
        }
        Ok(service)
    }
}
