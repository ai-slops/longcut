use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use error::AppResult;
use store::Store;

pub mod ai;
pub mod asr;
pub mod domain;
pub mod error;
pub mod http;
pub mod media;
mod media_http;
pub mod model;
mod ordinary_session;
pub mod render;
mod session_import;
pub mod store;
pub mod sync_diagnostics;
pub mod transcript;

#[derive(Clone)]
pub struct AppState {
    pub store: Store,
    pub artifacts: Arc<PathBuf>,
    pub codex: Arc<dyn ai::CodexClient>,
    pub openai: ai::OpenAiProvider,
    pub sessions: Arc<session_import::Service>,
    pub asr: Arc<asr::Service>,
    ai_cancellations: Arc<tokio::sync::Mutex<HashMap<String, tokio_util::sync::CancellationToken>>>,
}

impl AppState {
    pub async fn initialize(data: &Path) -> AppResult<Self> {
        Self::initialize_with_providers(
            data,
            Arc::new(ai::HttpCodexClient::from_env()?),
            ai::OpenAiProvider::from_env(),
        )
        .await
    }

    pub async fn initialize_with_codex(
        data: &Path,
        codex: Arc<dyn ai::CodexClient>,
    ) -> AppResult<Self> {
        Self::initialize_with_providers(data, codex, ai::OpenAiProvider::disabled()).await
    }

    pub async fn initialize_with_providers(
        data: &Path,
        codex: Arc<dyn ai::CodexClient>,
        openai: ai::OpenAiProvider,
    ) -> AppResult<Self> {
        Self::initialize_with_all(data, codex, openai, asr::AsrConfig::from_env()).await
    }

    pub async fn initialize_with_asr_config(
        data: &Path,
        config: asr::AsrConfig,
    ) -> AppResult<Self> {
        Self::initialize_with_all(
            data,
            Arc::new(ai::HttpCodexClient::from_env()?),
            ai::OpenAiProvider::disabled(),
            config,
        )
        .await
    }

    async fn initialize_with_all(
        data: &Path,
        codex: Arc<dyn ai::CodexClient>,
        openai: ai::OpenAiProvider,
        asr_config: asr::AsrConfig,
    ) -> AppResult<Self> {
        let artifacts = data.join("artifacts");
        tokio::fs::create_dir_all(&artifacts).await?;
        let store = Store::open(&data.join("longcut.sqlite3")).await?;
        let sessions =
            Arc::new(session_import::Service::initialize(data, &store, &artifacts).await?);
        let asr = asr::Service::initialize(data, asr_config).await?;
        Ok(Self {
            store,
            artifacts: Arc::new(artifacts),
            codex,
            openai,
            sessions,
            asr,
            ai_cancellations: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        })
    }

    pub(crate) async fn register_ai_work(
        &self,
        run_id: &str,
    ) -> tokio_util::sync::CancellationToken {
        let token = tokio_util::sync::CancellationToken::new();
        self.ai_cancellations
            .lock()
            .await
            .insert(run_id.into(), token.clone());
        token
    }

    pub(crate) async fn cancel_ai_work(&self, run_id: &str) {
        if let Some(token) = self.ai_cancellations.lock().await.get(run_id) {
            token.cancel();
        }
    }

    pub(crate) async fn finish_ai_work(&self, run_id: &str) {
        self.ai_cancellations.lock().await.remove(run_id);
    }
}

pub fn app(state: AppState) -> axum::Router {
    http::router(state)
}
