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

    pub async fn login(&self, email: &str, password: &str, slug: &str) -> ApiResult<String> {
        #[derive(Deserialize)]
        struct LoginResp {
            access_token: String,
        }
        let resp = self
            .inner
            .post(format!("{}/auth/login", self.base_url))
            .header("x-ovtl-tenant-slug", slug)
            .json(&serde_json::json!({ "email": email, "password": password }))
            .send()
            .await?;
        let body: LoginResp = self.check(resp).await?;
        Ok(body.access_token)
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
    ) -> ApiResult<OAuthClient> {
        let resp = self
            .inner
            .put(format!("{}/clients/{}", self.base_url, id))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({
                "name": name,
                "redirect_uris": redirect_uris,
                "scopes": scopes,
            }))
            .send()
            .await?;
        self.check(resp).await
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

    pub async fn set_user_active(&self, tenant_id: &str, id: &str, is_active: bool) -> ApiResult<()> {
        let resp = self
            .inner
            .put(format!("{}/users/{}", self.base_url, id))
            .headers(self.tenant_headers(tenant_id))
            .json(&serde_json::json!({ "is_active": is_active }))
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(ApiError::Api { status: status.as_u16(), message: "update failed".into() })
        }
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
}

// ── Response DTOs ─────────────────────────────────────────────────────────────

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
    pub created_at: String,
}
