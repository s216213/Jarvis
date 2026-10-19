use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    auth::{middleware::AuthUser, password},
    error::{AppError, AppResult},
    llm::limits,
    models::user::{CreateUserRequest, PublicUser, UpdateUserRequest, User},
    state::AppState,
};

async fn require_admin(auth: &AuthUser) -> AppResult<()> {
    if auth.is_admin() {
        Ok(())
    } else {
        Err(AppError::Forbidden)
    }
}

pub async fn list_users(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<PublicUser>>> {
    require_admin(&auth).await?;
    let rows = sqlx::query_as::<_, User>(
        r#"SELECT id, email, password_hash, display_name, role, is_active, created_at
           FROM users ORDER BY created_at ASC"#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("list users: {e}")))?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
}

pub async fn create_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<CreateUserRequest>,
) -> AppResult<Json<PublicUser>> {
    require_admin(&auth).await?;

    let role = body.role.to_lowercase();
    if role != "user" && role != "admin" {
        return Err(AppError::BadRequest("role must be user or admin".into()));
    }

    let email = body.email.trim().to_lowercase();
    if email.is_empty() || body.display_name.trim().is_empty() {
        return Err(AppError::BadRequest(
            "email and display_name are required".into(),
        ));
    }

    let hash = password::hash_password(&body.password)?;
    let id = Uuid::new_v4();
    let now = Utc::now();

    let user = sqlx::query_as::<_, User>(
        r#"INSERT INTO users (id, email, password_hash, display_name, role, is_active, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7)
           RETURNING id, email, password_hash, display_name, role, is_active, created_at"#,
    )
    .bind(id)
    .bind(&email)
    .bind(hash)
    .bind(body.display_name.trim())
    .bind(&role)
    .bind(true)
    .bind(now)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| {
        if e.as_database_error()
            .and_then(|d| d.constraint())
            .is_some_and(|c| c.contains("email"))
        {
            AppError::Conflict("email already exists".into())
        } else {
            AppError::internal(format!("create user: {e}"))
        }
    })?;

    Ok(Json(user.into()))
}

pub async fn update_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateUserRequest>,
) -> AppResult<Json<PublicUser>> {
    require_admin(&auth).await?;

    let mut user = sqlx::query_as::<_, User>(
        r#"SELECT id, email, password_hash, display_name, role, is_active, created_at
           FROM users WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("get user: {e}")))?
    .ok_or(AppError::NotFound)?;

    if let Some(active) = body.is_active {
        user.is_active = active;
    }
    if let Some(role) = body.role {
        let role = role.to_lowercase();
        if role != "user" && role != "admin" {
            return Err(AppError::BadRequest("role must be user or admin".into()));
        }
        user.role = role;
    }
    if let Some(name) = body.display_name {
        user.display_name = name;
    }
    if let Some(pw) = body.password {
        user.password_hash = password::hash_password(&pw)?;
    }

    let user = sqlx::query_as::<_, User>(
        r#"UPDATE users
           SET is_active = $2, role = $3, display_name = $4, password_hash = $5
           WHERE id = $1
           RETURNING id, email, password_hash, display_name, role, is_active, created_at"#,
    )
    .bind(id)
    .bind(user.is_active)
    .bind(&user.role)
    .bind(&user.display_name)
    .bind(&user.password_hash)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("update user: {e}")))?;

    Ok(Json(user.into()))
}

pub async fn delete_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    require_admin(&auth).await?;
    if auth.id == id {
        return Err(AppError::BadRequest("cannot delete yourself".into()));
    }

    let res = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("delete user: {e}")))?;

    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Serialize)]
pub struct LlmModelsResponse {
    active: String,
    source: String,
    provider: String,
    models: Vec<crate::llm::provider::ModelInfo>,
}

pub async fn llm_models(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<LlmModelsResponse>> {
    require_admin(&auth).await?;
    let (active, models) = state.llm.status().await?;
    Ok(Json(LlmModelsResponse {
        active: active.model.clone(),
        source: active.source.clone(),
        provider: active.provider,
        models,
    }))
}

#[derive(Serialize)]
pub struct LlmStatusResponse {
    provider: String,
    model: String,
    source: String,
    usage: limits::UsageSnapshot,
    limits: LimitConfigView,
}

#[derive(Serialize)]
pub struct LimitConfigView {
    user_rpm: u32,
    user_daily: u32,
    global_rpm: u32,
    global_daily: u32,
    max_input_chars: usize,
    context_messages: usize,
    max_inflight: usize,
}

pub async fn llm_status(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<LlmStatusResponse>> {
    require_admin(&auth).await?;
    let (active, _) = state.llm.status().await?;
    let usage = limits::snapshot(&state.pool, &state.config, auth.id).await?;
    Ok(Json(LlmStatusResponse {
        provider: active.provider,
        model: active.model,
        source: active.source,
        usage,
        limits: LimitConfigView {
            user_rpm: state.config.llm_user_rpm,
            user_daily: state.config.llm_user_daily,
            global_rpm: state.config.llm_global_rpm,
            global_daily: state.config.llm_global_daily,
            max_input_chars: state.config.llm_max_input_chars,
            context_messages: state.config.llm_context_messages,
            max_inflight: state.config.llm_max_inflight,
        },
    }))
}
