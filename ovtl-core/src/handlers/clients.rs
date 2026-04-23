use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{
    db,
    error::AppError,
    middleware::tenant::TenantContext,
    services::client_service::{self, CreateClientInput},
    state::AppState,
};

#[derive(Debug, Deserialize, Validate)]
pub struct CreateClientRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Option<Vec<String>>,
    pub grant_types: Option<Vec<String>>,
    pub is_confidential: Option<bool>,
    pub require_consent: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ClientResponse {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
    pub is_confidential: bool,
    pub require_consent: bool,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateClientResponse {
    #[serde(flatten)]
    pub client: ClientResponse,
    pub client_secret: String,
}

fn to_response(m: &crate::entity::oauth_clients::Model) -> ClientResponse {
    ClientResponse {
        id: m.id.to_string(),
        client_id: m.client_id.clone(),
        name: m.name.clone(),
        redirect_uris: serde_json::from_value(m.redirect_uris.clone()).unwrap_or_default(),
        scopes: serde_json::from_value(m.scopes.clone()).unwrap_or_default(),
        grant_types: serde_json::from_value(m.grant_types.clone()).unwrap_or_default(),
        is_confidential: m.is_confidential,
        require_consent: m.require_consent,
        created_at: m.created_at.to_rfc3339(),
    }
}

pub async fn create_client(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Json(payload): Json<CreateClientRequest>,
) -> Result<impl IntoResponse, AppError> {
    payload
        .validate()
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    if payload.redirect_uris.is_empty() {
        return Err(AppError::InvalidInput("redirect_uris cannot be empty".into()));
    }

    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;

    let (model, plain_secret) = client_service::create(
        &txn,
        ctx.tenant_id,
        CreateClientInput {
            name: payload.name,
            redirect_uris: payload.redirect_uris,
            scopes: payload
                .scopes
                .unwrap_or_else(|| vec!["openid".into(), "email".into(), "profile".into()]),
            grant_types: payload
                .grant_types
                .unwrap_or_else(|| vec!["authorization_code".into()]),
            is_confidential: payload.is_confidential.unwrap_or(true),
            require_consent: payload.require_consent.unwrap_or(false),
        },
    )
    .await?;

    txn.commit().await?;

    Ok((
        StatusCode::CREATED,
        Json(CreateClientResponse {
            client: to_response(&model),
            client_secret: plain_secret,
        }),
    ))
}

pub async fn list_clients(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
) -> Result<impl IntoResponse, AppError> {
    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;
    let clients = client_service::list(&txn).await?;
    txn.commit().await?;

    let response: Vec<ClientResponse> = clients.iter().map(to_response).collect();
    Ok(Json(response))
}

pub async fn delete_client(
    State(state): State<AppState>,
    Extension(ctx): Extension<TenantContext>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let txn = db::begin_tenant_txn(&state.db, ctx.tenant_id).await?;
    client_service::deactivate(&txn, id).await?;
    txn.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}
