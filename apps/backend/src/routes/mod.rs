pub mod admin;
pub mod auth;
pub mod chat;
pub mod health;

use axum::{
    Router,
    routing::{get, patch, post},
};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(health::root))
        .route("/health", get(health::health))
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
}
