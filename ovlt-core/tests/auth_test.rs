use ovlt_core::{
    config::Config,
    db,
    entity::tenants,
    services::{
        lockout_service, mfa_service, oauth_service, one_time_token_service, token_service,
        user_service,
    },
};
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};
use std::collections::HashMap;
use uuid::Uuid;

async fn setup() -> (sea_orm::DatabaseConnection, Config, Uuid, String) {
    dotenvy::dotenv().ok();
    let cfg = Config::from_env().expect("config");
    let db = db::connect(&cfg.database_url, 5, 1).await.expect("db");

    let tenant_key_plain = "dev-test-tenant-key-32chars-long!";
    let encrypted_key = hefesto::encrypt(
        tenant_key_plain,
        &cfg.tenant_wrap_key,
        &cfg.master_encryption_key,
    )
    .expect("encrypt");

    let tenant_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

    let existing = tenants::Entity::find_by_id(tenant_id)
        .one(&db)
        .await
        .expect("find");

    if let Some(t) = existing {
        let mut active: tenants::ActiveModel = t.into();
        active.encryption_key = Set(encrypted_key);
        active.update(&db).await.expect("update tenant");
    } else {
        tenants::ActiveModel {
            id: Set(tenant_id),
            name: Set("Dev Tenant".into()),
            slug: Set("dev".into()),
            encryption_key: Set(encrypted_key),
            ..Default::default()
        }
        .insert(&db)
        .await
        .expect("insert tenant");
    }

    (db, cfg, tenant_id, tenant_key_plain.to_string())
}

