use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use migration::{Migrator, MigratorTrait};
use ovlt_core::{
    config::Config,
    db,
    entity::tenants,
    router::build_router,
    services::{client_service, jwk_service::JwkService, token_service, user_service},
    state::AppState,
};
use sea_orm::{ActiveModelTrait, Set};
use sha2::{Digest, Sha256};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use uuid::Uuid;
use webauthn_rs::prelude::{Url, WebauthnBuilder};

// ── Server helpers ────────────────────────────────────────────────────────────

struct TestServer {
    pub base_url: String,
    pub config: Config,
    pub db: sea_orm::DatabaseConnection,
}

async fn spawn_server() -> TestServer {
    dotenvy::dotenv().ok();
    let cfg = Config::from_env().expect("config");
    let db = db::connect(&cfg.database_url, 5, 1).await.expect("db");
    Migrator::up(&db, None).await.expect("migrations");

    let jwk = JwkService::generate();

    let rp_origin = Url::parse(&cfg.ovlt_issuer).expect("issuer url");
    let rp_id = rp_origin.host_str().unwrap_or("localhost").to_string();
    let webauthn = Arc::new(
        WebauthnBuilder::new(&rp_id, &rp_origin)
            .unwrap()
            .rp_name("OVLT-Test")
            .build()
            .unwrap(),
    );

    let state = AppState::new(db.clone(), cfg.clone(), jwk, None, webauthn);
    let app = build_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");

    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });

    TestServer { base_url, config: cfg, db }
}

// ── Fixture helpers ───────────────────────────────────────────────────────────

async fn create_tenant(
    db: &sea_orm::DatabaseConnection,
    cfg: &Config,
) -> (Uuid, String) {
    let tenant_key = "test-tenant-key-exactly-32-chars!";
    let encrypted_key = hefesto::encrypt(
        tenant_key,
        &cfg.tenant_wrap_key,
        &cfg.master_encryption_key,
    )
    .expect("encrypt tenant key");

    let tenant_id = Uuid::new_v4();
    tenants::ActiveModel {
        id: Set(tenant_id),
        name: Set(format!("Test Tenant {tenant_id}")),
        slug: Set(tenant_id.to_string()),
        encryption_key: Set(encrypted_key),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert tenant");

    (tenant_id, tenant_key.to_string())
}

async fn create_user(
    db: &sea_orm::DatabaseConnection,
    cfg: &Config,
    tenant_id: Uuid,
    tenant_key: &str,
    email: &str,
) -> ovlt_core::entity::users::Model {
    let txn = db::begin_tenant_txn(db, tenant_id).await.unwrap();
    let user = user_service::create(
        &txn,
        user_service::CreateUserInput {
            tenant_id,
            email_encrypted: hefesto::encrypt(email, tenant_key, &cfg.master_encryption_key)
                .unwrap(),
            email_lookup: hefesto::hash_for_lookup(email, tenant_key).unwrap(),
            password_hash: hefesto::hash_password("Pass1234!").unwrap(),
        },
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();
    user
}

async fn create_public_client(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    redirect_uri: &str,
) -> ovlt_core::entity::oauth_clients::Model {
    let txn = db::begin_tenant_txn(db, tenant_id).await.unwrap();
    let (client, _) = client_service::create(
        &txn,
        client_service::CreateClientInput {
            tenant_id,
            name: "Test Client".into(),
            redirect_uris: vec![redirect_uri.to_string()],
            scopes: vec!["openid".into(), "email".into()],
            grant_types: vec!["authorization_code".into()],
            is_confidential: false,
            access_token_ttl_minutes: None,
            refresh_token_ttl_days: None,
        },
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();
    client
}

fn pkce_pair() -> (String, String) {
    let verifier = hex::encode(Sha256::digest(Uuid::new_v4().as_bytes()))
        + &hex::encode(Sha256::digest(Uuid::new_v4().as_bytes()));
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

fn bearer_token(cfg: &Config, user_id: Uuid, tenant_id: Uuid, email: &str) -> String {
    token_service::generate_access_token(
        user_id,
        tenant_id,
        email,
        vec![],
        vec![],
        HashMap::new(),
        &cfg.jwt_secret,
        15,
    )
    .unwrap()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_oidc_auth_code_flow() {
    let srv = spawn_server().await;
    let (tenant_id, tenant_key) = create_tenant(&srv.db, &srv.config).await;
    let user = create_user(&srv.db, &srv.config, tenant_id, &tenant_key, "oidc@test.dev").await;
    let redirect_uri = "https://app.example.com/callback";
    let client = create_public_client(&srv.db, tenant_id, redirect_uri).await;

    let (verifier, challenge) = pkce_pair();
    let access_token = bearer_token(&srv.config, user.id, tenant_id, "oidc@test.dev");

    // 1. Authorize → expect redirect with code
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let authorize_url = format!(
        "{}/oauth/authorize?client_id={}&redirect_uri={}&response_type=code\
         &code_challenge={}&code_challenge_method=S256&scope=openid+email",
        srv.base_url,
        client.client_id,
        urlencoding::encode(redirect_uri),
        challenge,
    );

    let resp = http
        .get(&authorize_url)
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 302, "authorize must redirect");

    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    let code = location
        .split("code=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap()
        .to_string();
    assert!(!code.is_empty(), "code must be present in redirect");

    // 2. Exchange code for tokens
    let token_resp = http
        .post(format!("{}/oauth/token", srv.base_url))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", &code),
            ("redirect_uri", redirect_uri),
            ("client_id", &client.client_id),
            ("code_verifier", &verifier),
        ])
        .send()
        .await
        .unwrap();

    assert_eq!(token_resp.status(), 200, "token exchange must succeed");
    let body: serde_json::Value = token_resp.json().await.unwrap();
    let issued_token = body["access_token"].as_str().unwrap().to_string();
    assert!(!issued_token.is_empty(), "access_token must be present");
    assert!(body["id_token"].is_string(), "id_token must be present");
    assert!(body["refresh_token"].is_string(), "refresh_token must be present");

    // 3. Introspect → active
    let Some(admin_key) = &srv.config.admin_key else {
        return; // introspect requires admin key; skip if unconfigured in test env
    };

    let introspect_resp = http
        .post(format!("{}/oauth/introspect", srv.base_url))
        .header("X-OVLT-Admin-Key", admin_key.as_str())
        .form(&[("token", issued_token.as_str())])
        .send()
        .await
        .unwrap();

    assert_eq!(introspect_resp.status(), 200);
    let introspect_body: serde_json::Value = introspect_resp.json().await.unwrap();
    assert_eq!(introspect_body["active"], true, "freshly issued token must be active");
    assert_eq!(introspect_body["sub"], user.id.to_string());
}

#[tokio::test]
async fn test_oidc_pkce_wrong_verifier_rejected() {
    let srv = spawn_server().await;
    let (tenant_id, tenant_key) = create_tenant(&srv.db, &srv.config).await;
    let user = create_user(&srv.db, &srv.config, tenant_id, &tenant_key, "pkce@test.dev").await;
    let redirect_uri = "https://app.example.com/callback";
    let client = create_public_client(&srv.db, tenant_id, redirect_uri).await;

    let (_verifier, challenge) = pkce_pair();
    let access_token = bearer_token(&srv.config, user.id, tenant_id, "pkce@test.dev");

    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let resp = http
        .get(format!(
            "{}/oauth/authorize?client_id={}&redirect_uri={}&response_type=code\
             &code_challenge={}&code_challenge_method=S256",
            srv.base_url,
            client.client_id,
            urlencoding::encode(redirect_uri),
            challenge,
        ))
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 302);
    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    let code = location.split("code=").nth(1).unwrap().split('&').next().unwrap();

    // Use wrong verifier — token endpoint must reject
    let token_resp = http
        .post(format!("{}/oauth/token", srv.base_url))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", &client.client_id),
            ("code_verifier", "this-is-the-wrong-verifier"),
        ])
        .send()
        .await
        .unwrap();

    assert_eq!(token_resp.status(), 400, "wrong verifier must be rejected");
}

