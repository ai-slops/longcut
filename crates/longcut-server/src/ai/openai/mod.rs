mod client;
mod config;
mod response;

use std::collections::HashMap;

use super::{AiProposal, EvidenceSample, ProviderStatus};
use crate::error::{AppError, AppResult};
use client::OpenAiClient;
use config::OpenAiConfig;

#[derive(Clone)]
pub struct OpenAiProvider {
    client: Option<OpenAiClient>,
    status: ProviderStatus,
}

impl OpenAiProvider {
    pub fn from_env() -> Self {
        Self::from_values(
            config::VARIABLES
                .into_iter()
                .filter_map(|name| std::env::var(name).ok().map(|value| (name, value))),
        )
    }

    pub fn from_values<I, K, V>(values: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: Into<String>,
    {
        let values = values
            .into_iter()
            .map(|(key, value)| (key.as_ref().to_owned(), value.into()))
            .collect::<HashMap<_, _>>();
        match OpenAiConfig::parse(&values) {
            Ok(config) => {
                let status = config.status(true, None);
                match OpenAiClient::new(config) {
                    Ok(client) => Self {
                        client: Some(client),
                        status,
                    },
                    Err(()) => Self {
                        client: None,
                        status: ProviderStatus {
                            configured: false,
                            detail: Some(
                                "OpenAI-compatible HTTP client could not be initialized".into(),
                            ),
                            ..status
                        },
                    },
                }
            }
            Err(status) => Self {
                client: None,
                status,
            },
        }
    }

    pub fn disabled() -> Self {
        Self::from_values(std::iter::empty::<(&str, String)>())
    }

    pub const fn configured(&self) -> bool {
        self.client.is_some()
    }

    pub const fn status(&self) -> &ProviderStatus {
        &self.status
    }

    pub async fn generate(
        &self,
        prompt: &str,
        schema: serde_json::Value,
        samples: &[EvidenceSample],
    ) -> AppResult<AiProposal> {
        let client = self.client.as_ref().ok_or_else(|| {
            AppError::Conflict("OpenAI-compatible provider is not configured".into())
        })?;
        client.generate(prompt, schema, samples).await
    }
}
