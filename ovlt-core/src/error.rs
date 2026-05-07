use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::{json, Map, Value};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("not found")]
    NotFound,
    #[error("conflict")]
    Conflict,
    #[error("too many requests")]
    TooManyRequests,
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("internal error")]
    Internal(#[from] sea_orm::DbErr),
    #[error("crypto error")]
    CryptoError(#[from] hefesto::HefestoError),
    #[error("token error")]
    TokenError(String),
    #[error("validation error")]
    Validation(Vec<(String, String)>),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        match self {
            AppError::Validation(fields) => {
                let mut field_map = Map::new();
                for (k, v) in fields {
                    field_map.insert(k, Value::String(v));
                }
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    Json(json!({
                        "error": "validation_failed",
                        "message": "Please check your input",
                        "fields": field_map,
                    })),
                )
                    .into_response()
            }
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "unauthorized",
                    "message": "Incorrect email or password",
                })),
            )
                .into_response(),
            AppError::Forbidden => (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "forbidden",
                    "message": "You don't have permission to perform this action",
                })),
            )
                .into_response(),
            AppError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "not_found",
                    "message": "The requested resource was not found",
                })),
            )
                .into_response(),
            AppError::Conflict => (
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "already_exists",
                    "message": "This already exists",
                })),
            )
                .into_response(),
            AppError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({
                    "error": "too_many_requests",
                    "message": "Too many attempts. Please wait before trying again",
                })),
            )
                .into_response(),
            AppError::InvalidInput(m) => (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "invalid_input",
                    "message": m,
                })),
            )
                .into_response(),
            AppError::TokenError(_) => (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": "unauthorized",
                    "message": "Invalid or expired token",
                })),
            )
                .into_response(),
            AppError::Internal(e) => {
                tracing::error!("db error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "server_error",
                        "message": "Something went wrong. Please try again",
                    })),
                )
                    .into_response()
            }
            AppError::CryptoError(e) => {
                tracing::error!("crypto error: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "server_error",
                        "message": "Something went wrong. Please try again",
                    })),
                )
                    .into_response()
            }
        }
    }
}
