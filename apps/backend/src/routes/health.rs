use axum::{Json, extract::State};
use serde::Serialize;
use serde_json::{Value, json};
use sysinfo::System;

use crate::{auth::middleware::AuthUser, error::AppResult, state::AppState};

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    db: &'static str,
}

pub async fn root() -> Json<Value> {
    Json(json!({
        "service": "jarvis-api",
        "version": env!("CARGO_PKG_VERSION"),
        "brain_version": env!("CARGO_PKG_VERSION"),
    }))
}

#[derive(Serialize)]
pub struct JarvisVersion {
    pub brain_version: &'static str,
    pub system_name: String,
    pub system_version: String,
}

/// Jarvis itself: the UI version is supplied by the client. This returns the brain and the host OS.
pub async fn version(_auth: AuthUser) -> Json<JarvisVersion> {
    Json(JarvisVersion {
        brain_version: env!("CARGO_PKG_VERSION"),
        system_name: System::name().unwrap_or_default(),
        system_version: System::os_version().unwrap_or_default(),
    })
}

pub async fn health(State(state): State<AppState>) -> AppResult<Json<HealthResponse>> {
    let db_ok = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();

    Ok(Json(HealthResponse {
        status: if db_ok { "ok" } else { "degraded" },
        db: if db_ok { "ok" } else { "error" },
    }))
}
