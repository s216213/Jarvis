use axum::{Json, extract::State};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    host::{
        exec::{self, ExecOutcome},
        metrics::{self, Snapshot},
    },
    state::AppState,
};

pub async fn metrics(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Snapshot>> {
    auth.require_admin()?;
    let mode = exec::exec_mode(&state.config);
    let snapshot = metrics::collect(&state.host, mode).await?;
    Ok(Json(snapshot))
}

#[derive(Debug, Deserialize)]
pub struct ExecRequest {
    pub command: String,
}

#[derive(Debug, Serialize)]
pub struct ExecResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub cwd: String,
    pub truncated: bool,
    pub timed_out: bool,
    pub exec_mode: String,
}

impl From<ExecOutcome> for ExecResponse {
    fn from(outcome: ExecOutcome) -> Self {
        Self {
            stdout: outcome.stdout,
            stderr: outcome.stderr,
            exit_code: outcome.exit_code,
            cwd: outcome.cwd,
            truncated: outcome.truncated,
            timed_out: outcome.timed_out,
            exec_mode: outcome.exec_mode,
        }
    }
}

pub async fn exec_command(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<ExecRequest>,
) -> AppResult<Json<ExecResponse>> {
    auth.require_admin()?;
    let command = body.command.trim();
    if command.is_empty() {
        return Err(AppError::BadRequest("command is required".into()));
    }
    if command.chars().count() > 4000 || command.contains('\0') {
        return Err(AppError::BadRequest("command is too long".into()));
    }

    let mode = exec::exec_mode(&state.config);
    let mut sessions = state.host.cwd.lock().await;
    let cwd = sessions
        .get(&auth.id)
        .cloned()
        .unwrap_or_else(|| exec::default_cwd(mode));
    let outcome = exec::run(mode, &cwd, command).await;
    sessions.insert(auth.id, outcome.cwd.clone());
    drop(sessions);

    if let Err(err) = sqlx::query(
        r#"INSERT INTO terminal_audit (id, user_id, command, exit_code, created_at)
           VALUES ($1, $2, $3, $4, $5)"#,
    )
    .bind(Uuid::new_v4())
    .bind(auth.id)
    .bind(command)
    .bind(outcome.exit_code)
    .bind(Utc::now())
    .execute(&state.pool)
    .await
    {
        tracing::warn!("terminal audit: {err}");
    }

    Ok(Json(outcome.into()))
}
