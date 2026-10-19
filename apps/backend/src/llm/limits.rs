use chrono::{Duration, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    config::Config,
    db::DbPool,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, Serialize)]
pub struct UsageSnapshot {
    pub user_rpm_used: i64,
    pub user_rpm_limit: u32,
    pub user_daily_used: i64,
    pub user_daily_limit: u32,
    pub global_rpm_used: i64,
    pub global_rpm_limit: u32,
    pub global_daily_used: i64,
    pub global_daily_limit: u32,
}

pub async fn snapshot(pool: &DbPool, config: &Config, user_id: Uuid) -> AppResult<UsageSnapshot> {
    let minute_ago = Utc::now() - Duration::minutes(1);
    let day_ago = Utc::now() - Duration::days(1);

    let user_rpm: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM llm_usage WHERE user_id = $1 AND created_at >= $2",
    )
    .bind(user_id)
    .bind(minute_ago)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::internal(format!("usage user rpm: {e}")))?;

    let user_daily: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM llm_usage WHERE user_id = $1 AND created_at >= $2",
    )
    .bind(user_id)
    .bind(day_ago)
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::internal(format!("usage user daily: {e}")))?;

    let global_rpm: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM llm_usage WHERE created_at >= $1")
            .bind(minute_ago)
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::internal(format!("usage global rpm: {e}")))?;

    let global_daily: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM llm_usage WHERE created_at >= $1")
            .bind(day_ago)
            .fetch_one(pool)
            .await
            .map_err(|e| AppError::internal(format!("usage global daily: {e}")))?;

    Ok(UsageSnapshot {
        user_rpm_used: user_rpm.0,
        user_rpm_limit: config.llm_user_rpm,
        user_daily_used: user_daily.0,
        user_daily_limit: config.llm_user_daily,
        global_rpm_used: global_rpm.0,
        global_rpm_limit: config.llm_global_rpm,
        global_daily_used: global_daily.0,
        global_daily_limit: config.llm_global_daily,
    })
}

pub async fn check_or_rate_limit(
    pool: &DbPool,
    config: &Config,
    user_id: Uuid,
) -> AppResult<UsageSnapshot> {
    let snap = snapshot(pool, config, user_id).await?;

    if snap.user_rpm_used >= i64::from(config.llm_user_rpm) {
        return Err(AppError::RateLimited {
            message: "Per-user requests-per-minute limit reached".into(),
            retry_after_sec: 60,
            scope: "user".into(),
        });
    }
    if snap.user_daily_used >= i64::from(config.llm_user_daily) {
        return Err(AppError::RateLimited {
            message: "Per-user daily limit reached".into(),
            retry_after_sec: 3600,
            scope: "daily".into(),
        });
    }
    if snap.global_rpm_used >= i64::from(config.llm_global_rpm) {
        return Err(AppError::RateLimited {
            message: "Global requests-per-minute limit reached".into(),
            retry_after_sec: 60,
            scope: "global".into(),
        });
    }
    if snap.global_daily_used >= i64::from(config.llm_global_daily) {
        return Err(AppError::RateLimited {
            message: "Global daily limit reached".into(),
            retry_after_sec: 3600,
            scope: "daily".into(),
        });
    }

    Ok(snap)
}

pub async fn record_usage(
    pool: &DbPool,
    user_id: Uuid,
    provider: &str,
    model: &str,
    prompt_chars: i32,
    success: bool,
) -> AppResult<()> {
    let id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        r#"INSERT INTO llm_usage (id, user_id, provider, model, prompt_chars, success, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(provider)
    .bind(model)
    .bind(prompt_chars)
    .bind(success)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|e| AppError::internal(format!("record usage: {e}")))?;
    Ok(())
}