async fn create_test_user(
    db: &sea_orm::DatabaseConnection,
    cfg: &Config,
    tenant_id: Uuid,
    tenant_key: &str,
    email: &str,
    password: &str,
) -> ovlt_core::entity::users::Model {
    let _ = db
        .execute_unprepared(&format!("SET app.tenant_id = '{tenant_id}'"))
        .await;
    let lookup = hefesto::hash_for_lookup(email, tenant_key).expect("hash lookup");
    let _ = db
        .execute_unprepared(&format!(
            "DELETE FROM users WHERE tenant_id = '{tenant_id}' AND email_lookup = '{lookup}'"
        ))
        .await;

    let txn = db::begin_tenant_txn(db, tenant_id).await.unwrap();
    let user = user_service::create(
        &txn,
        user_service::CreateUserInput {
            tenant_id,
            email_encrypted: hefesto::encrypt(email, tenant_key, &cfg.master_encryption_key)
                .unwrap(),
            email_lookup: lookup,
            password_hash: hefesto::hash_password(password).unwrap(),
        },
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();
    user
}

// ── Phase 1-2: token generation & refresh rotation ───────────────────────────

#[tokio::test]
async fn test_register_and_login() {
    let (db, cfg, tenant_id, tenant_key) = setup().await;

    let email = "integration@ovlt.dev";
    let password = "Test1234!";

    let user = create_test_user(&db, &cfg, tenant_id, &tenant_key, email, password).await;
    assert_eq!(user.tenant_id, tenant_id);

    let email_lookup = hefesto::hash_for_lookup(email, &tenant_key).expect("hash");
    let txn = db::begin_tenant_txn(&db, tenant_id).await.expect("txn");
    let found = user_service::find_by_email_lookup(&txn, &email_lookup)
        .await
        .expect("find")
        .expect("user exists");
    txn.commit().await.unwrap();

    assert!(hefesto::verify_password(password, &found.password_hash));
    assert_eq!(found.id, user.id);

    let token = token_service::generate_access_token(
        user.id,
        tenant_id,
        email,
        vec![],
        vec![],
        HashMap::new(),
        &cfg.jwt_secret,
        cfg.jwt_expiration_minutes,
    )
    .expect("generate token");

    let claims =
        token_service::validate_access_token(&token, &cfg.jwt_secret, None).expect("validate token");

    assert_eq!(claims.sub, user.id.to_string());
    assert_eq!(claims.tid, tenant_id.to_string());
    assert_eq!(claims.email, email);
    assert!(!claims.jti.is_empty());
}

#[tokio::test]
async fn test_me_endpoint_logic() {
    let (db, cfg, tenant_id, tenant_key) = setup().await;

    let email = "me_test@ovlt.dev";
    let password = "Secret5678!";

    let user = create_test_user(&db, &cfg, tenant_id, &tenant_key, email, password).await;

    let token = token_service::generate_access_token(
        user.id,
        tenant_id,
        email,
        vec![],
        vec![],
        HashMap::new(),
        &cfg.jwt_secret,
        cfg.jwt_expiration_minutes,
    )
    .unwrap();

    let claims = token_service::validate_access_token(&token, &cfg.jwt_secret, None).unwrap();
    assert_eq!(claims.sub, user.id.to_string());

    let txn = db::begin_tenant_txn(&db, tenant_id).await.unwrap();
    let fetched = ovlt_core::entity::users::Entity::find_by_id(user.id)
        .one(&txn)
        .await
        .unwrap()
        .expect("user found");
    txn.commit().await.unwrap();

    let decrypted =
        hefesto::decrypt(&fetched.email, &tenant_key, &cfg.master_encryption_key).unwrap();
    assert_eq!(decrypted, email);
}

#[tokio::test]
async fn test_refresh_token_rotation() {
    let (db, cfg, tenant_id, tenant_key) = setup().await;

    let user = create_test_user(
        &db,
        &cfg,
        tenant_id,
        &tenant_key,
        "refresh@ovlt.dev",
        "Pass1234!",
    )
    .await;

    let rt1 = token_service::generate_refresh_token();
    let hash1 = token_service::hash_refresh_token(&rt1);
    let txn = db::begin_tenant_txn(&db, tenant_id).await.unwrap();
    token_service::store_refresh_token(&txn, tenant_id, user.id, hash1.clone(), 30)
        .await
        .unwrap();
    txn.commit().await.unwrap();

    let txn = db::begin_tenant_txn(&db, tenant_id).await.unwrap();
    let record = token_service::find_valid_refresh_token(&txn, &hash1)
        .await
        .unwrap()
        .expect("token found");
    token_service::revoke_token(&txn, record).await.unwrap();

    let second = token_service::find_valid_refresh_token(&txn, &hash1)
        .await
        .unwrap();
    assert!(second.is_none(), "rotated token must not be reusable");
    txn.commit().await.unwrap();
}

#[tokio::test]
async fn test_revoke_all_tokens() {
    let (db, cfg, tenant_id, tenant_key) = setup().await;

    let user = create_test_user(
        &db,
        &cfg,
        tenant_id,
        &tenant_key,
        "revoke@ovlt.dev",
        "Pass1234!",
    )
    .await;

    let txn = db::begin_tenant_txn(&db, tenant_id).await.unwrap();
    for _ in 0..3 {
        let rt = token_service::generate_refresh_token();
        let hash = token_service::hash_refresh_token(&rt);
        token_service::store_refresh_token(&txn, tenant_id, user.id, hash, 30)
            .await
            .unwrap();
    }
    txn.commit().await.unwrap();

    let txn = db::begin_tenant_txn(&db, tenant_id).await.unwrap();
    token_service::revoke_all_user_tokens(&txn, user.id)
        .await
        .unwrap();
    txn.commit().await.unwrap();

    use ovlt_core::entity::refresh_tokens;
    use sea_orm::{ColumnTrait, QueryFilter};
    let active: Vec<refresh_tokens::Model> = refresh_tokens::Entity::find()
        .filter(refresh_tokens::Column::UserId.eq(user.id))
        .filter(refresh_tokens::Column::RevokedAt.is_null())
        .all(&db)
        .await
        .unwrap();
    assert!(active.is_empty(), "all tokens must be revoked");
}

#[tokio::test]
async fn test_oauth_state_roundtrip() {
    dotenvy::dotenv().ok();
    let cfg = Config::from_env().expect("config");
    let tenant_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

    let state = oauth_service::generate_state(tenant_id, &cfg.jwt_secret);
    let recovered = oauth_service::verify_state(&state, &cfg.jwt_secret);
    assert_eq!(recovered, Some(tenant_id));

    let mut bad = state.clone();
    bad.push('x');
    assert_eq!(oauth_service::verify_state(&bad, &cfg.jwt_secret), None);
    assert_eq!(oauth_service::verify_state(&state, "wrong_secret"), None);
}

// ── Phase 4: user lifecycle ───────────────────────────────────────────────────

#[tokio::test]
async fn test_lockout_and_clear() {
    let (db, _cfg, tenant_id, tenant_key) = setup().await;
    let email = "lockout_test@ovlt.dev";
    let lookup = hefesto::hash_for_lookup(email, &tenant_key).expect("hash");

    lockout_service::clear_attempts(&db, tenant_id, &lookup)
        .await
        .unwrap();

    // Not locked initially
    assert!(!lockout_service::is_locked(&db, tenant_id, &lookup, 3, 15)
        .await
        .unwrap());

    // Record 3 attempts
    for _ in 0..3 {
        lockout_service::record_attempt(&db, tenant_id, &lookup)
            .await
            .unwrap();
    }

    // Now locked
    assert!(lockout_service::is_locked(&db, tenant_id, &lookup, 3, 15)
        .await
        .unwrap());

    // Clear resets it
    lockout_service::clear_attempts(&db, tenant_id, &lookup)
        .await
        .unwrap();
    assert!(!lockout_service::is_locked(&db, tenant_id, &lookup, 3, 15)
        .await
        .unwrap());
}

#[tokio::test]
async fn test_one_time_token_consume() {
    let (db, cfg, tenant_id, tenant_key) = setup().await;
    let user = create_test_user(
        &db,
        &cfg,
        tenant_id,
        &tenant_key,
        "ott@ovlt.dev",
        "Pass1234!",
    )
    .await;

    let token = one_time_token_service::generate();
    let hash = one_time_token_service::hash(&token);

    one_time_token_service::store(
        &db,
        tenant_id,
        user.id,
        hash,
        ovlt_core::entity::one_time_tokens::TYPE_PASSWORD_RESET,
        60,
    )
    .await
    .unwrap();

    // First consume succeeds
    let record = one_time_token_service::consume(
        &db,
        &token,
        ovlt_core::entity::one_time_tokens::TYPE_PASSWORD_RESET,
    )
    .await
    .expect("first consume");
    assert_eq!(record.user_id, user.id);

    // Second consume fails (already used)
    let second = one_time_token_service::consume(
        &db,
        &token,
        ovlt_core::entity::one_time_tokens::TYPE_PASSWORD_RESET,
    )
    .await;
    assert!(second.is_err(), "token must not be reusable");
}

#[tokio::test]
async fn test_otp_email_verification() {
    let (db, cfg, tenant_id, tenant_key) = setup().await;
    let user = create_test_user(
        &db,
        &cfg,
        tenant_id,
        &tenant_key,
        "otp_verify@ovlt.dev",
        "Pass1234!",
    )
    .await;

    let otp = one_time_token_service::generate_otp();
    assert_eq!(otp.len(), 6);
    assert!(otp.chars().all(|c| c.is_ascii_digit()));

    one_time_token_service::store_otp(&db, tenant_id, user.id, &otp, 1)
        .await
        .unwrap();

    let record = one_time_token_service::consume_otp(&db, user.id, &otp)
        .await
        .expect("otp consumed");
    assert_eq!(record.user_id, user.id);

    // Second use fails
    let second = one_time_token_service::consume_otp(&db, user.id, &otp).await;
    assert!(second.is_err());
}

// ── Phase 6: MFA / TOTP ───────────────────────────────────────────────────────

#[tokio::test]
async fn test_totp_verify_correct_code() {
    let secret = mfa_service::generate_secret();
    assert!(!secret.is_empty());

    // Generate the current TOTP code and verify it
    let secret_bytes =
        base32::decode(base32::Alphabet::RFC4648 { padding: false }, &secret).expect("decode");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let code = totp_lite::totp_custom::<sha1::Sha1>(totp_lite::DEFAULT_STEP, 6, &secret_bytes, now);

    assert!(
        mfa_service::verify_code(&secret, &code),
        "current code must verify"
    );
}

#[tokio::test]
async fn test_totp_rejects_wrong_code() {
    let secret = mfa_service::generate_secret();
    assert!(!mfa_service::verify_code(&secret, "000000"));
    assert!(!mfa_service::verify_code(&secret, "999999"));
    assert!(!mfa_service::verify_code(&secret, "abc123"));
}

#[tokio::test]
async fn test_mfa_token_roundtrip() {
    dotenvy::dotenv().ok();
    let cfg = Config::from_env().expect("config");
    let user_id = Uuid::new_v4();
    let tenant_id = Uuid::new_v4();

    let token = token_service::generate_mfa_token(user_id, tenant_id, &cfg.jwt_secret)
        .expect("generate mfa token");

    let claims =
        token_service::verify_mfa_token(&token, &cfg.jwt_secret, None).expect("verify mfa token");

    assert_eq!(claims.sub, user_id.to_string());
    assert_eq!(claims.tid, tenant_id.to_string());
    assert_eq!(claims.purpose, "mfa_challenge");

    // Wrong secret must fail
    let bad = token_service::verify_mfa_token(&token, "wrong_secret_that_is_long_enough_32c", None);
    assert!(bad.is_err());
}

#[tokio::test]
async fn test_mfa_totp_uri_format() {
    let secret = mfa_service::generate_secret();
    let uri = mfa_service::totp_uri(&secret, "user@example.com", "OVLT");

    assert!(uri.starts_with("otpauth://totp/"));
    assert!(uri.contains(&secret));
    assert!(uri.contains("digits=6"));
    assert!(uri.contains("period=30"));
}
