use std::time::Duration;

use base64::{Engine, engine::general_purpose::STANDARD};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::Serialize;

use super::{
    config::{OpenAiConfig, ResponseFormat, TokenParameter},
    response,
};
use crate::{
    ai::{AiProposal, EvidenceSample},
    error::AppResult,
};

#[derive(Clone)]
pub(super) struct OpenAiClient {
    config: std::sync::Arc<OpenAiConfig>,
    client: reqwest::Client,
}

impl OpenAiClient {
    pub(super) fn new(config: OpenAiConfig) -> Result<Self, ()> {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, authorization_header(config.key.expose())?);
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(10 * 60))
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .build()
            .map_err(|_| ())?;
        Ok(Self {
            config: std::sync::Arc::new(config),
            client,
        })
    }

    pub(super) async fn generate(
        &self,
        prompt: &str,
        schema: serde_json::Value,
        samples: &[EvidenceSample],
    ) -> AppResult<AiProposal> {
        let mut content = Vec::with_capacity(samples.len() + 1);
        content.push(MessagePart::Text {
            text: prompt.to_owned(),
        });
        content.extend(samples.iter().map(|sample| MessagePart::ImageUrl {
            image_url: ImageUrl {
                url: format!("data:image/jpeg;base64,{}", STANDARD.encode(&sample.jpeg)),
            },
        }));
        let (max_completion_tokens, max_tokens) = match self.config.token_parameter {
            TokenParameter::MaxCompletionTokens => (Some(self.config.max_output_tokens), None),
            TokenParameter::MaxTokens => (None, Some(self.config.max_output_tokens)),
        };
        let response_format = match self.config.response_format {
            ResponseFormat::JsonSchema => Some(ResponseFormatBody::JsonSchema {
                json_schema: JsonSchema {
                    name: "ai_proposal",
                    strict: true,
                    schema,
                },
            }),
            ResponseFormat::JsonObject => Some(ResponseFormatBody::JsonObject),
            ResponseFormat::None => None,
        };
        let body = ChatRequest {
            model: &self.config.model,
            messages: [ChatMessage {
                role: "user",
                content,
            }],
            response_format,
            max_completion_tokens,
            max_tokens,
            stream: true,
        };
        let response = self
            .client
            .post(self.config.endpoint.clone())
            .json(&body)
            .send()
            .await
            .map_err(|_| response::unavailable())?;
        response::decode(response).await
    }
}

fn authorization_header(key: &str) -> Result<HeaderValue, ()> {
    let mut value = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| ())?;
    value.set_sensitive(true);
    Ok(value)
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: [ChatMessage; 1],
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormatBody>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    stream: bool,
}

#[derive(Serialize)]
struct ChatMessage {
    role: &'static str,
    content: Vec<MessagePart>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MessagePart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Serialize)]
struct ImageUrl {
    url: String,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ResponseFormatBody {
    JsonSchema { json_schema: JsonSchema },
    JsonObject,
}

#[derive(Serialize)]
struct JsonSchema {
    name: &'static str,
    strict: bool,
    schema: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::authorization_header;

    #[test]
    fn authorization_header_is_redacted_from_debug_output() {
        // Given
        let secret = "dummy-secret-for-test";
        // When
        let header = authorization_header(secret).expect("authorization header");
        let debug = format!("{header:?}");
        // Then
        assert!(header.is_sensitive());
        assert!(!debug.contains(secret));
    }
}
