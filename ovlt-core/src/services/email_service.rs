use lettre::{
    message::{header::ContentType, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

use crate::entity::tenant_smtp_config;

pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_name: String,
    pub from_email: String,
    pub use_tls: bool,
}

pub async fn load_config(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    tenant_key: &str,
    master_key: &str,
) -> Option<SmtpConfig> {
    let record = tenant_smtp_config::Entity::find_by_id(tenant_id)
        .one(db)
        .await
        .ok()??;

    if !record.enabled {
        return None;
    }

    let password = hefesto::decrypt(&record.password_enc, tenant_key, master_key).ok()?;

    Some(SmtpConfig {
        host: record.host,
        port: record.port as u16,
        username: record.username,
        password,
        from_name: record.from_name,
        from_email: record.from_email,
        use_tls: record.use_tls,
    })
}

/// Best-effort send. Logs on failure but never propagates the error so callers
/// can return 200 regardless of email delivery status.
#[allow(clippy::too_many_arguments)]
pub async fn try_send(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    tenant_key: &str,
    master_key: &str,
    to_email: &str,
    subject: &str,
    html: &str,
    text: &str,
) {
    let Some(cfg) = load_config(db, tenant_id, tenant_key, master_key).await else {
        return;
    };

    let from = format!("{} <{}>", cfg.from_name, cfg.from_email);
    let msg = match Message::builder()
        .from(from.parse().unwrap())
        .to(to_email.parse().unwrap())
        .subject(subject)
        .multipart(
            MultiPart::alternative()
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_PLAIN)
                        .body(text.to_string()),
                )
                .singlepart(
                    SinglePart::builder()
                        .header(ContentType::TEXT_HTML)
                        .body(html.to_string()),
                ),
        ) {
        Ok(m) => m,
        Err(e) => {
            tracing::error!(tenant_id = %tenant_id, error = %e, "failed to build email");
            return;
        }
    };

    let creds = Credentials::new(cfg.username, cfg.password);

    let transport_result = if cfg.use_tls {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&cfg.host)
            .map(|t| t.port(cfg.port).credentials(creds).build())
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&cfg.host)
            .map(|t| t.port(cfg.port).credentials(creds).build())
    };

    let transport = match transport_result {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(tenant_id = %tenant_id, error = %e, "failed to build SMTP transport");
            return;
        }
    };

    if let Err(e) = transport.send(msg).await {
        tracing::error!(tenant_id = %tenant_id, to = to_email, error = %e, "SMTP send failed");
    }
}
