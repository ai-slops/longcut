use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use error::AppResult;
use store::Store;

pub mod domain;
pub mod error;
pub mod http;
pub mod media;
mod media_http;
pub mod model;
pub mod render;
pub mod store;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub artifacts: Arc<PathBuf>,
}

impl AppState {
    pub async fn initialize(data: &Path) -> AppResult<Self> {
        let artifacts = data.join("artifacts");
        tokio::fs::create_dir_all(&artifacts).await?;
        let store = Store::open(&data.join("longcut.sqlite3")).await?;
        Ok(Self {
            store,
            artifacts: Arc::new(artifacts),
        })
    }
}

pub fn app(state: AppState) -> axum::Router {
    http::router(state)
}
