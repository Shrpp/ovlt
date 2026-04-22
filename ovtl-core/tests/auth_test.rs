use ovtl_core::{
    config::Config,
    db,
    entity::tenants,
    services::{oauth_service, token_service, user_service},
};
use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};
use uuid::Uuid;

async fn setup() -> (sea_orm::DatabaseConnection, Config, Uuid) {
    dotenvy::dotenv().ok();
    let cfg = Config::from_env().expect("config");
    let db = db::connect(&cfg.database_url).await.expect("db");

    // Encrypt a real tenant key and upsert a dev tenant
    let tenant_key_plain = "dev-test-tenant-key-32chars-long!";
    let encrypted_key = hefesto::encrypt(
        tenant_key_plain,
        &cfg.master_encryption_key,
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

    (db, cfg, tenant_id)
}

#[tokio::test]
async fn test_register_and_login() {
    let (db, cfg, tenant_id) = setup().await;

    let tenant = tenants::Entity::find_by_id(tenant_id)
        .one(&db)
        .await
        .expect("find")
        .expect("tenant exists");

    let tenant_key = hefesto::decrypt(
        &tenant.encryption_key,
        &cfg.master_encryption_key,
        &cfg.master_encryption_key,
    )
    .expect("decrypt tenant key");

    let email = "integration@ovtl.dev";
    let password = "Test1234!";

    // Cleanup previous run
    let _ = db
        .execute_unprepared(&format!("SET app.tenant_id = '{tenant_id}'"))
        .await;
    let _ = db
        .execute_unprepared(&format!(
            "DELETE FROM users WHERE tenant_id = '{tenant_id}' AND email_lookup = '{}'",
            hefesto::hash_for_lookup(email, &tenant_key)
        ))
        .await;

    // Register
    let email_lookup = hefesto::hash_for_lookup(email, &tenant_key);
    let email_encrypted = hefesto::encrypt(email, &tenant_key, &cfg.master_encryption_key)
        .expect("encrypt email");
    let password_hash = hefesto::hash_password(password).expect("hash password");

    let txn = db::begin_tenant_txn(&db, tenant_id).await.expect("begin txn");
    let user = ovtl_core::services::user_service::create(
        &txn,
        ovtl_core::services::user_service::CreateUserInput {
            tenant_id,
            email_encrypted,
            email_lookup: email_lookup.clone(),
            password_hash,
        },
    )
    .await
    .expect("create user");
    txn.commit().await.expect("commit");

    assert_eq!(user.tenant_id, tenant_id);

    // Login — find user and verify password
    let txn = db::begin_tenant_txn(&db, tenant_id).await.expect("begin txn");
    let found = ovtl_core::services::user_service::find_by_email_lookup(&txn, &email_lookup)
        .await
        .expect("find user")
        .expect("user exists");
    txn.commit().await.expect("commit");

    assert!(hefesto::verify_password(password, &found.password_hash));
    assert_eq!(found.id, user.id);

    // JWT round-trip
    let token = token_service::generate_access_token(
        user.id,
        tenant_id,
        email,
        &cfg.jwt_secret,
        cfg.jwt_expiration_minutes,
    )
    .expect("generate token");

    let claims = token_service::validate_access_token(&token, &cfg.jwt_secret)
        .expect("validate token");

    assert_eq!(claims.sub, user.id.to_string());
    assert_eq!(claims.tid, tenant_id.to_string());
    assert_eq!(claims.email, email);

    println!("✓ Register, login, JWT round-trip OK");
    println!("  Tenant ID : {tenant_id}");
    println!("  User   ID : {}", user.id);
    println!("  Token     : {}...{}", &token[..20], &token[token.len()-10..]);
}

#[tokio::test]
async fn test_me_endpoint_logic() {
    let (db, cfg, tenant_id) = setup().await;

    let tenant = tenants::Entity::find_by_id(tenant_id)
        .one(&db)
        .await
        .expect("find")
        .expect("tenant exists");

    let tenant_key = hefesto::decrypt(
        &tenant.encryption_key,
        &cfg.master_encryption_key,
        &cfg.master_encryption_key,
    )
    .expect("decrypt tenant key");

    let email = "me_test@ovtl.dev";
    let password = "Secret5678!";

    // Cleanup
    let _ = db
        .execute_unprepared(&format!("SET app.tenant_id = '{tenant_id}'"))
        .await;
    let _ = db
        .execute_unprepared(&format!(
            "DELETE FROM users WHERE tenant_id = '{tenant_id}' AND email_lookup = '{}'",
            hefesto::hash_for_lookup(email, &tenant_key)
        ))
        .await;

    // Register
    let txn = db::begin_tenant_txn(&db, tenant_id).await.unwrap();
    let user = ovtl_core::services::user_service::create(
        &txn,
        ovtl_core::services::user_service::CreateUserInput {
            tenant_id,
            email_encrypted: hefesto::encrypt(email, &tenant_key, &cfg.master_encryption_key)
                .unwrap(),
            email_lookup: hefesto::hash_for_lookup(email, &tenant_key),
            password_hash: hefesto::hash_password(password).unwrap(),
        },
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();

    // Generate token
    let token = token_service::generate_access_token(
        user.id,
        tenant_id,
        email,
        &cfg.jwt_secret,
        cfg.jwt_expiration_minutes,
    )
    .unwrap();

    // Validate token and verify AuthUser fields
    let claims = token_service::validate_access_token(&token, &cfg.jwt_secret).unwrap();
    assert_eq!(claims.sub, user.id.to_string());
    assert_eq!(claims.tid, tenant_id.to_string());
    assert_eq!(claims.email, email);

    // Simulate what /users/me does: fetch user + decrypt email
    let txn = db::begin_tenant_txn(&db, tenant_id).await.unwrap();
    let fetched = ovtl_core::entity::users::Entity::find_by_id(user.id)
        .one(&txn)
        .await
        .unwrap()
        .expect("user found");
    txn.commit().await.unwrap();

    let decrypted_email =
        hefesto::decrypt(&fetched.email, &tenant_key, &cfg.master_encryption_key).unwrap();
    assert_eq!(decrypted_email, email);

    println!("✓ /users/me logic OK — decrypt email roundtrip verified");
}

async fn create_test_user(
    db: &sea_orm::DatabaseConnection,
    cfg: &Config,
    tenant_id: Uuid,
    tenant_key: &str,
    email: &str,
    password: &str,
) -> ovtl_core::entity::users::Model {
    // cleanup
    let _ = db
        .execute_unprepared(&format!("SET app.tenant_id = '{tenant_id}'"))
        .await;
    let _ = db
        .execute_unprepared(&format!(
            "DELETE FROM users WHERE tenant_id = '{tenant_id}' AND email_lookup = '{}'",
            hefesto::hash_for_lookup(email, tenant_key)
        ))
        .await;

    let txn = db::begin_tenant_txn(db, tenant_id).await.unwrap();
    let user = user_service::create(
        &txn,
        user_service::CreateUserInput {
            tenant_id,
            email_encrypted: hefesto::encrypt(email, tenant_key, &cfg.master_encryption_key)
                .unwrap(),
            email_lookup: hefesto::hash_for_lookup(email, tenant_key),
            password_hash: hefesto::hash_password(password).unwrap(),
        },
    )
    .await
    .unwrap();
    txn.commit().await.unwrap();
    user
}

#[tokio::test]
async fn test_refresh_token_rotation() {
    let (db, cfg, tenant_id) = setup().await;
    let tenant = tenants::Entity::find_by_id(tenant_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let tenant_key = hefesto::decrypt(
        &tenant.encryption_key,
        &cfg.master_encryption_key,
        &cfg.master_encryption_key,
    )
    .unwrap();

    let user =
        create_test_user(&db, &cfg, tenant_id, &tenant_key, "refresh@ovtl.dev", "Pass1234!").await;

    // Store a refresh token
    let rt1 = token_service::generate_refresh_token();
    let hash1 = token_service::hash_refresh_token(&rt1);
    let txn = db::begin_tenant_txn(&db, tenant_id).await.unwrap();
    token_service::store_refresh_token(&txn, tenant_id, user.id, hash1.clone(), 30)
        .await
        .unwrap();
    txn.commit().await.unwrap();

    // Use it — should find and rotate
    let txn = db::begin_tenant_txn(&db, tenant_id).await.unwrap();
    let record = token_service::find_valid_refresh_token(&txn, &hash1)
        .await
        .unwrap()
        .expect("token found");
    token_service::revoke_token(&txn, record).await.unwrap();

    // Second lookup must fail (already revoked)
    let second = token_service::find_valid_refresh_token(&txn, &hash1)
        .await
        .unwrap();
    assert!(second.is_none(), "rotated token must not be reusable");
    txn.commit().await.unwrap();

    println!("✓ Refresh token rotation OK");
}

#[tokio::test]
async fn test_revoke_all_tokens() {
    let (db, cfg, tenant_id) = setup().await;
    let tenant = tenants::Entity::find_by_id(tenant_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let tenant_key = hefesto::decrypt(
        &tenant.encryption_key,
        &cfg.master_encryption_key,
        &cfg.master_encryption_key,
    )
    .unwrap();

    let user = create_test_user(
        &db, &cfg, tenant_id, &tenant_key, "revoke@ovtl.dev", "Pass1234!",
    )
    .await;

    // Store 3 tokens
    let txn = db::begin_tenant_txn(&db, tenant_id).await.unwrap();
    for _ in 0..3 {
        let rt = token_service::generate_refresh_token();
        let hash = token_service::hash_refresh_token(&rt);
        token_service::store_refresh_token(&txn, tenant_id, user.id, hash, 30)
            .await
            .unwrap();
    }
    txn.commit().await.unwrap();

    // Revoke all
    let txn = db::begin_tenant_txn(&db, tenant_id).await.unwrap();
    token_service::revoke_all_user_tokens(&txn, user.id)
        .await
        .unwrap();
    txn.commit().await.unwrap();

    // Verify all are revoked
    use ovtl_core::entity::refresh_tokens;
    use sea_orm::{ColumnTrait, QueryFilter};
    let active: Vec<refresh_tokens::Model> = refresh_tokens::Entity::find()
        .filter(refresh_tokens::Column::UserId.eq(user.id))
        .filter(refresh_tokens::Column::RevokedAt.is_null())
        .all(&db)
        .await
        .unwrap();
    assert!(active.is_empty(), "all tokens must be revoked");

    println!("✓ Revoke all tokens OK");
}

#[tokio::test]
async fn test_oauth_state_roundtrip() {
    dotenvy::dotenv().ok();
    let cfg = Config::from_env().expect("config");
    let tenant_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

    // State encodes tenant_id and survives a round-trip
    let state = oauth_service::generate_state(tenant_id, &cfg.jwt_secret);
    let recovered = oauth_service::verify_state(&state, &cfg.jwt_secret);
    assert_eq!(recovered, Some(tenant_id), "state must decode to original tenant_id");

    // Tampered state must be rejected
    let mut bad = state.clone();
    bad.push('x');
    assert_eq!(oauth_service::verify_state(&bad, &cfg.jwt_secret), None);

    // Wrong secret must be rejected
    assert_eq!(oauth_service::verify_state(&state, "wrong_secret"), None);

    println!("✓ OAuth HMAC state roundtrip OK");
}
