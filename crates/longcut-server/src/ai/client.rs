use std::{path::PathBuf, time::Duration};

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::{Method, StatusCode};
use serde::{Serialize, de::DeserializeOwned};
use tokio::time::{Instant, sleep};

use super::{
    AiProposal, AiProvider, CodexStatus, EvidenceSample, ProvidersResponse, RuntimeInfo,
    RuntimeJob, RuntimeJobStatus,
    runtime_contract::{invalid_runtime_response, validate_runtime_info, validate_runtime_job},
};
use crate::error::{AppError, AppResult};

const MAX_RESPONSE_BYTES: usize = 1_048_576;
const MAX_ERROR_BYTES: usize = 4096;

#[derive(Clone, Copy)]
pub enum AccountAction {
    Login,
    CancelLogin,
    Logout,
}

#[async_trait]
pub trait AgentRuntimeClient: Send + Sync {
    async fn codex_status(&self) -> AppResult<CodexStatus>;
    async fn account(&self, action: AccountAction) -> AppResult<CodexStatus>;
    async fn providers(&self) -> AppResult<ProvidersResponse>;
    async fn runtime_info(&self) -> AppResult<RuntimeInfo>;
    async fn job_runtime(&self, id: &str) -> AppResult<RuntimeJob>;
    async fn job_runtime_for(&self, provider: AiProvider, id: &str) -> AppResult<RuntimeJob> {
        let job = self.job_runtime(id).await?;
        validate_runtime_job(job, id, Some(provider), None, false)
    }
    async fn generate(
        &self,
        provider: AiProvider,
        id: &str,
        prompt: &str,
        schema: serde_json::Value,
        samples: &[EvidenceSample],
    ) -> AppResult<AiProposal>;
    async fn cancel(&self, id: &str) -> AppResult<()>;
    async fn cancel_for(&self, _provider: AiProvider, id: &str) -> AppResult<()> {
        self.cancel(id).await
    }
}

#[derive(Clone)]
pub struct HttpAgentRuntimeClient {
    base_url: String,
    token_path: PathBuf,
    client: reqwest::Client,
}

impl HttpAgentRuntimeClient {
    pub fn from_env() -> AppResult<Self> {
        let base_url = std::env::var("LONGCUT_AGENT_RUNTIME_URL")
            .or_else(|_| std::env::var("LONGCUT_CODEX_URL"))
            .unwrap_or_else(|_| "http://agent-runtime:4100".into());
        let token_path = std::env::var_os("LONGCUT_AGENT_RUNTIME_TOKEN_FILE")
            .or_else(|| std::env::var_os("LONGCUT_CODEX_TOKEN_FILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/run/codex-link/token"));
        Self::new(base_url, token_path)
    }

    pub fn new(base_url: impl Into<String>, token_path: impl Into<PathBuf>) -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| AppError::Internal(error.into()))?;
        Ok(Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            token_path: token_path.into(),
            client,
        })
    }

    async fn token(&self) -> AppResult<String> {
        let value = tokio::fs::read_to_string(&self.token_path)
            .await
            .map_err(|_| runtime_auth_unavailable())?;
        let token = value.trim();
        if token.is_empty() {
            return Err(runtime_auth_unavailable());
        }
        Ok(token.into())
    }

    async fn request<T: DeserializeOwned>(&self, method: Method, path: &str) -> AppResult<T> {
        let response = self
            .client
            .request(method, format!("{}{path}", self.base_url))
            .bearer_auth(self.token().await?)
            .send()
            .await
            .map_err(runtime_unavailable)?;
        decode(response).await
    }

    async fn cancel_job(&self, provider: Option<AiProvider>, id: &str) -> AppResult<()> {
        let response = self
            .client
            .post(format!("{}/jobs/{id}/cancel", self.base_url))
            .bearer_auth(self.token().await?)
            .send()
            .await
            .map_err(runtime_unavailable)?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(());
        }
        let job = decode(response).await?;
        validate_runtime_job(job, id, provider, None, true)?;
        Ok(())
    }
}

struct RuntimeJobLease {
    client: HttpAgentRuntimeClient,
    provider: AiProvider,
    id: String,
    armed: bool,
}

impl RuntimeJobLease {
    fn new(client: &HttpAgentRuntimeClient, provider: AiProvider, id: &str) -> Self {
        Self {
            client: client.clone(),
            provider,
            id: id.to_owned(),
            armed: true,
        }
    }

    fn finish(&mut self) {
        self.armed = false;
    }

    async fn cancel(&mut self) -> AppResult<()> {
        self.client
            .cancel_job(Some(self.provider), &self.id)
            .await?;
        self.finish();
        Ok(())
    }
}

impl Drop for RuntimeJobLease {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let client = self.client.clone();
        let provider = self.provider;
        let id = self.id.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _result = client.cancel_job(Some(provider), &id).await;
            });
        }
    }
}

