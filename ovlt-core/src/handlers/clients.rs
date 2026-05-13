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
    error::{validation_to_app_error, AppError},
    handlers::admin_auth,
    services::{audit_service, client_service},
    state::AppState,
};

fn extract_tenant_id(headers: &HeaderMap) -> Result<Uuid, AppError> {
    headers
        .get("x-ovlt-tenant-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::InvalidInput("x-ovlt-tenant-id header required".into()))
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateClientRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Option<Vec<String>>,
    pub grant_types: Option<Vec<String>>,
    pub is_confidential: Option<bool>,
    pub access_token_ttl_minutes: Option<i32>,
    pub refresh_token_ttl_days: Option<i32>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
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
    pub access_token_ttl_minutes: Option<i32>,
    pub refresh_token_ttl_days: Option<i32>,
    pub created_at: String,
}

#[utoipa::path(
    post,
    path = "/clients",
    tag = "clients",
    request_body = CreateClientRequest,
    responses(
        (status = 201, description = "Client created", body = ClientResponse),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn create_client(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateClientRequest>,
) -> Result<impl IntoResponse, AppError> {
    let actor = admin_auth::extract_actor(&headers, &state.config);
    admin_auth::require_admin(&headers, &state.config, state.master_tenant_id)?;
    let tenant_id = extract_tenant_id(&headers)?;

    payload.validate().map_err(validation_to_app_error)?;

    let is_confidential = payload.is_confidential.unwrap_or(true);
    let grant_types = payload
        .grant_types
        .unwrap_or_else(|| vec!["authorization_code".into()]);

    // Machine (M2M) clients using client_credentials exclusively don't need redirect_uris.
    let is_m2m = grant_types.iter().any(|g| g == "client_credentials")
        && !grant_types.iter().any(|g| g == "authorization_code");
    if !is_m2m && payload.redirect_uris.is_empty() {
        return Err(AppError::InvalidInput(
            "redirect_uris must not be empty".into(),
        ));
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
            access_token_ttl_minutes: payload.access_token_ttl_minutes,
            refresh_token_ttl_days: payload.refresh_token_ttl_days,
        },
    )
    .await?;

    txn.commit().await?;

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            tenant_id,
            actor,
            "client.created",
            serde_json::json!({"client_id": model.client_id.as_str(), "name": model.name.as_str()}),
        ),
    );

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
            access_token_ttl_minutes: model.access_token_ttl_minutes,
            refresh_token_ttl_days: model.refresh_token_ttl_days,
            created_at: model.created_at.to_rfc3339(),
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/clients",
    tag = "clients",
    responses(
        (status = 200, description = "List of clients", body = Vec<ClientResponse>),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn list_clients(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    admin_auth::require_admin(&headers, &state.config, state.master_tenant_id)?;
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
            access_token_ttl_minutes: m.access_token_ttl_minutes,
            refresh_token_ttl_days: m.refresh_token_ttl_days,
            created_at: m.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response))
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateClientRequest {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    pub redirect_uris: Vec<String>,
    pub scopes: Option<Vec<String>>,
    pub access_token_ttl_minutes: Option<i32>,
    pub refresh_token_ttl_days: Option<i32>,
    pub is_confidential: Option<bool>,
    pub grant_types: Option<Vec<String>>,
}

#[utoipa::path(
    put,
    path = "/clients/{id}",
    tag = "clients",
    request_body = UpdateClientRequest,
    responses(
        (status = 200, description = "Client updated", body = ClientResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Client not found"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("id" = String, Path, description = "Client UUID"),
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn update_client(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateClientRequest>,
) -> Result<impl IntoResponse, AppError> {
    let actor = admin_auth::extract_actor(&headers, &state.config);
    admin_auth::require_admin(&headers, &state.config, state.master_tenant_id)?;
    let tenant_id = extract_tenant_id(&headers)?;

    payload.validate().map_err(validation_to_app_error)?;

    let grant_types = payload
        .grant_types
        .unwrap_or_else(|| vec!["authorization_code".into()]);
    let is_m2m = grant_types.iter().any(|g| g == "client_credentials")
        && !grant_types.iter().any(|g| g == "authorization_code");
    if !is_m2m && payload.redirect_uris.is_empty() {
        return Err(AppError::InvalidInput(
            "redirect_uris must not be empty".into(),
        ));
    }

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    let model = client_service::update(
        &txn,
        id,
        client_service::UpdateClientInput {
            name: payload.name,
            redirect_uris: payload.redirect_uris,
            scopes: payload
                .scopes
                .unwrap_or_else(|| vec!["openid".into(), "email".into(), "profile".into()]),
            access_token_ttl_minutes: payload.access_token_ttl_minutes,
            refresh_token_ttl_days: payload.refresh_token_ttl_days,
            is_confidential: payload.is_confidential.unwrap_or(true),
            grant_types,
        },
    )
    .await?;
    txn.commit().await?;

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            tenant_id,
            actor,
            "client.updated",
            serde_json::json!({"client_uuid": id}),
        ),
    );

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
        access_token_ttl_minutes: model.access_token_ttl_minutes,
        refresh_token_ttl_days: model.refresh_token_ttl_days,
        created_at: model.created_at.to_rfc3339(),
    }))
}

#[utoipa::path(
    delete,
    path = "/clients/{id}",
    tag = "clients",
    responses(
        (status = 204, description = "Client deactivated"),
        (status = 401, description = "Unauthorized"),
    ),
    security(
        ("admin_key" = [])
    ),
    params(
        ("id" = String, Path, description = "Client UUID"),
        ("X-Ovlt-Tenant-Id" = String, Header, description = "Tenant UUID"),
    )
)]
pub async fn deactivate_client(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let actor = admin_auth::extract_actor(&headers, &state.config);
    admin_auth::require_admin(&headers, &state.config, state.master_tenant_id)?;
    let tenant_id = extract_tenant_id(&headers)?;

    let txn = db::begin_tenant_txn(&state.db, tenant_id).await?;
    client_service::deactivate(&txn, id).await?;
    txn.commit().await?;

    audit_service::record_best_effort(
        state.db.clone(),
        audit_service::AuditEvent::new(
            tenant_id,
            actor,
            "client.deactivated",
            serde_json::json!({"client_uuid": id}),
        ),
    );

    Ok(StatusCode::NO_CONTENT)
}
