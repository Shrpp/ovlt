use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{
    db,
    error::AppError,
    handlers::admin_auth,
    services::client_service,
    state::AppState,
};

fn extract_tenant_id(headers: &HeaderMap) -> Result<Uuid, AppError> {
    headers
        .get("x-ovtl-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::InvalidInput("x-ovtl-tenant-id header required".into()))
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateClientRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Option<Vec<String>>,
    pub grant_types: Option<Vec<String>>,
    pub is_confidential: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ClientResponse {
    pub id: String,
    pub client_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub is_confidential: bool,
    pub is_active: bool,
    pub created_at: String,
}

pub async fn create_client(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateClientRequest>,
) -> Result<impl IntoResponse, AppError> {
    admin_auth::require_admin(&headers, &state.config.admin_key, &state.config.jwt_secret, state.master_tenant_id)?;
    let tenant_id = extract_tenant_id(&headers)?;

    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    let is_confidential = payload.is_confidential.unwrap_or(true);
    let grant_types = payload
        .grant_types
        .unwrap_or_else(|| vec!["authorization_code".into()]);

    // Machine (M2M) clients using client_credentials exclusively don't need redirect_uris.
    let is_m2m = grant_types.iter().any(|g| g == "client_credentials")
        && !grant_types.iter().any(|g| g == "authorization_code");
    if !is_m2m && payload.redirect_uris.is_empty() {
        return Err(AppError::InvalidInput("redirect_uris must not be empty".into()));
    }

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;

    let (model, plain_secret) = client_service::create(
        &txn,
        client_service::CreateClientInput {
            tenant_id,
            name: payload.name,
            redirect_uris: payload.redirect_uris,
            scopes: payload
                .scopes
                .unwrap_or_else(|| vec!["openid".into(), "email".into(), "profile".into()]),
            grant_types,
            is_confidential,
        },
    )
    .await?;

    txn.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(ClientResponse {
            id: model.id.to_string(),
            client_id: model.client_id,
            client_secret: plain_secret,
            name: model.name,
            redirect_uris: client_service::scopes_to_vec(&model.redirect_uris),
            scopes: client_service::scopes_to_vec(&model.scopes),
            grant_types: client_service::scopes_to_vec(&model.grant_types),
            is_confidential: model.is_confidential,
            is_active: model.is_active,
            created_at: model.created_at.to_rfc3339(),
        }),
    ))
}

pub async fn list_clients(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    admin_auth::require_admin(&headers, &state.config.admin_key, &state.config.jwt_secret, state.master_tenant_id)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let models = client_service::list(&txn).await?;
    txn.commit().await?;

    let response: Vec<ClientResponse> = models
        .into_iter()
        .map(|m| ClientResponse {
            id: m.id.to_string(),
            client_id: m.client_id,
            client_secret: None,
            name: m.name,
            redirect_uris: client_service::scopes_to_vec(&m.redirect_uris),
            scopes: client_service::scopes_to_vec(&m.scopes),
            grant_types: client_service::scopes_to_vec(&m.grant_types),
            is_confidential: m.is_confidential,
            is_active: m.is_active,
            created_at: m.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response))
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateClientRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Option<Vec<String>>,
}

pub async fn update_client(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateClientRequest>,
) -> Result<impl IntoResponse, AppError> {
    admin_auth::require_admin(&headers, &state.config.admin_key, &state.config.jwt_secret, state.master_tenant_id)?;
    let tenant_id = extract_tenant_id(&headers)?;

    payload.validate().map_err(|e| AppError::InvalidInput(e.to_string()))?;
    if payload.redirect_uris.is_empty() {
        return Err(AppError::InvalidInput("redirect_uris must not be empty".into()));
    }

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let model = client_service::update(
        &txn,
        id,
        client_service::UpdateClientInput {
            name: payload.name,
            redirect_uris: payload.redirect_uris,
            scopes: payload.scopes.unwrap_or_else(|| vec!["openid".into(), "email".into(), "profile".into()]),
        },
    ).await?;
    txn.commit().await?;

    Ok(Json(ClientResponse {
        id: model.id.to_string(),
        client_id: model.client_id,
        client_secret: None,
        name: model.name,
        redirect_uris: client_service::scopes_to_vec(&model.redirect_uris),
        scopes: client_service::scopes_to_vec(&model.scopes),
        grant_types: client_service::scopes_to_vec(&model.grant_types),
        is_confidential: model.is_confidential,
        is_active: model.is_active,
        created_at: model.created_at.to_rfc3339(),
    }))
}

pub async fn deactivate_client(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    admin_auth::require_admin(&headers, &state.config.admin_key, &state.config.jwt_secret, state.master_tenant_id)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    client_service::deactivate(&txn, id).await?;
    txn.commit().await?;

    Ok(StatusCode::NO_CONTENT)
}
