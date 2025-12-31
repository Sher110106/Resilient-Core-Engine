use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Coordinator error: {0}")]
    CoordinatorError(#[from] crate::coordinator::CoordinatorError),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Transfer not found: {0}")]
    NotFound(String),

    #[error("Internal server error: {0}")]
    InternalError(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, error_message, error_code) = match self {
            ApiError::CoordinatorError(e) => {
                (StatusCode::BAD_REQUEST, e.to_string(), "COORDINATOR_ERROR")
            }
            ApiError::InvalidRequest(e) => {
                (StatusCode::BAD_REQUEST, e, "INVALID_REQUEST")
            }
            ApiError::NotFound(e) => {
                (StatusCode::NOT_FOUND, e, "NOT_FOUND")
            }
            ApiError::InternalError(e) => {
                (StatusCode::INTERNAL_SERVER_ERROR, e, "INTERNAL_ERROR")
            }
        };

        let body = Json(json!({
            "error": error_message,
            "code": error_code,
        }));

        (status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
