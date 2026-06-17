use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::domain::errors::AppError;

pub struct ApiError(pub AppError);

#[derive(Debug, Serialize)]
struct ErrorResponse {
    code: &'static str,
    message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self.0 {
            AppError::NoSessionAvailable => (
                StatusCode::SERVICE_UNAVAILABLE,
                "no_session_available",
                "no authenticated session is available".to_string(),
            ),
            AppError::SessionStoreUnavailable(message) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "session_store_unavailable",
                message,
            ),
            AppError::InvalidCachedHeaders { username, reason } => (
                StatusCode::SERVICE_UNAVAILABLE,
                "invalid_cached_headers",
                format!("invalid cached headers for {username}: {reason}"),
            ),
            AppError::MissingCustomerId { username } => (
                StatusCode::SERVICE_UNAVAILABLE,
                "missing_customer_id",
                format!("missing customer id in cached session for {username}"),
            ),
            AppError::RciAuthFailed { status } => (
                StatusCode::BAD_GATEWAY,
                "rci_auth_failed",
                format!("RCI authentication failed with status {status}"),
            ),
            AppError::RciUnavailable(message) => {
                (StatusCode::BAD_GATEWAY, "rci_unavailable", message)
            }
            AppError::RciUnexpectedStatus { status } => (
                StatusCode::BAD_GATEWAY,
                "rci_unexpected_status",
                format!("RCI returned unexpected status {status}"),
            ),
        };

        (status, Json(ErrorResponse { code, message })).into_response()
    }
}

impl From<AppError> for ApiError {
    fn from(value: AppError) -> Self {
        Self(value)
    }
}
