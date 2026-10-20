use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::{
    db::DbPool,
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct FeatureFlag {
    pub key: String,
    pub enabled: bool,
    pub description: String,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateFlagRequest {
    pub key: String,
    pub description: String,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFlagRequest {
    pub enabled: Option<bool>,
    pub description: Option<String>,
}

pub fn valid_flag_key(key: &str) -> bool {
    let n = key.len();
    (1..=64).contains(&n)
        && key
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Missing flag is treated as enabled-by-default so a deleted seed does not lock the product.
pub async fn is_disabled(pool: &DbPool, key: &str) -> AppResult<bool> {
    let row: Option<(bool,)> = sqlx::query_as("SELECT enabled FROM feature_flags WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::internal(format!("flag {key}: {e}")))?;
    Ok(row.is_some_and(|(enabled,)| !enabled))
}
