use axum::{
    routing::{delete, get, put},
    Router,
};

use crate::{handlers::admin_permissions, state::AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/permissions",
            get(admin_permissions::list_permissions).post(admin_permissions::create_permission),
        )
        .route(
            "/permissions/:id",
            put(admin_permissions::update_permission).delete(admin_permissions::delete_permission),
        )
        .route(
            "/roles/:id/permissions",
            get(admin_permissions::list_role_permissions)
                .post(admin_permissions::assign_role_permission),
        )
        .route(
            "/roles/:role_id/permissions/:perm_id",
            delete(admin_permissions::revoke_role_permission),
        )
}
