use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use pyo3::PyErr;
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("Provider error: {0}")]
    Provider(String),

    #[error("GRPO error: {0}")]
    Grpo(String),

    #[error("Insufficient credits")]
    InsufficientCredits,

    #[error("Rate limit exceeded")]
    RateLimit,

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        // Capture error type before match consumes self
        let error_type = match &self {
            AppError::Auth(_) => "Auth",
            AppError::Database(_) => "Database",
            AppError::Redis(_) => "Redis",
            AppError::Provider(_) => "Provider",
            AppError::Grpo(_) => "Grpo",
            AppError::InsufficientCredits => "InsufficientCredits",
            AppError::RateLimit => "RateLimit",
            AppError::BadRequest(_) => "BadRequest",
            AppError::NotFound(_) => "NotFound",
            AppError::Internal(_) => "Internal",
        };

        let (status, error_message) = match self {
            AppError::Auth(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::InsufficientCredits => (StatusCode::PAYMENT_REQUIRED, "Insufficient credits".to_string()),
            AppError::RateLimit => (StatusCode::TOO_MANY_REQUESTS, "Rate limit exceeded".to_string()),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Provider(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::Grpo(msg) => (StatusCode::INTERNAL_SERVER_ERROR, format!("GRPO error: {}", msg)),
            AppError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Database error: {}", e)),
            AppError::Redis(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Cache error: {}", e)),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        let body = Json(json!({
            "error": {
                "message": error_message,
                "type": error_type,
            }
        }));

        (status, body).into_response()
    }
}

impl From<PyErr> for AppError {
    fn from(err: PyErr) -> Self {
        AppError::Grpo(format!("Python error: {}", err))
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
