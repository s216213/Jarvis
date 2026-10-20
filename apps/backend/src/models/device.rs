use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct Device {
    pub id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub platform: String,
    pub ui_version: String,
    pub client_version: String,
    pub system_name: String,
    pub system_version: String,
    pub fingerprint: String,
    pub last_seen_at: DateTime<Utc>,
    pub is_revoked: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AdminDevice {
    pub id: Uuid,
    pub user_id: Uuid,
    pub user_email: String,
    pub user_display_name: String,
    pub name: String,
    pub platform: String,
    pub ui_version: String,
    pub client_version: String,
    pub system_name: String,
    pub system_version: String,
    pub fingerprint: String,
    pub last_seen_at: DateTime<Utc>,
    pub is_revoked: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct HeartbeatRequest {
    pub name: String,
    pub platform: String,
    pub ui_version: String,
    pub client_version: String,
    pub system_name: String,
    pub system_version: String,
    pub fingerprint: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateDeviceRequest {
    pub name: Option<String>,
    pub is_revoked: Option<bool>,
}

pub fn valid_platform(platform: &str) -> bool {
    matches!(platform, "web" | "android" | "desktop" | "unknown")
}

pub fn valid_fingerprint(fingerprint: &str) -> bool {
    let n = fingerprint.len();
    (8..=128).contains(&n)
        && fingerprint
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}
