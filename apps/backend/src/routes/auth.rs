use axum::{
    Json,
    extract::State,
    http::HeaderMap,
};
use serde::Serialize;

use crate::{
    auth::{jwt, middleware::AuthUser, password},
    error::{AppError, AppResult},
    llm::limits,
    models::user::{LoginRequest, PublicUser, User},
    state::AppState,
};

#[derive(Serialize)]
pub struct LoginResponse {
    access_token: String,
    user: PublicUser,
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .split(',')
        .next()
        .unwrap_or("unknown")
        .trim()
        .to_string();

    {
        let mut limiter = state.login_limiter.lock().await;
        if !limiter.check(&ip, 10) {
            return Err(AppError::RateLimited {
                message: "Too many login attempts".into(),
                retry_after_sec: 60,
                scope: "login".into(),
            });
        }
    }

    let email = body.email.trim().to_lowercase();
    let user = sqlx::query_as::<_, User>(
        r#"SELECT id, email, password_hash, display_name, role, is_active, created_at
           FROM users WHERE email = $1"#,
    )
    .bind(&email)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("login query: {e}")))?
    .ok_or(AppError::Unauthorized)?;

    if !user.is_active {
        return Err(AppError::Forbidden);
    }

    if !password::verify_password(&body.password, &user.password_hash)? {
        return Err(AppError::Unauthorized);
    }

    let token = jwt::issue_token(user.id, &user.role, &state.config.jwt_secret)?;
    Ok(Json(LoginResponse {
        access_token: token,
        user: user.into(),
    }))
}

pub async fn me(auth: AuthUser) -> Json<PublicUser> {
    Json(auth.user.into())
}

pub async fn my_usage(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<limits::UsageSnapshot>> {
    let snap = limits::snapshot(&state.pool, &state.config, auth.id).await?;
    Ok(Json(snap))
}
