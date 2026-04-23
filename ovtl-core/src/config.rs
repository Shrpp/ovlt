use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiration_minutes: i64,
    pub refresh_token_expiration_days: i64,
    pub master_encryption_key: String,
    /// Separate key used to wrap per-tenant encryption keys.
    /// Must differ from master_encryption_key.
    pub tenant_wrap_key: String,
    pub server_host: String,
    pub server_port: u16,
    pub environment: Environment,
    /// Comma-separated list of allowed CORS origins.
    /// Use `*` (or omit in dev) for permissive mode. Explicit list required in production.
    pub cors_allowed_origins: Vec<String>,
    pub google_oauth: Option<OAuthProviderConfig>,
    pub github_oauth: Option<OAuthProviderConfig>,
    /// Static key required in `X-OVTL-Admin-Key` to call admin endpoints.
    /// If not set, admin endpoints return 404.
    pub admin_key: Option<String>,
    /// Bootstrap: slug for the first tenant created on startup. Default: "master".
    pub bootstrap_tenant_slug: Option<String>,
    /// Bootstrap: admin user email created in the first tenant on startup.
    pub bootstrap_admin_email: Option<String>,
    /// Bootstrap: admin user password. Required if bootstrap_admin_email is set.
    pub bootstrap_admin_password: Option<String>,
}

#[derive(Clone, Debug)]
pub struct OAuthProviderConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Environment {
    Development,
    Production,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let jwt_secret = require("JWT_SECRET")?;
        if jwt_secret.len() < 32 {
            return Err("JWT_SECRET must be at least 32 characters".into());
        }

        let master_encryption_key = require("MASTER_ENCRYPTION_KEY")?;
        if master_encryption_key.len() < 32 {
            return Err("MASTER_ENCRYPTION_KEY must be at least 32 characters".into());
        }

        let tenant_wrap_key = require("TENANT_WRAP_KEY")?;
        if tenant_wrap_key.len() < 32 {
            return Err("TENANT_WRAP_KEY must be at least 32 characters".into());
        }
        if tenant_wrap_key == master_encryption_key {
            return Err("TENANT_WRAP_KEY must differ from MASTER_ENCRYPTION_KEY".into());
        }

        let environment = match env::var("ENVIRONMENT")
            .unwrap_or_else(|_| "development".into())
            .as_str()
        {
            "production" => Environment::Production,
            _ => Environment::Development,
        };

        let database_url = require("DATABASE_URL")?;
        if environment == Environment::Production && !database_url.contains("sslmode") {
            return Err(
                "DATABASE_URL must include sslmode parameter in production (e.g. sslmode=require)"
                    .into(),
            );
        }

        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_else(|_| "*".into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();

        if environment == Environment::Production
            && cors_allowed_origins == vec!["*".to_string()]
        {
            return Err(
                "CORS_ALLOWED_ORIGINS must be set explicitly in production (no wildcard)".into(),
            );
        }

        Ok(Self {
            database_url,
            jwt_secret,
            jwt_expiration_minutes: parse_i64("JWT_EXPIRATION_MINUTES", 15)?,
            refresh_token_expiration_days: parse_i64("REFRESH_TOKEN_EXPIRATION_DAYS", 30)?,
            master_encryption_key,
            tenant_wrap_key,
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3000".into())
                .parse::<u16>()
                .map_err(|_| "SERVER_PORT must be a valid port number".to_string())?,
            environment,
            cors_allowed_origins,
            google_oauth: opt_oauth("GOOGLE"),
            github_oauth: opt_oauth("GITHUB"),
            admin_key: env::var("OVTL_ADMIN_KEY").ok(),
            bootstrap_tenant_slug: env::var("OVTL_BOOTSTRAP_TENANT_SLUG").ok(),
            bootstrap_admin_email: env::var("OVTL_BOOTSTRAP_ADMIN_EMAIL").ok(),
            bootstrap_admin_password: env::var("OVTL_BOOTSTRAP_ADMIN_PASSWORD").ok(),
        })
    }

    pub fn is_production(&self) -> bool {
        self.environment == Environment::Production
    }

    pub fn oauth_for(&self, provider: &str) -> Option<&OAuthProviderConfig> {
        match provider {
            "google" => self.google_oauth.as_ref(),
            "github" => self.github_oauth.as_ref(),
            _ => None,
        }
    }
}

fn opt_oauth(prefix: &str) -> Option<OAuthProviderConfig> {
    Some(OAuthProviderConfig {
        client_id: env::var(format!("{prefix}_CLIENT_ID")).ok()?,
        client_secret: env::var(format!("{prefix}_CLIENT_SECRET")).ok()?,
        redirect_url: env::var(format!("{prefix}_REDIRECT_URL")).ok()?,
    })
}

fn require(key: &str) -> Result<String, String> {
    env::var(key).map_err(|_| format!("missing required env var: {key}"))
}

fn parse_i64(key: &str, default: i64) -> Result<i64, String> {
    match env::var(key) {
        Ok(v) => v.parse::<i64>().map_err(|_| format!("{key} must be an integer")),
        Err(_) => Ok(default),
    }
}
