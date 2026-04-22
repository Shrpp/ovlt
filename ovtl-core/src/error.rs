use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("unauthorized")]
    Unauthorized,
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
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            AppError::Unauthorized     => (StatusCode::UNAUTHORIZED, "Unauthorized".to_string()),
            AppError::NotFound         => (StatusCode::NOT_FOUND, "Not found".to_string()),
            AppError::Conflict         => (StatusCode::CONFLICT, "Already exists".to_string()),
            AppError::TooManyRequests  => (StatusCode::TOO_MANY_REQUESTS, "Too many requests".to_string()),
            AppError::InvalidInput(m)  => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::Internal(e) => {
                tracing::error!("db error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".to_string())
            }
            AppError::CryptoError(e) => {
                tracing::error!("crypto error: {e}");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal error".to_string())
            }
            AppError::TokenError(_) => (StatusCode::UNAUTHORIZED, "Invalid token".to_string()),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