#[tokio::test]
async fn test_oidc_code_reuse_rejected() {
    let srv = spawn_server().await;
    let (tenant_id, tenant_key) = create_tenant(&srv.db, &srv.config).await;
    let user = create_user(&srv.db, &srv.config, tenant_id, &tenant_key, "reuse@test.dev").await;
    let redirect_uri = "https://app.example.com/callback";
    let client = create_public_client(&srv.db, tenant_id, redirect_uri).await;

    let (verifier, challenge) = pkce_pair();
    let access_token = bearer_token(&srv.config, user.id, tenant_id, "reuse@test.dev");

    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();

    let resp = http
        .get(format!(
            "{}/oauth/authorize?client_id={}&redirect_uri={}&response_type=code\
             &code_challenge={}&code_challenge_method=S256",
            srv.base_url,
            client.client_id,
            urlencoding::encode(redirect_uri),
            challenge,
        ))
        .header("Authorization", format!("Bearer {access_token}"))
        .send()
        .await
        .unwrap();

    let location = resp.headers().get("location").unwrap().to_str().unwrap();
    let code = location
        .split("code=")
        .nth(1)
        .unwrap()
        .split('&')
        .next()
        .unwrap()
        .to_string();

    let params = [
        ("grant_type", "authorization_code"),
        ("code", &code),
        ("redirect_uri", redirect_uri),
        ("client_id", &client.client_id),
        ("code_verifier", &verifier),
    ];

    // First use succeeds
    let first = http
        .post(format!("{}/oauth/token", srv.base_url))
        .form(&params)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 200, "first use must succeed");

    // Second use of the same code must be rejected
    let second = http
        .post(format!("{}/oauth/token", srv.base_url))
        .form(&params)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), 400, "code reuse must be rejected");
}

#[tokio::test]
async fn test_introspect_invalid_token_returns_inactive() {
    let srv = spawn_server().await;

    let Some(admin_key) = &srv.config.admin_key else {
        return;
    };

    let http = reqwest::Client::new();
    let resp = http
        .post(format!("{}/oauth/introspect", srv.base_url))
        .header("X-OVLT-Admin-Key", admin_key.as_str())
        .form(&[("token", "this.is.not.a.valid.jwt")])
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["active"], false);
}
