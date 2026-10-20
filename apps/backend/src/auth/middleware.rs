use axum::{extract::FromRequestParts, http::request::Parts};
use uuid::Uuid;

use crate::{
    auth::jwt,
    error::{AppError, AppResult},
    models::user::User,
    state::AppState,
};

#[derive(Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub role: String,
    pub user: User,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }

    pub fn require_admin(&self) -> AppResult<()> {
        if self.is_admin() {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;

        let token = auth
            .strip_prefix("Bearer ")
            .ok_or(AppError::Unauthorized)?;

        let claims = jwt::decode_token(token, &state.config.jwt_secret)?;

        let user = sqlx::query_as::<_, User>(
            r#"SELECT id, email, password_hash, display_name, role, is_active, created_at
               FROM users WHERE id = $1"#,
        )
        .bind(claims.sub)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("auth user: {e}")))?
        .ok_or(AppError::Unauthorized)?;

        if !user.is_active {
            return Err(AppError::Forbidden);
        }

        Ok(AuthUser {
            id: user.id,
            role: user.role.clone(),
            user,
        })
    }
}
