use reqwest::{StatusCode, header::CONTENT_TYPE};
use serde::{Deserialize, de::IgnoredAny};

use crate::{
    ai::AiProposal,
    error::{AppError, AppResult},
};

const MAX_RESPONSE_BYTES: usize = 262_144;

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    #[serde(default)]
    error: Option<IgnoredAny>,
}

#[derive(Deserialize)]
struct Choice {
    finish_reason: String,
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<IgnoredAny>>,
}

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
    #[serde(default)]
    error: Option<IgnoredAny>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct StreamDelta {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<IgnoredAny>>,
}

#[derive(Default)]
struct StreamResult {
    content: String,
    stopped: bool,
    done: bool,
}

pub(super) async fn decode(mut response: reqwest::Response) -> AppResult<AiProposal> {
    let status = response.status();
    if !status.is_success() {
        return Err(provider_status(status));
    }
    let event_stream = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("text/event-stream"));
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| unavailable())? {
        if body.len() + chunk.len() > MAX_RESPONSE_BYTES {
            return Err(AppError::Conflict(
                "OpenAI-compatible provider response exceeded the output limit".into(),
            ));
        }
        body.extend_from_slice(&chunk);
    }
    if event_stream || body.starts_with(b"data:") {
        decode_stream(&body)
    } else {
        decode_json(&body)
    }
}

fn decode_json(body: &[u8]) -> AppResult<AiProposal> {
    let mut parsed: ChatResponse = serde_json::from_slice(body).map_err(|_| invalid_response())?;
    if parsed.error.is_some() {
        return Err(reported_error());
    }
    if parsed.choices.len() != 1 {
        return Err(invalid_response());
    }
    let choice = parsed.choices.pop().ok_or_else(invalid_response)?;
    if choice.finish_reason != "stop"
        || choice
            .message
            .tool_calls
            .is_some_and(|tool_calls| !tool_calls.is_empty())
    {
        return Err(incomplete_response());
    }
    decode_proposal(&choice.message.content)
}

fn decode_stream(body: &[u8]) -> AppResult<AiProposal> {
    let text = std::str::from_utf8(body).map_err(|_| invalid_response())?;
    let mut result = StreamResult::default();
    let mut event = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            accept_event(&event.join("\n"), &mut result)?;
            event.clear();
        } else if let Some(data) = line.strip_prefix("data:") {
            event.push(data.strip_prefix(' ').unwrap_or(data));
        }
    }
    if !event.is_empty() {
        accept_event(&event.join("\n"), &mut result)?;
    }
    if !result.done || !result.stopped {
        return Err(incomplete_response());
    }
    decode_proposal(&result.content)
}

fn accept_event(data: &str, result: &mut StreamResult) -> AppResult<()> {
    if data.is_empty() {
        return Ok(());
    }
    if data.trim() == "[DONE]" {
        result.done = true;
        return Ok(());
    }
    let chunk: StreamChunk = serde_json::from_str(data).map_err(|_| invalid_response())?;
    if chunk.error.is_some() {
        return Err(reported_error());
    }
    if chunk.choices.len() > 1 {
        return Err(invalid_response());
    }
    if let Some(choice) = chunk.choices.into_iter().next() {
        if choice
            .delta
            .tool_calls
            .is_some_and(|tool_calls| !tool_calls.is_empty())
        {
            return Err(incomplete_response());
        }
        if let Some(content) = choice.delta.content {
            result.content.push_str(&content);
        }
        if let Some(reason) = choice.finish_reason {
            if reason != "stop" {
                return Err(incomplete_response());
            }
            result.stopped = true;
        }
    }
    Ok(())
}

fn decode_proposal(content: &str) -> AppResult<AiProposal> {
    serde_json::from_str(content).map_err(|_| {
        AppError::Conflict("OpenAI-compatible provider returned an invalid JSON proposal".into())
    })
}

fn provider_status(status: StatusCode) -> AppError {
    AppError::Conflict(format!(
        "OpenAI-compatible provider returned HTTP {}",
        status.as_u16()
    ))
}

pub(super) fn unavailable() -> AppError {
    AppError::Conflict("OpenAI-compatible provider is unavailable".into())
}

fn invalid_response() -> AppError {
    AppError::Conflict("OpenAI-compatible provider returned an invalid response".into())
}

fn incomplete_response() -> AppError {
    AppError::Conflict(
        "OpenAI-compatible provider did not return a complete tool-free proposal".into(),
    )
}

fn reported_error() -> AppError {
    AppError::Conflict(
        "OpenAI-compatible provider reported an error; no proposal was accepted".into(),
    )
}
