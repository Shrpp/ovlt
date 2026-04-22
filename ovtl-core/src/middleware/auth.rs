use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::Response,
};
use uuid::Uuid;

use crate::{
    error::AppError,
    middleware::tenant::TenantContext,
    services::token_service,
    state::AppState,
};

/// Authenticated user extracted from a valid JWT.
/// Available in handlers via `Extension(auth): Extension<AuthUser>`.
#[derive(Clone, Debug)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub email: String,
}

/// Axum middleware — validates Bearer token, cross-checks tid against TenantContext.
/// Must run after tenant_middleware (depends on TenantContext being in extensions).
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized)?;

    let claims = token_service::validate_access_token(token, &state.config.jwt_secret)?;

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::TokenError("invalid sub".into()))?;
    let token_tenant_id = Uuid::parse_str(&claims.tid)
        .map_err(|_| AppError::TokenError("invalid tid".into()))?;

    // tid in JWT must match the tenant from X-Pandora-Tenant-ID header
    if let Some(ctx) = req.extensions().get::<TenantContext>() {
        if token_tenant_id != ctx.tenant_id {
            return Err(AppError::Unauthorized);
        }
    }

    req.extensions_mut().insert(AuthUser {
        user_id,
        tenant_id: token_tenant_id,
        email: claims.email,
    });

    Ok(next.run(req).await)
}
