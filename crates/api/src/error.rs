//! `ApiError`: one error type for all handlers → consistent JSON envelope + HTTP status.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("resource not found")]
    NotFound,

    #[error("{0}")]
    BadRequest(String),

    #[error("validation failed")]
    Validation(validator::ValidationErrors),

    #[error("unauthorized")]
    Unauthorized,

    #[error("forbidden")]
    Forbidden,

    #[error("{0}")]
    Conflict(String),

    #[error("service unavailable: {0}")]
    Unavailable(String),

    /// Anything unexpected. The detail is logged, never sent to the client.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

/// Machine-readable error code, stable across responses.
impl ApiError {
    fn parts(&self) -> (StatusCode, &'static str) {
        match self {
            ApiError::NotFound => (StatusCode::NOT_FOUND, "not_found"),
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            ApiError::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "validation_error"),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized"),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "forbidden"),
            ApiError::Conflict(_) => (StatusCode::CONFLICT, "conflict"),
            ApiError::Unavailable(_) => (StatusCode::SERVICE_UNAVAILABLE, "unavailable"),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fields: Option<serde_json::Value>,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code) = self.parts();

        // Log server-side faults with full detail; clients see a generic message.
        if status.is_server_error() {
            tracing::error!(error = ?self, "request failed");
        }

        let message = match &self {
            ApiError::Internal(_) => "an internal error occurred".to_string(),
            other => other.to_string(),
        };

        let fields = match &self {
            ApiError::Validation(errors) => serde_json::to_value(errors).ok(),
            _ => None,
        };

        let body = ErrorBody {
            error: ErrorDetail {
                code,
                message,
                fields,
            },
        };

        (status, Json(body)).into_response()
    }
}

/// Convenience: the JSON 404 used by the router's global fallback.
pub async fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": { "code": "not_found", "message": "resource not found" } })),
    )
        .into_response()
}

pub type ApiResult<T> = Result<T, ApiError>;

// Map common infra errors into ApiError so handlers can use `?` directly.
impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => ApiError::NotFound,
            other => ApiError::Internal(other.into()),
        }
    }
}

impl From<ferrite_stream_storage::StorageError> for ApiError {
    fn from(e: ferrite_stream_storage::StorageError) -> Self {
        ApiError::Internal(anyhow::anyhow!(e))
    }
}

impl From<ferrite_stream_queue::QueueError> for ApiError {
    fn from(e: ferrite_stream_queue::QueueError) -> Self {
        ApiError::Internal(anyhow::anyhow!(e))
    }
}
