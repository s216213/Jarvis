use chrono::Utc;
use uuid::Uuid;

use crate::{
    auth::password,
    config::Config,
    db::DbPool,
    error::{AppError, AppResult},
    llm::RoutePreference,
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

pub async fn ensure_flags(pool: &DbPool) -> AppResult<()> {
    let defaults = [
        (
            "chat_enabled",
            true,
            "Czat z Jarvisem. Wyłączenie blokuje nowe wiadomości.",
        ),
        (
            "device_registration",
            true,
            "Nowe urządzenia klientów (web, android, desktop). Wyłączenie zostawia już znane urządzenia.",
        ),
        (
            "voice_enabled",
            true,
            "Tryb głosowy na desktopie. Wyłączenie blokuje POST /chat/voice.",
        ),
    ];

    for (key, enabled, description) in defaults {
        sqlx::query(
            r#"INSERT INTO feature_flags (key, enabled, description, updated_at)
               VALUES ($1, $2, $3, $4)
               ON CONFLICT (key) DO NOTHING"#,
        )
        .bind(key)
        .bind(enabled)
        .bind(description)
        .bind(Utc::now())
        .execute(pool)
        .await
        .map_err(|e| AppError::internal(format!("seed flag {key}: {e}")))?;
    }

    Ok(())
}

pub async fn load_llm_route(pool: &DbPool, env_model: Option<&str>) -> AppResult<RoutePreference> {
    let existing = sqlx::query_as::<_, (String, String)>(
        "SELECT provider, model FROM llm_route WHERE id = 1",
    )
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::internal(format!("load llm route: {e}")))?;

    if let Some((provider, model)) = existing {
        return Ok(RoutePreference { provider, model });
    }

    let model = env_model
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("auto");
    let preference = RoutePreference {
        provider: "auto".into(),
        model: model.to_string(),
    };
    save_llm_route(pool, &preference).await?;
    Ok(preference)
}

pub async fn save_llm_route(pool: &DbPool, preference: &RoutePreference) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO llm_route (id, provider, model, updated_at)
           VALUES (1, $1, $2, $3)
           ON CONFLICT (id) DO UPDATE
           SET provider = excluded.provider,
               model = excluded.model,
               updated_at = excluded.updated_at"#,
    )
    .bind(&preference.provider)
    .bind(&preference.model)
    .bind(Utc::now())
    .execute(pool)
    .await
    .map_err(|e| AppError::internal(format!("save llm route: {e}")))?;
    Ok(())
}
