use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    models::flag::{CreateFlagRequest, FeatureFlag, UpdateFlagRequest, valid_flag_key},
    state::AppState,
};

pub async fn list_flags(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> AppResult<Json<Vec<FeatureFlag>>> {
    let rows = sqlx::query_as::<_, FeatureFlag>(
        r#"SELECT key, enabled, description, updated_at
           FROM feature_flags
           ORDER BY key ASC"#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("list flags: {e}")))?;
    Ok(Json(rows))
}

pub async fn create_flag(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateFlagRequest>,
) -> AppResult<Json<FeatureFlag>> {
    auth.require_admin()?;
    let key = body.key.trim().to_string();
    if !valid_flag_key(&key) {
        return Err(AppError::BadRequest(
            "key must be 1-64 chars of a-z, 0-9, or _".into(),
        ));
    }
    let description = clean_description(&body.description)?;
    let now = Utc::now();
    let flag = sqlx::query_as::<_, FeatureFlag>(
        r#"INSERT INTO feature_flags (key, enabled, description, updated_at)
           VALUES ($1, $2, $3, $4)
           RETURNING key, enabled, description, updated_at"#,
    )
    .bind(&key)
    .bind(body.enabled)
    .bind(description)
    .bind(now)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if e.as_database_error()
            .and_then(|d| d.constraint())
            .is_some()
        {
            AppError::Conflict("flag already exists".into())
        } else {
            AppError::internal(format!("create flag: {e}"))
        }
    })?;
    Ok(Json(flag))
}

pub async fn update_flag(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(key): Path<String>,
    Json(body): Json<UpdateFlagRequest>,
) -> AppResult<Json<FeatureFlag>> {
    auth.require_admin()?;
    if !valid_flag_key(&key) {
        return Err(AppError::BadRequest("invalid flag key".into()));
    }
    let mut flag = sqlx::query_as::<_, FeatureFlag>(
        r#"SELECT key, enabled, description, updated_at FROM feature_flags WHERE key = $1"#,
    )
    .bind(&key)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("get flag: {e}")))?
    .ok_or(AppError::NotFound)?;

    if let Some(enabled) = body.enabled {
        flag.enabled = enabled;
    }
    if let Some(description) = body.description {
        flag.description = clean_description(&description)?;
    }

    let flag = sqlx::query_as::<_, FeatureFlag>(
        r#"UPDATE feature_flags
           SET enabled = $2, description = $3, updated_at = $4
           WHERE key = $1
           RETURNING key, enabled, description, updated_at"#,
    )
    .bind(&flag.key)
    .bind(flag.enabled)
    .bind(&flag.description)
    .bind(Utc::now())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("update flag: {e}")))?;
    Ok(Json(flag))
}

pub async fn delete_flag(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(key): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_admin()?;
    if !valid_flag_key(&key) {
        return Err(AppError::BadRequest("invalid flag key".into()));
    }
    let res = sqlx::query("DELETE FROM feature_flags WHERE key = $1")
        .bind(&key)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("delete flag: {e}")))?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

fn clean_description(description: &str) -> AppResult<String> {
    let description = description.trim();
    if description.chars().count() > 400 {
        return Err(AppError::BadRequest(
            "description must be at most 400 characters".into(),
        ));
    }
    Ok(description.to_string())
}
