use axum::{
    extract::{ConnectInfo, State},
    http::header,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::net::SocketAddr;
use validator::Validate;

use crate::{
    db,
    error::AppError,
    handlers::login::TokenResponse,
    services::{
        audit_service, lockout_service, mfa_service, permission_service, role_service,
        session_service, tenant_service, tenant_settings_service, token_service, user_service,
    },
    state::AppState,
};

#[derive(Debug, Deserialize, Validate)]
pub struct UniversalLoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8, max = 128))]
    pub password: String,
}

struct TenantMatch {
    tenant_id: uuid::Uuid,
    tenant_key: String,
    slug: String,
    name: String,
}

#[allow(clippy::too_many_arguments)]
pub async fn login_universal(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Json(payload): Json<UniversalLoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    if let Err(errs) = payload.validate() {
        let fields: Vec<(String, String)> = errs
            .field_errors()
            .iter()
            .map(|(field, errors)| {
                let msg = match *field {
                    "email" => "Enter a valid email address".to_string(),
                    "password" => "Password must be at least 8 characters".to_string(),
                    _ => errors
                        .first()
                        .and_then(|e| e.message.as_ref().map(|m| m.to_string()))
                        .unwrap_or_else(|| "Invalid value".to_string()),
                };
                (field.to_string(), msg)
            })
            .collect();
        return Err(AppError::Validation(fields));
    }

    let ip = addr.ip().to_string();
    let email_normalized = payload.email.trim().to_lowercase();

    // Load all active tenants and search each one for the user
    let all_tenants = tenant_service::list_all_active(&state.db).await?;

    let mut matches: Vec<TenantMatch> = Vec::new();

    for t in &all_tenants {
        let tenant_key = match hefesto::decrypt(
            &t.encryption_key_encrypted,
            &state.config.tenant_wrap_key,
            &state.config.master_encryption_key,
        ) {
            Ok(k) => k,
            Err(_) => continue,
        };

        let email_lookup = match hefesto::hash_for_lookup(&email_normalized, &tenant_key) {
            Ok(h) => h,
            Err(_) => continue,
        };

        // We need to look up by tenant_id — use a direct query without RLS txn
        // since we're scanning across tenants
        let txn = match db::begin_tenant_txn(&state.db, t.id).await {
            Ok(t) => t,
            Err(_) => continue,
        };

        let user_opt = user_service::find_by_email_lookup(&txn, &email_lookup).await;
        let _ = txn.commit().await;

        if let Ok(Some(_user)) = user_opt {
            // We need the slug and name — fetch from the tenants list
            // Re-query to get slug/name (the TenantRecord doesn't have them)
            matches.push(TenantMatch {
                tenant_id: t.id,
                tenant_key,
                slug: String::new(), // filled below
                name: String::new(),
            });
        }
    }

    if matches.is_empty() {
        return Err(AppError::Unauthorized);
    }

    // Fetch tenant details for matched tenants to get slug/name
    // Re-use the entity directly
    use crate::entity::tenants;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let matched_ids: Vec<uuid::Uuid> = matches.iter().map(|m| m.tenant_id).collect();
    let tenant_rows = tenants::Entity::find()
        .filter(tenants::Column::Id.is_in(matched_ids))
        .all(&state.db)
        .await?;

    // Rebuild matches with slug/name
    let mut full_matches: Vec<TenantMatch> = Vec::new();
    for m in matches {
        if let Some(row) = tenant_rows.iter().find(|r| r.id == m.tenant_id) {
            full_matches.push(TenantMatch {
                tenant_id: m.tenant_id,
                tenant_key: m.tenant_key,
                slug: row.slug.clone(),
                name: row.name.clone(),
            });
        }
    }

    if full_matches.is_empty() {
        return Err(AppError::Unauthorized);
    }

    // If >1 tenant matched, return the list for the client to pick
    if full_matches.len() > 1 {
        let tenants_json: Vec<serde_json::Value> = full_matches
            .iter()
            .map(|m| json!({ "slug": m.slug, "name": m.name }))
            .collect();
        return Ok(Json(json!({ "tenants": tenants_json })).into_response());
    }

    // Exactly one match — run the full login flow
    let matched = full_matches.remove(0);
    let tenant_id = matched.tenant_id;
    let tenant_key = matched.tenant_key;

    let email_lookup = hefesto::hash_for_lookup(&email_normalized, &tenant_key)?;

    let settings = tenant_settings_service::get(&state.db, tenant_id).await?;

    if lockout_service::is_locked(
        &state.db,
        tenant_id,
        &email_lookup,
        settings.lockout_max_attempts,
        settings.lockout_window_minutes,
    )
    .await?
    {
        audit_service::record(
            state.db.clone(),
            tenant_id,
            None,
            "login.locked",
            Some(ip.clone()),
            None,
        );
        return Err(AppError::Unauthorized);
    }

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;

    let user = match user_service::find_by_email_lookup(&txn, &email_lookup).await? {
        Some(u) => u,
        None => {
            txn.commit().await?;
            lockout_service::record_attempt(&state.db, tenant_id, &email_lookup).await?;
            audit_service::record(
                state.db.clone(),
                tenant_id,
                None,
                "login.failed.unknown_email",
                Some(ip),
                None,
            );
            return Err(AppError::Unauthorized);
        }
    };

    if !user.is_active {
        txn.commit().await?;
        return Err(AppError::Unauthorized);
    }

    if settings.require_email_verified && !user.email_verified {
        txn.commit().await?;
        return Err(AppError::InvalidInput("email not verified".into()));
    }

    if !hefesto::verify_password(&payload.password, &user.password_hash) {
        txn.commit().await?;
        lockout_service::record_attempt(&state.db, tenant_id, &email_lookup).await?;
        audit_service::record(
            state.db.clone(),
            tenant_id,
            Some(user.id),
            "login.failed.wrong_password",
            Some(ip),
            None,
        );
        return Err(AppError::Unauthorized);
    }

    let email_plain = hefesto::decrypt(
        &user.email,
        &tenant_key,
        &state.config.master_encryption_key,
    )?;

    // MFA check
    if mfa_service::find_enabled(&txn, tenant_id, user.id)
        .await?
        .is_some()
    {
        txn.commit().await?;
        lockout_service::clear_attempts(&state.db, tenant_id, &email_lookup).await?;
        let mfa_token =
            token_service::generate_mfa_token(user.id, tenant_id, &state.config.jwt_secret)?;
        return Ok(Json(json!({
            "mfa_required": true,
            "mfa_token": mfa_token,
        }))
        .into_response());
    }

    let roles = role_service::list_names_for_user(&txn, user.id, tenant_id)
        .await
        .unwrap_or_default();
    let permissions = permission_service::list_names_for_user(&txn, user.id, tenant_id)
        .await
        .unwrap_or_default();

    let access_token = token_service::generate_access_token(
        user.id,
        tenant_id,
        &email_plain,
        roles,
        permissions,
        std::collections::HashMap::new(),
        &state.config.jwt_secret,
        settings.access_token_ttl_minutes,
    )?;

    let refresh_token = token_service::generate_refresh_token();
    let token_hash = token_service::hash_refresh_token(&refresh_token);

    token_service::store_refresh_token(
        &txn,
        tenant_id,
        user.id,
        token_hash,
        settings.refresh_token_ttl_days,
    )
    .await?;

    txn.commit().await?;

    lockout_service::clear_attempts(&state.db, tenant_id, &email_lookup).await?;
    audit_service::record(
        state.db.clone(),
        tenant_id,
        Some(user.id),
        "login.success",
        Some(ip.clone()),
        None,
    );

    let session_id = session_service::create(
        &state.db,
        tenant_id,
        user.id,
        session_service::SessionData {
            email: email_plain,
            ip: Some(ip),
        },
        settings.refresh_token_ttl_days,
    )
    .await
    .unwrap_or_default();

    let cookie = format!(
        "ovlt_session={session_id}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}",
        settings.refresh_token_ttl_days * 86400
    );

    let mut response_headers = axum::http::HeaderMap::new();
    response_headers.insert(header::SET_COOKIE, cookie.parse().unwrap());

    Ok((
        response_headers,
        Json(TokenResponse {
            access_token,
            refresh_token,
            expires_in: settings.access_token_ttl_minutes * 60,
        }),
    )
        .into_response())
}
