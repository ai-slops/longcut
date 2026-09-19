use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("stale revision")]
    Stale,
    #[error("lock conflict: {0}")]
    Locked(String),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal error")]
    Internal(#[from] anyhow::Error),
    #[error("database error")]
    Database(#[from] sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            Self::NotFound => (StatusCode::NOT_FOUND, "not_found", self.to_string()),
            Self::Stale => (StatusCode::CONFLICT, "stale_revision", self.to_string()),
            Self::Locked(message) => (StatusCode::LOCKED, "lock_conflict", message.clone()),
            Self::Invalid(message) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_input",
                message.clone(),
            ),
            Self::Conflict(message) => (StatusCode::CONFLICT, "conflict", message.clone()),
            Self::Internal(error) => {
                tracing::error!(error = ?error, "request.internal");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "internal error".into(),
                )
            }
            Self::Database(error) => {
                tracing::error!(error = ?error, "request.database");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database",
                    "internal error".into(),
                )
            }
        };
        (
            status,
            Json(json!({"error": {"code": code, "message": message}})),
        )
            .into_response()
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Internal(value.into())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Internal(value.into())
    }
}

pub type AppResult<T> = Result<T, AppError>;
