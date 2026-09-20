use std::{path::PathBuf, time::Duration};

use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::{Method, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::time::{Instant, sleep};

use super::{AiProposal, CodexStatus, EvidenceSample};
use crate::error::{AppError, AppResult};

#[derive(Clone, Copy)]
pub enum AccountAction {
    Login,
    CancelLogin,
    Logout,
}

#[async_trait]
pub trait CodexClient: Send + Sync {
    async fn status(&self) -> AppResult<CodexStatus>;
    async fn account(&self, action: AccountAction) -> AppResult<CodexStatus>;
    async fn generate(
        &self,
        id: &str,
        prompt: &str,
        schema: serde_json::Value,
        samples: &[EvidenceSample],
    ) -> AppResult<AiProposal>;
    async fn cancel(&self, id: &str) -> AppResult<()>;
}

#[derive(Clone)]
pub struct HttpCodexClient {
    base_url: String,
    token_path: PathBuf,
    client: reqwest::Client,
}

impl HttpCodexClient {
    pub fn from_env() -> AppResult<Self> {
        let base_url =
            std::env::var("LONGCUT_CODEX_URL").unwrap_or_else(|_| "http://codex:4100".into());
        let token_path = std::env::var_os("LONGCUT_CODEX_TOKEN_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/run/codex-link/token"));
        Self::new(base_url, token_path)
    }

    pub fn new(base_url: impl Into<String>, token_path: impl Into<PathBuf>) -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| AppError::Internal(error.into()))?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token_path: token_path.into(),
            client,
        })
    }

    async fn token(&self) -> AppResult<String> {
        let value = tokio::fs::read_to_string(&self.token_path)
            .await
            .map_err(|_| {
                AppError::Conflict("Codex sidecar authentication is unavailable".into())
            })?;
        let token = value.trim();
        if token.is_empty() {
            return Err(AppError::Conflict(
                "Codex sidecar authentication is unavailable".into(),
            ));
        }
        Ok(token.into())
    }

    async fn request<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
    ) -> AppResult<T> {
        let response = self
            .client
            .request(method, format!("{}{path}", self.base_url))
            .bearer_auth(self.token().await?)
            .send()
            .await
            .map_err(sidecar_unavailable)?;
        decode(response).await
    }
}

#[async_trait]
impl CodexClient for HttpCodexClient {
    async fn status(&self) -> AppResult<CodexStatus> {
        self.request(Method::GET, "/status").await
    }

    async fn account(&self, action: AccountAction) -> AppResult<CodexStatus> {
        let path = match action {
            AccountAction::Login => "/login",
            AccountAction::CancelLogin => "/login/cancel",
            AccountAction::Logout => "/logout",
        };
        self.request(Method::POST, path).await
    }

    async fn generate(
        &self,
        id: &str,
        prompt: &str,
        schema: serde_json::Value,
        samples: &[EvidenceSample],
    ) -> AppResult<AiProposal> {
        let body = JobRequest {
            id,
            prompt,
            schema,
            images: samples
                .iter()
                .map(|sample| STANDARD.encode(&sample.jpeg))
                .collect(),
        };
        let response = self
            .client
            .post(format!("{}/jobs", self.base_url))
            .bearer_auth(self.token().await?)
            .json(&body)
            .send()
            .await
            .map_err(sidecar_unavailable)?;
        let mut job: Job = decode(response).await?;
        let deadline = Instant::now() + Duration::from_secs(10 * 60);
        loop {
            match job.status {
                JobStatus::Succeeded => {
                    return job.result.ok_or_else(|| {
                        AppError::Conflict("Codex job succeeded without a proposal".into())
                    });
                }
                JobStatus::Failed => {
                    return Err(AppError::Conflict(
                        job.error.unwrap_or_else(|| "Codex job failed".into()),
                    ));
                }
                JobStatus::Cancelled => {
                    return Err(AppError::Conflict("Codex job was cancelled".into()));
                }
                JobStatus::Running => {}
            }
            if Instant::now() >= deadline {
                let _ = self.cancel(id).await;
                return Err(AppError::Conflict("Codex job timed out".into()));
            }
            sleep(Duration::from_millis(500)).await;
            job = self.request(Method::GET, &format!("/jobs/{id}")).await?;
        }
    }

    async fn cancel(&self, id: &str) -> AppResult<()> {
        let _: Job = self
            .request(Method::POST, &format!("/jobs/{id}/cancel"))
            .await?;
        Ok(())
    }
}

#[derive(Serialize)]
struct JobRequest<'a> {
    id: &'a str,
    prompt: &'a str,
    schema: serde_json::Value,
    images: Vec<String>,
}

#[derive(Deserialize)]
struct Job {
    #[serde(rename = "id")]
    _id: String,
    status: JobStatus,
    result: Option<AiProposal>,
    error: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum JobStatus {
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Deserialize)]
struct ErrorEnvelope {
    error: PublicError,
}

#[derive(Deserialize)]
struct PublicError {
    code: String,
    message: String,
}

async fn decode<T: serde::de::DeserializeOwned>(mut response: reqwest::Response) -> AppResult<T> {
    let status = response.status();
    if !status.is_success() {
        const MAX_ERROR_BYTES: usize = 4096;
        let mut body = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| sidecar_unavailable_error())?
        {
            if body.len() + chunk.len() > MAX_ERROR_BYTES {
                return Err(AppError::Conflict(format!(
                    "Codex sidecar returned {}",
                    public_status(status)
                )));
            }
            body.extend_from_slice(&chunk);
        }
        if let Ok(envelope) = serde_json::from_slice::<ErrorEnvelope>(&body) {
            let code = bounded_public_text(&envelope.error.code, 64);
            let message = bounded_public_text(&envelope.error.message, 512);
            if !code.is_empty() && !message.is_empty() {
                return Err(AppError::Conflict(format!("{code}: {message}")));
            }
        }
        return Err(AppError::Conflict(format!(
            "Codex sidecar returned {}",
            public_status(status)
        )));
    }
    response
        .json()
        .await
        .map_err(|error| AppError::Internal(error.into()))
}

fn sidecar_unavailable(_error: reqwest::Error) -> AppError {
    sidecar_unavailable_error()
}

fn sidecar_unavailable_error() -> AppError {
    AppError::Conflict("Codex sidecar is unavailable".into())
}

fn bounded_public_text(value: &str, limit: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(limit)
        .collect()
}

const fn public_status(status: StatusCode) -> u16 {
    status.as_u16()
}
