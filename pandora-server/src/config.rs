use std::env;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub jwt_secret: String,
    pub jwt_expiration_minutes: i64,
    pub refresh_token_expiration_days: i64,
    pub master_encryption_key: String,
    pub server_host: String,
    pub server_port: u16,
    pub environment: Environment,
    pub google_oauth: Option<OAuthProviderConfig>,
    pub github_oauth: Option<OAuthProviderConfig>,
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
        Ok(Self {
            database_url: require("DATABASE_URL")?,
            jwt_secret: require("JWT_SECRET")?,
            jwt_expiration_minutes: parse_i64("JWT_EXPIRATION_MINUTES", 15)?,
            refresh_token_expiration_days: parse_i64("REFRESH_TOKEN_EXPIRATION_DAYS", 30)?,
            master_encryption_key: require("MASTER_ENCRYPTION_KEY")?,
            server_host: env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            server_port: env::var("SERVER_PORT")
                .unwrap_or_else(|_| "3000".into())
                .parse::<u16>()
                .map_err(|_| "SERVER_PORT must be a valid port number".to_string())?,
            environment: match env::var("ENVIRONMENT")
                .unwrap_or_else(|_| "development".into())
                .as_str()
            {
                "production" => Environment::Production,
                _ => Environment::Development,
            },
            google_oauth: opt_oauth("GOOGLE"),
            github_oauth: opt_oauth("GITHUB"),
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
