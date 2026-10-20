use axum::{
    Json,
    extract::{Path, State},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    host::apps,
    models::{
        device::{
            AdminDevice, Device, HeartbeatRequest, UpdateDeviceRequest, valid_fingerprint,
            valid_platform,
        },
        flag,
    },
    state::AppState,
};

pub async fn heartbeat(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<HeartbeatRequest>,
) -> AppResult<Json<Device>> {
    let platform = body.platform.trim().to_lowercase();
    if !valid_platform(&platform) {
        return Err(AppError::BadRequest(
            "platform must be web, android, desktop, or unknown".into(),
        ));
    }
    let fingerprint = body.fingerprint.trim().to_string();
    if !valid_fingerprint(&fingerprint) {
        return Err(AppError::BadRequest("invalid fingerprint".into()));
    }
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::BadRequest("name must be 1-80 characters".into()));
    }
    let ui_version = version_label("ui_version", &body.ui_version, true)?;
    let client_version = version_label("client_version", &body.client_version, true)?;
    let system_name = system_label(&body.system_name)?;
    let system_version = version_label("system_version", &body.system_version, false)?;
    let now = Utc::now();

    let existing = sqlx::query_as::<_, Device>(
        r#"SELECT id, user_id, name, platform, ui_version, client_version, system_name, system_version,
                  fingerprint, last_seen_at, is_revoked, created_at
           FROM devices WHERE user_id = $1 AND fingerprint = $2"#,
    )
    .bind(auth.id)
    .bind(&fingerprint)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("get device: {e}")))?;

    if let Some(device) = existing {
        if device.is_revoked {
            return Err(AppError::Forbidden);
        }
        let updated = sqlx::query_as::<_, Device>(
            r#"UPDATE devices
               SET platform = $2, ui_version = $3, client_version = $4,
                   system_name = $5, system_version = $6, last_seen_at = $7
               WHERE id = $1
               RETURNING id, user_id, name, platform, ui_version, client_version, system_name, system_version,
                         fingerprint, last_seen_at, is_revoked, created_at"#,
        )
        .bind(device.id)
        .bind(&platform)
        .bind(&ui_version)
        .bind(&client_version)
        .bind(&system_name)
        .bind(&system_version)
        .bind(now)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("touch device: {e}")))?;
        return Ok(Json(updated));
    }

    if flag::is_disabled(&state.pool, "device_registration").await? {
        return Err(AppError::BadRequest(
            "device registration is disabled".into(),
        ));
    }

    let created = sqlx::query_as::<_, Device>(
        r#"INSERT INTO devices
           (id, user_id, name, platform, ui_version, client_version, system_name, system_version,
            fingerprint, last_seen_at, is_revoked, created_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $10)
           RETURNING id, user_id, name, platform, ui_version, client_version, system_name, system_version,
                     fingerprint, last_seen_at, is_revoked, created_at"#,
    )
    .bind(Uuid::new_v4())
    .bind(auth.id)
    .bind(name)
    .bind(&platform)
    .bind(&ui_version)
    .bind(&client_version)
    .bind(&system_name)
    .bind(&system_version)
    .bind(&fingerprint)
    .bind(now)
    .bind(false)
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("create device: {e}")))?;

    Ok(Json(created))
}

pub async fn list_devices(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<AdminDevice>>> {
    auth.require_admin()?;
    let rows = sqlx::query_as::<_, AdminDevice>(
        r#"SELECT d.id, d.user_id, u.email AS user_email, u.display_name AS user_display_name,
                  d.name, d.platform, d.ui_version, d.client_version,
                  d.system_name, d.system_version, d.fingerprint,
                  d.last_seen_at, d.is_revoked, d.created_at
           FROM devices d
           JOIN users u ON u.id = d.user_id
           ORDER BY d.last_seen_at DESC"#,
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("list devices: {e}")))?;
    Ok(Json(rows))
}

pub async fn update_device(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateDeviceRequest>,
) -> AppResult<Json<AdminDevice>> {
    auth.require_admin()?;
    let current = sqlx::query_as::<_, Device>(
        r#"SELECT id, user_id, name, platform, ui_version, client_version, system_name, system_version,
                  fingerprint, last_seen_at, is_revoked, created_at
           FROM devices WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("get device: {e}")))?
    .ok_or(AppError::NotFound)?;

    let mut name = current.name;
    let mut revoked = current.is_revoked;
    if let Some(next) = body.name {
        let next = next.trim().to_string();
        if next.is_empty() || next.chars().count() > 80 {
            return Err(AppError::BadRequest("name must be 1-80 characters".into()));
        }
        name = next;
    }
    if let Some(flag) = body.is_revoked {
        revoked = flag;
    }

    sqlx::query(
        r#"UPDATE devices SET name = $2, is_revoked = $3 WHERE id = $1"#,
    )
    .bind(id)
    .bind(&name)
    .bind(revoked)
    .execute(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("update device: {e}")))?;

    load_admin(&state, id).await
}

pub async fn delete_device(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    auth.require_admin()?;
    let res = sqlx::query("DELETE FROM devices WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await
        .map_err(|e| AppError::internal(format!("delete device: {e}")))?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ClaimLaunchesRequest {
    pub fingerprint: String,
}

#[derive(Serialize)]
pub struct ClaimedLaunchBody {
    pub id: Uuid,
    pub app: String,
}

#[derive(Serialize)]
pub struct ClaimLaunchesResponse {
    pub launches: Vec<ClaimedLaunchBody>,
    pub handoff: Option<String>,
}

pub async fn claim_launches(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<ClaimLaunchesRequest>,
) -> AppResult<Json<ClaimLaunchesResponse>> {
    let fingerprint = body.fingerprint.trim();
    if !valid_fingerprint(fingerprint) {
        return Err(AppError::BadRequest("invalid fingerprint".into()));
    }
    let claimed = apps::claim_client(&state.pool, auth.id, fingerprint).await?;
    Ok(Json(ClaimLaunchesResponse {
        launches: claimed
            .launches
            .into_iter()
            .map(|item| ClaimedLaunchBody {
                id: item.id,
                app: item.app,
            })
            .collect(),
        handoff: claimed.handoff,
    }))
}

async fn load_admin(state: &AppState, id: Uuid) -> AppResult<Json<AdminDevice>> {
    let row = sqlx::query_as::<_, AdminDevice>(
        r#"SELECT d.id, d.user_id, u.email AS user_email, u.display_name AS user_display_name,
                  d.name, d.platform, d.ui_version, d.client_version,
                  d.system_name, d.system_version, d.fingerprint,
                  d.last_seen_at, d.is_revoked, d.created_at
           FROM devices d
           JOIN users u ON u.id = d.user_id
           WHERE d.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("load device: {e}")))?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

fn version_label(field: &str, value: &str, required: bool) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty() {
        return if required {
            Err(AppError::BadRequest(format!("{field} is required")))
        } else {
            Ok(String::new())
        };
    }
    if value.chars().count() > 40
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '+'))
    {
        return Err(AppError::BadRequest(format!(
            "{field} must be 1-40 chars of letters, digits, ., -, _, +"
        )));
    }
    Ok(value.to_string())
}

fn system_label(value: &str) -> AppResult<String> {
    let value = value.trim();
    if value.is_empty()
        || value.chars().count() > 40
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_'))
    {
        return Err(AppError::BadRequest(
            "system_name must be 1-40 chars of letters, digits, spaces, ., -".into(),
        ));
    }
    Ok(value.to_string())
}
