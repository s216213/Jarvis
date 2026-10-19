use axum::{Json, extract::State};
use serde::Serialize;
use serde_json::{Value, json};

use crate::{error::AppResult, state::AppState};

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    db: &'static str,
}

pub async fn root() -> Json<Value> {
    Json(json!({
        "service": "jarvis-api",
        "version": env!("CARGO_PKG_VERSION"),
    }))
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
