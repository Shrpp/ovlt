use axum::Router;
use utoipa::{
    openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme},
    Modify, OpenApi,
};
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    handlers::{
        admin_identity_providers::{CreateIdpRequest, IdpResponse, UpdateIdpRequest},
        admin_permissions::{
            AssignPermissionRequest, CreatePermissionRequest, PermissionResponse,
            UpdatePermissionRequest,
        },
        admin_roles::{AssignRoleRequest, CreateRoleRequest, RoleResponse, UpdateRoleRequest},
        admin_sessions::SessionResponse,
        admin_smtp::{SmtpConfigResponse, UpsertSmtpRequest},
        admin_tenant_settings::{
            LockoutResponse, PolicyResponse, RegistrationResponse, TokenTtlResponse,
            UpsertLockoutRequest, UpsertPolicyRequest, UpsertRegistrationRequest,
            UpsertTokenTtlRequest,
        },
        admin_users::{
            CreateUserRequest, PasswordResetTokenResponse, UpdateUserRequest, UserResponse,
            VerificationCodeResponse,
        },
        admin_webauthn::PasskeyInfo,
        audit_log::AuditLogEntry,
        clients::{ClientResponse, CreateClientRequest, UpdateClientRequest},
        forgot_password::ForgotPasswordRequest,
        login::{LoginRequest, MfaRequiredResponse, TokenResponse},
        logout::LogoutRequest,
        mfa::{ChallengeRequest, ConfirmRequest, SetupResponse},
        oauth_as::{
            AuthorizeParams, IntrospectRequest, TokenRequest, TokenResponse as OauthTokenResponse,
        },
        oauth_revoke::RevokeRequest,
        refresh::RefreshRequest,
        register::{RegisterRequest, RegisterResponse},
        reset_password::ResetPasswordRequest,
        tenants::{CreateTenantRequest, TenantResponse, TenantSlugEntry},
        verify_email::VerifyOtpRequest,
        webauthn::{
            AuthFinishPayload, AuthStartPayload, RegisterFinishPayload,
            TokenResponse as WebauthnTokenResponse,
        },
    },
    state::AppState,
};

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer_auth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
            components.add_security_scheme(
                "admin_key",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-OVLT-Admin-Key"))),
            );
        }
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "OVLT Auth Service",
        version = "0.1.0-alpha",
        description = "Developer-first auth service — Keycloak alternative. Tenant = realm. Full production auth on free tier.",
    ),
    paths(
        // auth
        crate::handlers::login::login,
        crate::handlers::register::register,
        crate::handlers::logout::logout,
        crate::handlers::refresh::refresh,
        crate::handlers::revoke::revoke,
        crate::handlers::forgot_password::forgot_password,
        crate::handlers::reset_password::reset_password,
        crate::handlers::verify_email::verify_email,
        // mfa
        crate::handlers::mfa::mfa_setup,
        crate::handlers::mfa::mfa_confirm,
        crate::handlers::mfa::mfa_disable,
        crate::handlers::mfa::mfa_challenge,
        crate::handlers::mfa::admin_disable_mfa,
        // webauthn
        crate::handlers::webauthn::register_start,
        crate::handlers::webauthn::register_finish,
        crate::handlers::webauthn::authenticate_start,
        crate::handlers::webauthn::authenticate_finish,
        // user
        crate::handlers::me::me,
        // settings
        crate::handlers::admin_tenant_settings::get_policy,
        crate::handlers::admin_tenant_settings::put_policy,
        crate::handlers::admin_tenant_settings::get_lockout,
        crate::handlers::admin_tenant_settings::put_lockout,
        crate::handlers::admin_tenant_settings::get_token_ttl,
        crate::handlers::admin_tenant_settings::put_token_ttl,
        crate::handlers::admin_tenant_settings::get_registration,
        crate::handlers::admin_tenant_settings::put_registration,
        // tenants
        crate::handlers::tenants::list_tenants,
        crate::handlers::tenants::create_tenant,
        crate::handlers::tenants::list_tenant_slugs,
        // clients
        crate::handlers::clients::list_clients,
        crate::handlers::clients::create_client,
        crate::handlers::clients::update_client,
        crate::handlers::clients::deactivate_client,
        // admin users
        crate::handlers::admin_users::list_users,
        crate::handlers::admin_users::create_user,
        crate::handlers::admin_users::update_user,
        crate::handlers::admin_users::deactivate_user,
        crate::handlers::admin_users::get_verification_code,
        crate::handlers::admin_users::get_password_reset_token,
        // admin roles
        crate::handlers::admin_roles::list_roles,
        crate::handlers::admin_roles::create_role,
        crate::handlers::admin_roles::update_role,
        crate::handlers::admin_roles::delete_role,
        crate::handlers::admin_roles::list_user_roles,
        crate::handlers::admin_roles::assign_user_role,
        crate::handlers::admin_roles::revoke_user_role,
        crate::handlers::admin_roles::list_client_roles,
        crate::handlers::admin_roles::assign_client_role,
        crate::handlers::admin_roles::revoke_client_role,
        // admin permissions
        crate::handlers::admin_permissions::list_permissions,
        crate::handlers::admin_permissions::create_permission,
        crate::handlers::admin_permissions::update_permission,
        crate::handlers::admin_permissions::delete_permission,
        crate::handlers::admin_permissions::list_role_permissions,
        crate::handlers::admin_permissions::assign_role_permission,
        crate::handlers::admin_permissions::revoke_role_permission,
        // admin sessions
        crate::handlers::admin_sessions::list_sessions,
        crate::handlers::admin_sessions::delete_session,
        // admin smtp
        crate::handlers::admin_smtp::get_smtp,
        crate::handlers::admin_smtp::put_smtp,
        // admin webauthn
        crate::handlers::admin_webauthn::list_passkeys,
        crate::handlers::admin_webauthn::delete_passkey,
        // admin identity providers
        crate::handlers::admin_identity_providers::list_idps,
        crate::handlers::admin_identity_providers::create_idp,
        crate::handlers::admin_identity_providers::update_idp,
        crate::handlers::admin_identity_providers::delete_idp,
        // audit log
        crate::handlers::audit_log::list_audit_log,
        // oauth
        crate::handlers::oauth_as::authorize,
        crate::handlers::oauth_as::token,
        crate::handlers::oauth_as::introspect,
        crate::handlers::oauth_revoke::revoke,
        // well-known
        crate::handlers::well_known::discovery,
        crate::handlers::well_known::jwks,
    ),
    components(
        schemas(
            LoginRequest,
            TokenResponse,
            MfaRequiredResponse,
            RegisterRequest,
            RegisterResponse,
            LogoutRequest,
            RefreshRequest,
            ForgotPasswordRequest,
            ResetPasswordRequest,
            VerifyOtpRequest,
            SetupResponse,
            ConfirmRequest,
            ChallengeRequest,
            RegisterFinishPayload,
            AuthStartPayload,
            AuthFinishPayload,
            WebauthnTokenResponse,
            PolicyResponse,
            UpsertPolicyRequest,
            LockoutResponse,
            UpsertLockoutRequest,
            TokenTtlResponse,
            UpsertTokenTtlRequest,
            RegistrationResponse,
            UpsertRegistrationRequest,
            TenantResponse,
            TenantSlugEntry,
            CreateTenantRequest,
            ClientResponse,
            CreateClientRequest,
            UpdateClientRequest,
            UserResponse,
            CreateUserRequest,
            UpdateUserRequest,
            VerificationCodeResponse,
            PasswordResetTokenResponse,
            RoleResponse,
            CreateRoleRequest,
            UpdateRoleRequest,
            AssignRoleRequest,
            PermissionResponse,
            CreatePermissionRequest,
            UpdatePermissionRequest,
            AssignPermissionRequest,
            SessionResponse,
            SmtpConfigResponse,
            UpsertSmtpRequest,
            PasskeyInfo,
            IdpResponse,
            CreateIdpRequest,
            UpdateIdpRequest,
            AuditLogEntry,
            AuthorizeParams,
            TokenRequest,
            OauthTokenResponse,
            IntrospectRequest,
            RevokeRequest,
        )
    ),
    modifiers(&SecurityAddon),
    tags(
        (name = "auth", description = "User-facing authentication endpoints"),
        (name = "user", description = "Authenticated user profile endpoints"),
        (name = "settings", description = "Per-tenant settings (requires JWT)"),
        (name = "oauth", description = "OAuth 2.0 / OIDC authorization server"),
        (name = "tenants", description = "Tenant management (admin)"),
        (name = "clients", description = "OAuth client management (admin)"),
        (name = "admin-users", description = "User management (admin)"),
        (name = "admin-roles", description = "Role and permission management (admin)"),
        (name = "admin-sessions", description = "Session management (admin)"),
        (name = "admin-smtp", description = "SMTP configuration (admin)"),
        (name = "admin-webauthn", description = "Passkey / WebAuthn management (admin)"),
        (name = "admin-idp", description = "Identity provider management (admin)"),
        (name = "audit", description = "Audit log (admin)"),
    )
)]
pub struct ApiDoc;

/// Returns a router that serves:
/// - `GET /openapi.json` — raw OpenAPI spec
/// - `GET /docs`         — Swagger UI
pub fn swagger_router() -> Router<AppState> {
    let spec = ApiDoc::openapi();
    Router::new().merge(SwaggerUi::new("/docs").url("/openapi.json", spec))
}
