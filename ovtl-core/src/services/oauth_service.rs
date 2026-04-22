use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64URL, Engine};
use hmac::{Hmac, Mac};
use oauth2::{basic::BasicClient, AuthUrl, ClientId, ClientSecret, CsrfToken, RedirectUrl, Scope, TokenUrl};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use sha2::Sha256;
use uuid::Uuid;

use crate::{
    config::Config,
    entity::{oauth_accounts, users},
    error::AppError,
    services::user_service,
};

// ── State (CSRF + tenant encoding) ───────────────────────────────────────────

/// Generates an OAuth state that encodes tenant_id and a nonce, signed with HMAC-SHA256.
/// Format (before base64): `{tenant_id}:{nonce}:{hmac_hex}`
pub fn generate_state(tenant_id: Uuid, jwt_secret: &str) -> String {
    let nonce = Uuid::new_v4().to_string();
    let payload = format!("{tenant_id}:{nonce}");
    let mac = hmac_sign(&payload, jwt_secret);
    B64URL.encode(format!("{payload}:{mac}"))
}

/// Returns `Some(tenant_id)` if the state is valid, `None` otherwise.
pub fn verify_state(state: &str, jwt_secret: &str) -> Option<Uuid> {
    let decoded = String::from_utf8(B64URL.decode(state).ok()?).ok()?;
    // format: {tenant_id}:{nonce}:{hmac}
    let last_colon = decoded.rfind(':')?;
    let (payload, provided_mac) = decoded.split_at(last_colon);
    let provided_mac = &provided_mac[1..]; // drop the leading ':'

    let expected_mac = hmac_sign(payload, jwt_secret);
    if provided_mac != expected_mac {
        return None;
    }

    let tenant_id_str = payload.split(':').next()?;
    Uuid::parse_str(tenant_id_str).ok()
}