#[async_trait]
impl AgentRuntimeClient for HttpAgentRuntimeClient {
    async fn codex_status(&self) -> AppResult<CodexStatus> {
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

    async fn providers(&self) -> AppResult<ProvidersResponse> {
        self.request(Method::GET, "/providers").await
    }

    async fn runtime_info(&self) -> AppResult<RuntimeInfo> {
        validate_runtime_info(self.request(Method::GET, "/runtime").await?)
    }

    async fn job_runtime(&self, id: &str) -> AppResult<RuntimeJob> {
        let job = self.request(Method::GET, &format!("/jobs/{id}")).await?;
        validate_runtime_job(job, id, None, None, false)
    }

    async fn job_runtime_for(&self, provider: AiProvider, id: &str) -> AppResult<RuntimeJob> {
        let job = self.request(Method::GET, &format!("/jobs/{id}")).await?;
        validate_runtime_job(job, id, Some(provider), None, false)
    }

    async fn generate(
        &self,
        provider: AiProvider,
        id: &str,
        prompt: &str,
        schema: serde_json::Value,
        samples: &[EvidenceSample],
    ) -> AppResult<AiProposal> {
        let body = JobRequest {
            provider,
            id,
            prompt,
            schema,
            images: samples
                .iter()
                .map(|sample| STANDARD.encode(&sample.jpeg))
                .collect(),
        };
        let token = self.token().await?;
        let mut lease = RuntimeJobLease::new(self, provider, id);
        let response = self
            .client
            .post(format!("{}/jobs", self.base_url))
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(runtime_unavailable)?;
        let mut job =
            validate_runtime_job(decode(response).await?, id, Some(provider), None, false)?;
        let deadline = Instant::now() + Duration::from_secs(10 * 60);
        loop {
            match job.status {
                RuntimeJobStatus::Succeeded => {
                    lease.finish();
                    return job.result.ok_or_else(invalid_runtime_response);
                }
                RuntimeJobStatus::Failed => {
                    lease.finish();
                    return Err(runtime_job_error(job.error));
                }
                RuntimeJobStatus::Cancelled => {
                    lease.finish();
                    return Err(AppError::Conflict("Agent runtime job was cancelled".into()));
                }
                RuntimeJobStatus::Running => {}
            }
            if Instant::now() >= deadline {
                let _result = lease.cancel().await;
                return Err(AppError::Conflict("Agent runtime job timed out".into()));
            }
            sleep(Duration::from_millis(500)).await;
            let next = self.request(Method::GET, &format!("/jobs/{id}")).await?;
            job = validate_runtime_job(next, id, Some(provider), Some(&job), false)?;
        }
    }

    async fn cancel(&self, id: &str) -> AppResult<()> {
        self.cancel_job(None, id).await
    }

    async fn cancel_for(&self, provider: AiProvider, id: &str) -> AppResult<()> {
        self.cancel_job(Some(provider), id).await
    }
}

#[derive(Serialize)]
struct JobRequest<'a> {
    provider: AiProvider,
    id: &'a str,
    prompt: &'a str,
    schema: serde_json::Value,
    images: Vec<String>,
}

#[derive(serde::Deserialize)]
struct ErrorEnvelope {
    error: RuntimeErrorEnvelope,
}

#[derive(serde::Deserialize)]
struct RuntimeErrorEnvelope {
    code: String,
    message: String,
}

async fn decode<T: DeserializeOwned>(mut response: reqwest::Response) -> AppResult<T> {
    let status = response.status();
    let limit = if status.is_success() {
        MAX_RESPONSE_BYTES
    } else {
        MAX_ERROR_BYTES
    };
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(runtime_unavailable)? {
        if body.len() + chunk.len() > limit {
            return Err(runtime_status(status));
        }
        body.extend_from_slice(&chunk);
    }
    if !status.is_success() {
        if let Ok(envelope) = serde_json::from_slice::<ErrorEnvelope>(&body) {
            let code = bounded_public_text(&envelope.error.code, 64);
            let message = bounded_public_text(&envelope.error.message, 512);
            if !code.is_empty() && !message.is_empty() {
                return Err(AppError::Conflict(format!("{code}: {message}")));
            }
        }
        return Err(runtime_status(status));
    }
    serde_json::from_slice(&body)
        .map_err(|_| AppError::Conflict("Agent runtime returned an invalid response".into()))
}

fn runtime_job_error(error: Option<super::RuntimeError>) -> AppError {
    match error {
        Some(error) => {
            let code = bounded_public_text(&error.code, 64);
            let message = bounded_public_text(&error.message, 512);
            if code.is_empty() || message.is_empty() {
                AppError::Conflict("Agent runtime job failed".into())
            } else {
                AppError::Conflict(format!("{code}: {message}"))
            }
        }
        None => AppError::Conflict("Agent runtime job failed".into()),
    }
}

fn runtime_auth_unavailable() -> AppError {
    AppError::Conflict("Agent runtime authentication is unavailable".into())
}

fn runtime_unavailable(_error: reqwest::Error) -> AppError {
    AppError::Conflict("Agent runtime is unavailable".into())
}

fn runtime_status(status: StatusCode) -> AppError {
    AppError::Conflict(format!("Agent runtime returned {}", status.as_u16()))
}

fn bounded_public_text(value: &str, limit: usize) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(limit)
        .collect()
}
