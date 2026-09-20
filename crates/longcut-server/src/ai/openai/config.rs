use std::collections::HashMap;

use reqwest::Url;

use crate::ai::{AiProvider, ProviderStatus};

pub(super) const VARIABLES: [&str; 6] = [
    "OPENAI_API_KEY",
    "OPENAI_BASE_URL",
    "OPENAI_MODEL",
    "OPENAI_RESPONSE_FORMAT",
    "OPENAI_MAX_OUTPUT_TOKENS",
    "OPENAI_TOKEN_PARAMETER",
];

pub(super) struct OpenAiConfig {
    pub endpoint: Url,
    pub model: String,
    pub key: SecretKey,
    pub response_format: ResponseFormat,
    pub max_output_tokens: u32,
    pub token_parameter: TokenParameter,
}

pub(super) struct SecretKey(String);

impl SecretKey {
    pub(super) fn expose(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy)]
pub(super) enum ResponseFormat {
    JsonSchema,
    JsonObject,
    None,
}

#[derive(Clone, Copy)]
pub(super) enum TokenParameter {
    MaxCompletionTokens,
    MaxTokens,
}

impl OpenAiConfig {
    pub(super) fn parse(values: &HashMap<String, String>) -> Result<Self, ProviderStatus> {
        let base = value(values, "OPENAI_BASE_URL").unwrap_or("https://api.openai.com/v1");
        let endpoint = endpoint(base).map_err(|detail| disabled(None, None, detail))?;
        let model = value(values, "OPENAI_MODEL").filter(|value| !value.trim().is_empty());
        let public_model = model.map(str::to_owned);
        let key = value(values, "OPENAI_API_KEY").filter(|value| !value.trim().is_empty());
        if model.is_none() || key.is_none() {
            return Err(disabled(
                public_model,
                Some(endpoint.to_string()),
                "Set non-empty OPENAI_API_KEY and OPENAI_MODEL server environment variables",
            ));
        }
        let model = model.unwrap_or_default();
        if model.chars().count() > 200 || model.chars().any(char::is_control) {
            return Err(disabled(
                None,
                Some(endpoint.to_string()),
                "OPENAI_MODEL must be a short single-line model identifier",
            ));
        }
        let response_format = match value(values, "OPENAI_RESPONSE_FORMAT").unwrap_or("json_schema")
        {
            "json_schema" => ResponseFormat::JsonSchema,
            "json_object" => ResponseFormat::JsonObject,
            "none" => ResponseFormat::None,
            _ => {
                return Err(disabled(
                    Some(model.to_owned()),
                    Some(endpoint.to_string()),
                    "OPENAI_RESPONSE_FORMAT must be json_schema, json_object, or none",
                ));
            }
        };
        let max_output_tokens = value(values, "OPENAI_MAX_OUTPUT_TOKENS")
            .unwrap_or("8192")
            .parse::<u32>()
            .ok()
            .filter(|value| (256..=32_768).contains(value))
            .ok_or_else(|| {
                disabled(
                    Some(model.to_owned()),
                    Some(endpoint.to_string()),
                    "OPENAI_MAX_OUTPUT_TOKENS must be an integer from 256 through 32768",
                )
            })?;
        let token_parameter =
            match value(values, "OPENAI_TOKEN_PARAMETER").unwrap_or("max_completion_tokens") {
                "max_completion_tokens" => TokenParameter::MaxCompletionTokens,
                "max_tokens" => TokenParameter::MaxTokens,
                _ => {
                    return Err(disabled(
                        Some(model.to_owned()),
                        Some(endpoint.to_string()),
                        "OPENAI_TOKEN_PARAMETER must be max_completion_tokens or max_tokens",
                    ));
                }
            };
        Ok(Self {
            endpoint,
            model: model.to_owned(),
            key: SecretKey(key.unwrap_or_default().to_owned()),
            response_format,
            max_output_tokens,
            token_parameter,
        })
    }

    pub(super) fn status(&self, configured: bool, detail: Option<String>) -> ProviderStatus {
        ProviderStatus {
            id: AiProvider::OpenaiCompatible,
            name: "OpenAI-compatible API".into(),
            configured,
            model: Some(self.model.clone()),
            endpoint: Some(self.endpoint.to_string()),
            detail,
        }
    }
}

fn value<'a>(values: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    values.get(name).map(String::as_str)
}

fn endpoint(value: &str) -> Result<Url, &'static str> {
    let mut url = Url::parse(value).map_err(|_| "OPENAI_BASE_URL must be a valid HTTP(S) URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
    {
        return Err("OPENAI_BASE_URL must be HTTP(S) without credentials, query, or fragment");
    }
    let path = format!("{}/chat/completions", url.path().trim_end_matches('/'));
    url.set_path(&path);
    Ok(url)
}

fn disabled(model: Option<String>, endpoint: Option<String>, detail: &str) -> ProviderStatus {
    ProviderStatus {
        id: AiProvider::OpenaiCompatible,
        name: "OpenAI-compatible API".into(),
        configured: false,
        model,
        endpoint,
        detail: Some(detail.into()),
    }
}