fn hmac_sign(msg: &str, key: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(key.as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(msg.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

// ── OAuth client builder ──────────────────────────────────────────────────────

pub fn build_authorize_url(
    provider: &str,
    config: &Config,
    tenant_id: Uuid,
) -> Result<(String, String), AppError> {
    let (auth_url_str, token_url_str, scopes) = provider_urls(provider)?;
    let oauth_cfg = config
        .oauth_for(provider)
        .ok_or_else(|| AppError::InvalidInput(format!("{provider} OAuth not configured")))?;

    let state_value = generate_state(tenant_id, &config.jwt_secret);

    let client = BasicClient::new(
        ClientId::new(oauth_cfg.client_id.clone()),
        Some(ClientSecret::new(oauth_cfg.client_secret.clone())),
        AuthUrl::new(auth_url_str.to_string()).map_err(|e| AppError::InvalidInput(e.to_string()))?,
        Some(
            TokenUrl::new(token_url_str.to_string())
                .map_err(|e| AppError::InvalidInput(e.to_string()))?,
        ),
    )
    .set_redirect_uri(
        RedirectUrl::new(oauth_cfg.redirect_url.clone())
            .map_err(|e| AppError::InvalidInput(e.to_string()))?,
    );

    let state_clone = state_value.clone();
    let (url, _) = client
        .authorize_url(move || CsrfToken::new(state_clone))
        .add_scopes(scopes.into_iter().map(|s| Scope::new(s.to_string())))
        .url();

    Ok((url.to_string(), state_value))
}

fn provider_urls(provider: &str) -> Result<(&'static str, &'static str, Vec<&'static str>), AppError> {
    match provider {
        "google" => Ok((
            "https://accounts.google.com/o/oauth2/v2/auth",
            "https://oauth2.googleapis.com/token",
            vec!["openid", "email", "profile"],
        )),
        "github" => Ok((
            "https://github.com/login/oauth/authorize",
            "https://github.com/login/oauth/access_token",
            vec!["user:email"],
        )),
        other => Err(AppError::InvalidInput(format!("unknown provider: {other}"))),
    }
}

// ── Token exchange ────────────────────────────────────────────────────────────

pub async fn exchange_code(
    provider: &str,
    code: &str,
    config: &Config,
) -> Result<String, AppError> {
    let oauth_cfg = config
        .oauth_for(provider)
        .ok_or_else(|| AppError::InvalidInput(format!("{provider} OAuth not configured")))?;

    let (_, token_url, _) = provider_urls(provider)?;

    let client = reqwest::Client::new();

    match provider {
        "google" => {
            #[derive(Deserialize)]
            struct GoogleToken {
                access_token: String,
            }
            let resp: GoogleToken = client
                .post(token_url)
                .form(&[
                    ("code", code),
                    ("client_id", &oauth_cfg.client_id),
                    ("client_secret", &oauth_cfg.client_secret),
                    ("redirect_uri", &oauth_cfg.redirect_url),
                    ("grant_type", "authorization_code"),
                ])
                .send()
                .await
                .map_err(|e| AppError::InvalidInput(e.to_string()))?
                .json()
                .await
                .map_err(|e| AppError::InvalidInput(e.to_string()))?;
            Ok(resp.access_token)
        }
        "github" => {
            #[derive(Deserialize)]
            struct GithubToken {
                access_token: String,
            }
            let resp: GithubToken = client
                .post(token_url)
                .header("Accept", "application/json")
                .form(&[
                    ("code", code),
                    ("client_id", &oauth_cfg.client_id),
                    ("client_secret", &oauth_cfg.client_secret),
                    ("redirect_uri", &oauth_cfg.redirect_url),
                ])
                .send()
                .await
                .map_err(|e| AppError::InvalidInput(e.to_string()))?
                .json()
                .await
                .map_err(|e| AppError::InvalidInput(e.to_string()))?;
            Ok(resp.access_token)
        }
        _ => Err(AppError::InvalidInput(format!("unknown provider: {provider}"))),
    }
}

// ── Profile fetching ──────────────────────────────────────────────────────────

pub struct OAuthProfile {
    pub provider_user_id: String,
    pub email: String,
}

pub async fn fetch_profile(provider: &str, access_token: &str) -> Result<OAuthProfile, AppError> {
    match provider {
        "google" => fetch_google_profile(access_token).await,
        "github" => fetch_github_profile(access_token).await,
        other => Err(AppError::InvalidInput(format!("unknown provider: {other}"))),
    }
}

async fn fetch_google_profile(access_token: &str) -> Result<OAuthProfile, AppError> {
    #[derive(Deserialize)]
    struct GoogleProfile {
        sub: String,
        email: String,
    }
    let profile: GoogleProfile = reqwest::Client::new()
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| AppError::InvalidInput(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    Ok(OAuthProfile {
        provider_user_id: profile.sub,
        email: profile.email,
    })
}

async fn fetch_github_profile(access_token: &str) -> Result<OAuthProfile, AppError> {
    #[derive(Deserialize)]
    struct GithubUser {
        id: i64,
        email: Option<String>,
    }
    #[derive(Deserialize)]
    struct GithubEmail {
        email: String,
        primary: bool,
        verified: bool,
    }

    let client = reqwest::Client::new();

    let user: GithubUser = client
        .get("https://api.github.com/user")
        .bearer_auth(access_token)
        .header("User-Agent", "ovtl-core")
        .send()
        .await
        .map_err(|e| AppError::InvalidInput(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::InvalidInput(e.to_string()))?;

    // GitHub may not expose email on the user object — fetch from /user/emails
    let email = if let Some(e) = user.email {
        e
    } else {
        let emails: Vec<GithubEmail> = client
            .get("https://api.github.com/user/emails")
            .bearer_auth(access_token)
            .header("User-Agent", "ovtl-core")
            .send()
            .await
            .map_err(|e| AppError::InvalidInput(e.to_string()))?
            .json()
            .await
            .map_err(|e| AppError::InvalidInput(e.to_string()))?;

        emails
            .into_iter()
            .find(|e| e.primary && e.verified)
            .ok_or_else(|| AppError::InvalidInput("no verified primary email".into()))?
            .email
    };

    Ok(OAuthProfile {
        provider_user_id: user.id.to_string(),
        email,
    })
}

// ── Find or create user ───────────────────────────────────────────────────────

pub async fn find_or_create_user(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    tenant_key: &str,
    master_key: &str,
    provider: &str,
    profile: &OAuthProfile,
) -> Result<users::Model, AppError> {
    // 1. Check if this OAuth account is already linked
    let existing_account = oauth_accounts::Entity::find()
        .filter(oauth_accounts::Column::Provider.eq(provider))
        .filter(oauth_accounts::Column::ProviderUserId.eq(&profile.provider_user_id))
        .one(txn)
        .await?;

    if let Some(account) = existing_account {
        let user = users::Entity::find_by_id(account.user_id)
            .one(txn)
            .await?
            .ok_or(AppError::NotFound)?;
        return Ok(user);
    }

    // 2. Check if a local user exists with this email
    let email_lookup = hefesto::hash_for_lookup(&profile.email, tenant_key);
    let existing_user =
        user_service::find_by_email_lookup(txn, &email_lookup).await?;

    let user = if let Some(u) = existing_user {
        u
    } else {
        // 3. Create new user — OAuth-only users get an unguessable password hash
        let random_pw = format!("{}{}", Uuid::new_v4(), Uuid::new_v4());
        let password_hash = hefesto::hash_password(&random_pw)?;
        let email_encrypted = hefesto::encrypt(&profile.email, tenant_key, master_key)?;

        user_service::create(
            txn,
            user_service::CreateUserInput {
                tenant_id,
                email_encrypted,
                email_lookup,
                password_hash,
            },
        )
        .await?
    };

    // 4. Link the OAuth account
    oauth_accounts::ActiveModel {
        tenant_id: Set(tenant_id),
        user_id: Set(user.id),
        provider: Set(provider.to_string()),
        provider_user_id: Set(profile.provider_user_id.clone()),
        ..Default::default()
    }
    .insert(txn)
    .await?;

    Ok(user)
}
