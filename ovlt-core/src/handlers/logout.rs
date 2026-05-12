use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use serde::Deserialize;

use crate::{
    error::AppError,
    extractors::TenantDb,
    middleware::auth::AuthUser,
    services::{session_service, token_service},
    state::AppState,
};

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "auth",
    request_body = LogoutRequest,
    responses(
        (status = 204, description = "Logged out successfully"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("bearer_auth" = [])
    ),
    params(
        ("X-Tenant-ID" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    Extension(auth): Extension<AuthUser>,
    db: TenantDb,
    Json(payload): Json<LogoutRequest>,
) -> Result<impl IntoResponse, AppError> {
    let TenantDb { txn, .. } = db;

    // Revoke the access token JTI (cross-tenant table, uses raw connection intentionally).
    let exp = chrono::DateTime::from_timestamp(
        jsonwebtoken::decode::<token_service::Claims>(
            headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .unwrap_or(""),
            &jsonwebtoken::DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
            &jsonwebtoken::Validation::default(),
        )
        .map(|d| d.claims.exp)
        .unwrap_or(0),
        0,
    )
    .unwrap_or_else(chrono::Utc::now)
    .fixed_offset();

    token_service::revoke_jti(&state.db, &auth.jti, exp).await?;

    // Revoke the refresh token through the RLS-scoped transaction.
    let token_hash = token_service::hash_refresh_token(&payload.refresh_token);
    if let Some(r) = token_service::find_valid_refresh_token(&txn, &token_hash).await? {
        if r.user_id == auth.user_id {
            token_service::revoke_token(&txn, r).await?;
        }
    }
    txn.commit().await?;

    // Delete session cookie if present.
    if let Some(session_id) = get_session_cookie(&headers) {
        let _ = session_service::delete(&state.db, &session_id).await;
    }

    let secure = if state.config.is_production() {
        "; Secure"
    } else {
        ""
    };
    let mut response_headers = HeaderMap::new();
    response_headers.insert(
        header::SET_COOKIE,
        format!("ovlt_session=; HttpOnly; SameSite=Lax{secure}; Path=/; Max-Age=0")
            .parse()
            .unwrap(),
    );

    Ok((StatusCode::NO_CONTENT, response_headers))
}

pub fn get_session_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get("cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|c| {
                c.trim()
                    .strip_prefix("ovlt_session=")
                    .map(|v| v.to_string())
            })
        })
}
