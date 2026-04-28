use axum::{
    extract::{Form, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
};
use serde::Deserialize;

use crate::{error::AppError, services::token_service, state::AppState};

#[derive(Debug, Deserialize)]
pub struct RevokeRequest {
    pub token: String,
    pub token_type_hint: Option<String>,
}

/// RFC 7009 token revocation endpoint.
/// Always returns 200; errors are silently swallowed (per spec).
pub async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(req): Form<RevokeRequest>,
) -> impl IntoResponse {
    let _ = do_revoke(&state, &headers, &req).await;
    StatusCode::OK
}

async fn do_revoke(
    state: &AppState,
    headers: &HeaderMap,
    req: &RevokeRequest,
) -> Result<(), AppError> {
    // Require Bearer token to identify the caller.
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let caller_claims = token_service::validate_access_token(bearer, &state.config.jwt_secret)?;

    let hint = req.token_type_hint.as_deref().unwrap_or("");

    if hint == "refresh_token" {
        try_revoke_refresh(state, &req.token, &caller_claims.sub).await?;
    } else {
        // Try as access token first, then refresh token.
        if try_revoke_access(state, &req.token, &caller_claims.sub)
            .await
            .is_err()
        {
            let _ = try_revoke_refresh(state, &req.token, &caller_claims.sub).await;
        }
    }

    Ok(())
}

async fn try_revoke_access(
    state: &AppState,
    token: &str,
    caller_sub: &str,
) -> Result<(), AppError> {
    let claims = token_service::validate_access_token(token, &state.config.jwt_secret)?;
    // Only allow revoking own tokens.
    if claims.sub != caller_sub {
        return Err(AppError::Unauthorized);
    }
    let expires_at = chrono::DateTime::from_timestamp(claims.exp, 0)
        .ok_or_else(|| AppError::TokenError("invalid exp".into()))?
        .fixed_offset();
    token_service::revoke_jti(&state.db, &claims.jti, expires_at).await
}

async fn try_revoke_refresh(
    state: &AppState,
    token: &str,
    caller_sub: &str,
) -> Result<(), AppError> {
    use crate::db;
    use uuid::Uuid;

    let token_hash = token_service::hash_refresh_token(token);
    let caller_id = Uuid::parse_str(caller_sub).map_err(|_| AppError::Unauthorized)?;

    // We need a tenant transaction but don't know the tenant from a refresh token alone.
    // Find the token without RLS by using direct DB (refresh_tokens has no FORCE RLS yet).
    use crate::entity::refresh_tokens;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let record = refresh_tokens::Entity::find()
        .filter(refresh_tokens::Column::TokenHash.eq(&token_hash))
        .filter(refresh_tokens::Column::RevokedAt.is_null())
        .one(&state.db)
        .await?;

    if let Some(r) = record {
        if r.user_id != caller_id {
            return Err(AppError::Unauthorized);
        }
        let txn = db::begin_tenant_txn(&state.db, r.tenant_id).await?;
        token_service::revoke_token(&txn, r).await?;
        txn.commit().await?;
    }

    Ok(())
}
