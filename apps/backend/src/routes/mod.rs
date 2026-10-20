pub mod admin;
pub mod auth;
pub mod chat;
pub mod devices;
pub mod flags;
pub mod health;
pub mod host;

use axum::{
    Router,
    routing::{get, patch, post},
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(health::root))
        .route("/health", get(health::health))
        .route("/version", get(health::version))
        .route("/auth/login", post(auth::login))
        .route("/me", get(auth::me))
        .route("/me/usage", get(auth::my_usage))
        .route("/chat/messages", get(chat::list_messages).post(chat::send_message))
        .route("/admin/users", get(admin::list_users).post(admin::create_user))
        .route(
            "/admin/users/{id}",
            patch(admin::update_user).delete(admin::delete_user),
        )
        .route("/admin/llm/models", get(admin::llm_models))
        .route("/admin/llm/status", get(admin::llm_status))
        .route("/admin/host/metrics", get(host::metrics))
        .route("/admin/host/exec", post(host::exec_command))
        .route("/me/devices", post(devices::heartbeat))
        .route(
            "/admin/devices",
            get(devices::list_devices),
        )
        .route(
            "/admin/devices/{id}",
            patch(devices::update_device).delete(devices::delete_device),
        )
        .route("/flags", get(flags::list_flags))
        .route(
            "/admin/flags",
            get(flags::list_flags).post(flags::create_flag),
        )
        .route(
            "/admin/flags/{key}",
            patch(flags::update_flag).delete(flags::delete_flag),
        )
}
