#![allow(dead_code)]
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("API error {status}: {message}")]
    Api { status: u16, message: String },
}

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Clone)]
pub struct Client {
    inner: reqwest::Client,
    pub base_url: String,
    token: Option<String>,
}

impl Client {
    pub fn new(base_url: String) -> Self {
        Self {
            inner: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
            base_url,
            token: None,
        }
    }

    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    fn auth_headers(&self) -> reqwest::header::HeaderMap {
        let mut map = reqwest::header::HeaderMap::new();
        if let Some(token) = &self.token {
            map.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {token}").parse().unwrap(),
            );
        }
        map
    }

    fn tenant_headers(&self, tenant_id: &str) -> reqwest::header::HeaderMap {
        let mut map = self.auth_headers();
        map.insert("x-ovtl-tenant-id", tenant_id.parse().unwrap());
        map
    }

    async fn check<T: for<'de> Deserialize<'de>>(
        &self,
        resp: reqwest::Response,
    ) -> ApiResult<T> {
        let status = resp.status();
        if status.is_success() {
            Ok(resp.json::<T>().await?)
        } else {
            let message = resp
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| v["error"].as_str().map(|s| s.to_owned()))
                .unwrap_or_else(|| status.to_string());
            Err(ApiError::Api {
                status: status.as_u16(),
                message,
            })
        }
    }

    // ── Auth ──────────────────────────────────────────────────────────────────

    pub async fn login(&self, email: &str, password: &str, slug: &str) -> ApiResult<LoginResult> {
        #[derive(Deserialize)]
        struct LoginResp {
            access_token: Option<String>,
            mfa_required: Option<bool>,
            mfa_token: Option<String>,
        }
        let resp = self
            .inner
            .post(format!("{}/auth/login", self.base_url))
            .header("x-ovtl-tenant-slug", slug)
            .json(&serde_json::json!({ "email": email, "password": password }))
            .send()
            .await?;
        let body: LoginResp = self.check(resp).await?;
        if body.mfa_required == Some(true) {
            if let Some(mfa_token) = body.mfa_token {
                return Ok(LoginResult::MfaRequired { mfa_token });
            }
        }
        Ok(LoginResult::Token(body.access_token.unwrap_or_default()))
    }

    pub async fn mfa_challenge(&self, slug: &str, mfa_token: &str, code: &str) -> ApiResult<String> {
        #[derive(Deserialize)]
        struct TokenResp {
            access_token: String,
        }
        let resp = self
            .inner
            .post(format!("{}/auth/mfa/challenge", self.base_url))
            .header("x-ovtl-tenant-slug", slug)
            .json(&serde_json::json!({ "mfa_token": mfa_token, "code": code }))
            .send()
            .await?;
        let body: TokenResp = self.check(resp).await?;
        Ok(body.access_token)
    }

    pub async fn admin_disable_user_mfa(&self, tenant_id: &str, user_id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .delete(format!("{}/users/{}/mfa", self.base_url, user_id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "disable mfa failed".into() })
        }
    }

    // ── Tenants ───────────────────────────────────────────────────────────────

    pub async fn list_tenant_slugs(&self) -> ApiResult<Vec<(String, String)>> {
        #[derive(Deserialize)]
        struct Entry { slug: String, name: String }
        let resp = self
            .inner
            .get(format!("{}/tenants/slugs", self.base_url))
            .send()
            .await?;
        let entries: Vec<Entry> = self.check(resp).await?;
        Ok(entries.into_iter().map(|e| (e.slug, e.name)).collect())
    }

    pub async fn list_tenants(&self) -> ApiResult<Vec<Tenant>> {
        let resp = self
            .inner
            .get(format!("{}/tenants", self.base_url))
            .headers(self.auth_headers())
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn create_tenant(&self, name: &str, slug: &str) -> ApiResult<Tenant> {
        let resp = self
            .inner
            .post(format!("{}/tenants", self.base_url))
            .headers(self.auth_headers())
            .json(&serde_json::json!({ "name": name, "slug": slug }))
            .send()
            .await?;
        self.check(resp).await
    }

    // ── Clients ───────────────────────────────────────────────────────────────

    pub async fn list_clients(&self, tenant_id: &str) -> ApiResult<Vec<OAuthClient>> {
        let resp = self
            .inner
            .get(format!("{}/clients", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn create_client(
        &self,
        tenant_id: &str,
        name: &str,
        redirect_uris: Vec<String>,
        scopes: Vec<String>,
        is_confidential: bool,
        grant_types: Vec<String>,
        access_token_ttl_minutes: Option<i32>,
        refresh_token_ttl_days: Option<i32>,
    ) -> ApiResult<OAuthClient> {
        let resp = self
            .inner
            .post(format!("{}/clients", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({
                "name": name,
                "redirect_uris": redirect_uris,
                "scopes": scopes,
                "is_confidential": is_confidential,
                "grant_types": grant_types,
                "access_token_ttl_minutes": access_token_ttl_minutes,
                "refresh_token_ttl_days": refresh_token_ttl_days,
            }))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn update_client(
        &self,
        tenant_id: &str,
        id: &str,
        name: &str,
        redirect_uris: Vec<String>,
        scopes: Vec<String>,
        access_token_ttl_minutes: Option<i32>,
        refresh_token_ttl_days: Option<i32>,
        is_confidential: bool,
        grant_types: Vec<String>,
    ) -> ApiResult<OAuthClient> {
        let resp = self
            .inner
            .put(format!("{}/clients/{}", self.base_url, id))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({
                "name": name,
                "redirect_uris": redirect_uris,
                "scopes": scopes,
                "access_token_ttl_minutes": access_token_ttl_minutes,
                "refresh_token_ttl_days": refresh_token_ttl_days,
                "is_confidential": is_confidential,
                "grant_types": grant_types,
            }))
            .send()
            .await?;
        self.check(resp).await
    }

    // ── Client Roles ──────────────────────────────────────────────────────────

    pub async fn list_client_roles(&self, tenant_id: &str, client_id: &str) -> ApiResult<Vec<Role>> {
        let resp = self
            .inner
            .get(format!("{}/clients/{}/roles", self.base_url, client_id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn assign_client_role(&self, tenant_id: &str, client_id: &str, role_id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .post(format!("{}/clients/{}/roles", self.base_url, client_id))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({ "role_id": role_id }))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "assign failed".into() })
        }
    }

    pub async fn revoke_client_role(&self, tenant_id: &str, client_id: &str, role_id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .delete(format!("{}/clients/{}/roles/{}", self.base_url, client_id, role_id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "revoke failed".into() })
        }
    }

    // ── Identity Providers ────────────────────────────────────────────────────

    pub async fn list_identity_providers(&self, tenant_id: &str) -> ApiResult<Vec<IdentityProvider>> {
        let resp = self
            .inner
            .get(format!("{}/identity-providers", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn create_identity_provider(
        &self,
        tenant_id: &str,
        provider: &str,
        client_id: &str,
        client_secret: &str,
        redirect_url: &str,
        scopes: Vec<String>,
    ) -> ApiResult<IdentityProvider> {
        let resp = self
            .inner
            .post(format!("{}/identity-providers", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({
                "provider": provider,
                "client_id": client_id,
                "client_secret": client_secret,
                "redirect_url": redirect_url,
                "scopes": scopes,
            }))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn update_identity_provider(
        &self,
        tenant_id: &str,
        id: &str,
        client_id: &str,
        client_secret: Option<&str>,
        redirect_url: &str,
        scopes: Vec<String>,
        enabled: bool,
    ) -> ApiResult<IdentityProvider> {
        let mut body = serde_json::json!({
            "client_id": client_id,
            "redirect_url": redirect_url,
            "scopes": scopes,
            "enabled": enabled,
        });
        if let Some(secret) = client_secret {
            body["client_secret"] = serde_json::Value::String(secret.to_string());
        }
        let resp = self
            .inner
            .put(format!("{}/identity-providers/{}", self.base_url, id))
            .headers(self.tenant_headers(tenant_id))
            .json(&body)
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn delete_identity_provider(&self, tenant_id: &str, id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .delete(format!("{}/identity-providers/{}", self.base_url, id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "delete failed".into() })
        }
    }

    pub async fn deactivate_client(&self, tenant_id: &str, id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .delete(format!("{}/clients/{}", self.base_url, id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(ApiError::Api {
                status: status.as_u16(),
                message: "deactivate failed".into(),
            })
        }
    }

    // ── Users ─────────────────────────────────────────────────────────────────

    pub async fn list_users(&self, tenant_id: &str) -> ApiResult<Vec<User>> {
        let resp = self
            .inner
            .get(format!("{}/users", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn create_user(
        &self,
        tenant_id: &str,
        email: &str,
        password: &str,
    ) -> ApiResult<User> {
        let resp = self
            .inner
            .post(format!("{}/users", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({ "email": email, "password": password }))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn deactivate_user(&self, tenant_id: &str, id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .delete(format!("{}/users/{}", self.base_url, id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(ApiError::Api {
                status: status.as_u16(),
                message: "deactivate failed".into(),
            })
        }
    }

    // ── Roles ─────────────────────────────────────────────────────────────────

    pub async fn list_roles(&self, tenant_id: &str) -> ApiResult<Vec<Role>> {
        let resp = self
            .inner
            .get(format!("{}/roles", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn create_role(&self, tenant_id: &str, name: &str, description: &str) -> ApiResult<Role> {
        let resp = self
            .inner
            .post(format!("{}/roles", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({ "name": name, "description": description }))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn update_role(&self, tenant_id: &str, role_id: &str, name: &str, description: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .put(format!("{}/roles/{}", self.base_url, role_id))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({ "name": name, "description": description }))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "update failed".into() })
        }
    }

    pub async fn delete_role(&self, tenant_id: &str, role_id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .delete(format!("{}/roles/{}", self.base_url, role_id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "delete failed".into() })
        }
    }

    pub async fn list_role_permissions(&self, tenant_id: &str, role_id: &str) -> ApiResult<Vec<Permission>> {
        let resp = self
            .inner
            .get(format!("{}/roles/{}/permissions", self.base_url, role_id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn assign_role_permission(&self, tenant_id: &str, role_id: &str, permission_id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .post(format!("{}/roles/{}/permissions", self.base_url, role_id))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({ "permission_id": permission_id }))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "assign failed".into() })
        }
    }

    pub async fn revoke_role_permission(&self, tenant_id: &str, role_id: &str, perm_id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .delete(format!("{}/roles/{}/permissions/{}", self.base_url, role_id, perm_id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "revoke failed".into() })
        }
    }

    pub async fn list_user_roles(&self, tenant_id: &str, user_id: &str) -> ApiResult<Vec<Role>> {
        let resp = self
            .inner
            .get(format!("{}/users/{}/roles", self.base_url, user_id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn assign_user_role(&self, tenant_id: &str, user_id: &str, role_id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .post(format!("{}/users/{}/roles", self.base_url, user_id))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({ "role_id": role_id }))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "assign failed".into() })
        }
    }

    pub async fn revoke_user_role(&self, tenant_id: &str, user_id: &str, role_id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .delete(format!("{}/users/{}/roles/{}", self.base_url, user_id, role_id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "revoke failed".into() })
        }
    }

    // ── Sessions ──────────────────────────────────────────────────────────────

    pub async fn list_sessions(&self, tenant_id: &str) -> ApiResult<Vec<Session>> {
        let resp = self
            .inner
            .get(format!("{}/sessions", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn delete_session(&self, tenant_id: &str, id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .delete(format!("{}/sessions/{}", self.base_url, id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(ApiError::Api {
                status: status.as_u16(),
                message: "delete failed".into(),
            })
        }
    }

    // ── Permissions ───────────────────────────────────────────────────────────

    pub async fn list_permissions(&self, tenant_id: &str) -> ApiResult<Vec<Permission>> {
        let resp = self
            .inner
            .get(format!("{}/permissions", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn create_permission(&self, tenant_id: &str, name: &str, description: &str) -> ApiResult<Permission> {
        let resp = self
            .inner
            .post(format!("{}/permissions", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({ "name": name, "description": description }))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn update_permission(&self, tenant_id: &str, perm_id: &str, name: &str, description: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .put(format!("{}/permissions/{}", self.base_url, perm_id))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({ "name": name, "description": description }))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "update failed".into() })
        }
    }

    pub async fn delete_permission(&self, tenant_id: &str, perm_id: &str) -> ApiResult<()> {
        let resp = self
            .inner
            .delete(format!("{}/permissions/{}", self.base_url, perm_id))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "delete failed".into() })
        }
    }

    pub async fn update_user_email(&self, tenant_id: &str, user_id: &str, email: &str, password: Option<&str>, is_active: bool) -> ApiResult<()> {
        let mut body = serde_json::json!({ "email": email, "is_active": is_active });
        if let Some(pw) = password {
            body["password"] = serde_json::Value::String(pw.to_string());
        }
        let resp = self
            .inner
            .put(format!("{}/users/{}", self.base_url, user_id))
            .headers(self.tenant_headers(tenant_id))
            .json(&body)
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() { Ok(()) } else {
            Err(ApiError::Api { status: status.as_u16(), message: "update failed".into() })
        }
    }

    // ── Health ────────────────────────────────────────────────────────────────

    pub async fn health(&self) -> ApiResult<serde_json::Value> {
        let resp = self
            .inner
            .get(format!("{}/health", self.base_url))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn get_password_policy(&self, tenant_id: &str) -> ApiResult<PasswordPolicyResponse> {
        let resp = self
            .inner
            .get(format!("{}/settings/password-policy", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn put_password_policy(
        &self,
        tenant_id: &str,
        min_length: i32,
        require_uppercase: bool,
        require_digit: bool,
        require_special: bool,
    ) -> ApiResult<serde_json::Value> {
        let resp = self
            .inner
            .put(format!("{}/settings/password-policy", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({
                "min_length": min_length,
                "require_uppercase": require_uppercase,
                "require_digit": require_digit,
                "require_special": require_special,
                "history_size": 0,
            }))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn get_lockout_policy(&self, tenant_id: &str) -> ApiResult<LockoutPolicyResponse> {
        let resp = self
            .inner
            .get(format!("{}/settings/lockout", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn put_lockout_policy(
        &self,
        tenant_id: &str,
        max_attempts: i32,
        window_minutes: i32,
        duration_minutes: i32,
    ) -> ApiResult<serde_json::Value> {
        let resp = self
            .inner
            .put(format!("{}/settings/lockout", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({
                "max_attempts": max_attempts,
                "window_minutes": window_minutes,
                "duration_minutes": duration_minutes,
            }))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn get_token_ttl(&self, tenant_id: &str) -> ApiResult<TokenTtlResponse> {
        let resp = self
            .inner
            .get(format!("{}/settings/tokens", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn put_token_ttl(
        &self,
        tenant_id: &str,
        access_token_ttl_minutes: i32,
        refresh_token_ttl_days: i32,
    ) -> ApiResult<serde_json::Value> {
        let resp = self
            .inner
            .put(format!("{}/settings/tokens", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({
                "access_token_ttl_minutes": access_token_ttl_minutes,
                "refresh_token_ttl_days": refresh_token_ttl_days,
            }))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn get_registration_policy(&self, tenant_id: &str) -> ApiResult<RegistrationPolicyResponse> {
        let resp = self
            .inner
            .get(format!("{}/settings/registration", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .send()
            .await?;
        self.check(resp).await
    }

    pub async fn put_registration_policy(
        &self,
        tenant_id: &str,
        allow_public_registration: bool,
        require_email_verified: bool,
    ) -> ApiResult<serde_json::Value> {
        let resp = self
            .inner
            .put(format!("{}/settings/registration", self.base_url))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({
                "allow_public_registration": allow_public_registration,
                "require_email_verified": require_email_verified,
            }))
            .send()
            .await?;
        self.check(resp).await
    }
}

// ── Response DTOs ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum LoginResult {
    Token(String),
    MfaRequired { mfa_token: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct Tenant {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub plan: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub is_active: bool,
    pub email_verified: bool,
    #[serde(default)]
    pub mfa_enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Role {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Permission {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Session {
    pub id: String,
    pub user_id: String,
    pub email: String,
    pub ip: Option<String>,
    pub created_at: String,
    pub last_seen_at: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthClient {
    pub id: String,
    pub client_id: String,
    #[serde(default)]
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

#[derive(Debug, Clone, Deserialize)]
pub struct IdentityProvider {
    pub id: String,
    pub provider: String,
    pub client_id: String,
    pub redirect_url: String,
    pub scopes: Vec<String>,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct PasswordPolicyResponse {
    pub min_length: i32,
    pub require_uppercase: bool,
    pub require_digit: bool,
    pub require_special: bool,
    pub history_size: i32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LockoutPolicyResponse {
    pub max_attempts: i32,
    pub window_minutes: i32,
    pub duration_minutes: i32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TokenTtlResponse {
    pub access_token_ttl_minutes: i32,
    pub refresh_token_ttl_days: i32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct RegistrationPolicyResponse {
    pub allow_public_registration: bool,
    pub require_email_verified: bool,
}
