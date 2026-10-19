use axum::{Json, extract::State};
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    llm::{
        limits,
        provider::ChatTurn,
    },
    models::message::{Message, SendMessageRequest},
    state::AppState,
};

#[derive(Serialize)]
pub struct SendMessageResponse {
    user_message: Message,
    assistant_message: Message,
    model: String,
}

pub async fn list_messages(
    State(state): State<AppState>,
    auth: AuthUser,
) -> AppResult<Json<Vec<Message>>> {
    let rows = sqlx::query_as::<_, Message>(
        r#"SELECT id, user_id, role, content, created_at
           FROM messages
           WHERE user_id = $1
           ORDER BY created_at ASC
           LIMIT 500"#,
    )
    .bind(auth.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("list messages: {e}")))?;
    Ok(Json(rows))
}

pub async fn send_message(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(body): Json<SendMessageRequest>,
) -> AppResult<Json<SendMessageResponse>> {
    let content = body.content.trim().to_string();
    if content.is_empty() {
        return Err(AppError::BadRequest("content is required".into()));
    }
    if content.chars().count() > state.config.llm_max_input_chars {
        return Err(AppError::BadRequest(format!(
            "content exceeds LLM_MAX_INPUT_CHARS ({})",
            state.config.llm_max_input_chars
        )));
    }

    limits::check_or_rate_limit(&state.pool, &state.config, auth.id).await?;

    let permit = state
        .llm_inflight
        .clone()
        .try_acquire_owned()
        .map_err(|_| AppError::RateLimited {
            message: "Too many concurrent LLM requests".into(),
            retry_after_sec: 5,
            scope: "global".into(),
        })?;

    let user_message = sqlx::query_as::<_, Message>(
        r#"INSERT INTO messages (id, user_id, role, content, created_at)
           VALUES ($1, $2, 'user', $3, $4)
           RETURNING id, user_id, role, content, created_at"#,
    )
    .bind(Uuid::new_v4())
    .bind(auth.id)
    .bind(&content)
    .bind(Utc::now())
    .fetch_one(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("insert user message: {e}")))?;

    let history = sqlx::query_as::<_, Message>(
        r#"SELECT id, user_id, role, content, created_at
           FROM messages
           WHERE user_id = $1
           ORDER BY created_at DESC
           LIMIT $2"#,
    )
    .bind(auth.id)
    .bind(state.config.llm_context_messages as i64)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| AppError::internal(format!("history: {e}")))?;

    let mut turns: Vec<ChatTurn> = history
        .into_iter()
        .rev()
        .map(|m| ChatTurn {
            role: m.role,
            content: m.content,
        })
        .collect();

    turns.insert(
        0,
        ChatTurn {
            role: "system".into(),
            content: format!(
                "You are Jarvis, a helpful personal assistant. The user's display name is {}.",
                auth.user.display_name
            ),
        },
    );

    let prompt_chars = content.chars().count() as i32;
    let result = state.llm.complete(&turns).await;
    drop(permit);

    match result {
        Ok((reply, active)) => {
            limits::record_usage(
                &state.pool,
                auth.id,
                &active.provider,
                &active.model,
                prompt_chars,
                true,
            )
            .await?;

            let assistant_message = sqlx::query_as::<_, Message>(
                r#"INSERT INTO messages (id, user_id, role, content, created_at)
                   VALUES ($1, $2, 'assistant', $3, $4)
                   RETURNING id, user_id, role, content, created_at"#,
            )
            .bind(Uuid::new_v4())
            .bind(auth.id)
            .bind(&reply)
            .bind(Utc::now())
            .fetch_one(&state.pool)
            .await
            .map_err(|e| AppError::internal(format!("insert assistant: {e}")))?;

            Ok(Json(SendMessageResponse {
                user_message,
                assistant_message,
                model: active.model,
            }))
        }
        Err(err) => {
            let (provider, model) = state
                .llm
                .status()
                .await
                .map(|(a, _)| (a.provider, a.model))
                .unwrap_or_else(|_| ("unknown".into(), "unknown".into()));
            let _ = limits::record_usage(
                &state.pool,
                auth.id,
                &provider,
                &model,
                prompt_chars,
                false,
            )
            .await;
            Err(err)
        }
    }
}
