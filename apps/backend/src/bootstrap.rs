use chrono::Utc;
use uuid::Uuid;

use crate::{
    auth::password,
    config::Config,
    db::DbPool,
    error::{AppError, AppResult},
};

pub async fn ensure_admin(pool: &DbPool, config: &Config) -> AppResult<()> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
        .map_err(|e| AppError::internal(format!("count users: {e}")))?;

    if count.0 > 0 {
        return Ok(());
    }

    let (Some(email), Some(password)) = (
        config.bootstrap_admin_email.as_ref(),
        config.bootstrap_admin_password.as_ref(),
    ) else {
        tracing::warn!(
            "users table empty and BOOTSTRAP_ADMIN_EMAIL/PASSWORD not set — no admin seeded"
        );
        return Ok(());
    };

    let hash = password::hash_password(password)?;
    let id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        r#"INSERT INTO users (id, email, password_hash, display_name, role, is_active, created_at)
           VALUES ($1, $2, $3, $4, 'admin', $5, $6)"#,
    )
    .bind(id)
    .bind(email.to_lowercase())
    .bind(hash)
    .bind("Admin")
    .bind(true)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| AppError::internal(format!("bootstrap admin: {e}")))?;

    tracing::info!(email = %email, "bootstrap admin created");
    Ok(())
}
