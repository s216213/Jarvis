use axum::{Json, extract::{Multipart, State}};
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    llm::{limits, provider::ChatTurn},
    models::{
        flag,
        message::{Message, SendMessageRequest},
    },
    speech,
    state::AppState,
};

#[derive(Serialize)]
pub struct SendMessageResponse {
    user_message: Message,
    assistant_message: Message,
    model: String,
}

#[derive(Serialize)]
pub struct VoiceResponse {
    user_message: Message,
    assistant_message: Message,
    model: String,
    audio_base64: Option<String>,
    audio_mime: Option<String>,
    speech_error: Option<String>,
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
    if flag::is_disabled(&state.pool, "chat_enabled").await? {
        return Err(AppError::BadRequest(
            "chat is disabled by a feature flag".into(),
        ));
    }
    if content.chars().count() > state.config.llm_max_input_chars {
        return Err(AppError::BadRequest(format!(
            "content exceeds LLM_MAX_INPUT_CHARS ({})",
            state.config.llm_max_input_chars
        )));
    }

    let turn = complete_turn(&state, &auth, &content, false).await?;
    Ok(Json(turn))
}

pub async fn send_voice(
    State(state): State<AppState>,
    auth: AuthUser,
    multipart: Multipart,
) -> AppResult<Json<VoiceResponse>> {
    if flag::is_disabled(&state.pool, "voice_enabled").await? {
        return Err(AppError::BadRequest(
            "voice is disabled by a feature flag".into(),
        ));
    }

    let (Some(stt), Some(tts)) = (
        state.config.speech_stt_url.clone(),
        state.config.speech_tts_url.clone(),
    ) else {
        return Err(AppError::BadRequest(
            "voice engine is not configured".into(),
        ));
    };

    let upload = speech::read_upload(multipart).await?;
    // Transcribe before any row is written. An empty or failed transcript
    // must not land in the chat history.
    let content = speech::transcribe(&stt, &upload.wav).await?;
    if content.chars().count() > state.config.llm_max_input_chars {
        return Err(AppError::BadRequest(format!(
            "content exceeds LLM_MAX_INPUT_CHARS ({})",
            state.config.llm_max_input_chars
        )));
    }

    let turn = complete_turn(&state, &auth, &content, true).await?;
    match speech::synthesize(&tts, &turn.assistant_message.content, &upload.voice).await {
        Ok(wav) => Ok(Json(VoiceResponse {
            user_message: turn.user_message,
            assistant_message: turn.assistant_message,
            model: turn.model,
            audio_base64: Some(speech::encode_wav(&wav)),
            audio_mime: Some("audio/wav".into()),
            speech_error: None,
        })),
        Err(err) => {
            tracing::warn!("kokoro synthesis failed after the reply was saved: {err}");
            Ok(Json(VoiceResponse {
                user_message: turn.user_message,
                assistant_message: turn.assistant_message,
                model: turn.model,
                audio_base64: None,
                audio_mime: None,
                speech_error: Some("Synteza mowy niedostępna.".into()),
            }))
        }
    }
}

async fn complete_turn(
    state: &AppState,
    auth: &AuthUser,
    content: &str,
    spoken: bool,
) -> AppResult<SendMessageResponse> {
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
    .bind(content)
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
            content: system_prompt(&auth.user.display_name, spoken),
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

            Ok(SendMessageResponse {
                user_message,
                assistant_message,
                model: active.model,
            })
        }
        Err(err) => {
            let (provider, model) = state
                .llm
                .status()
                .await
                .map(|status| (status.provider, status.model))
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

fn system_prompt(display_name: &str, spoken: bool) -> String {
    let mut prompt = format!(
        "You are Jarvis, a helpful personal assistant. The user's display name is {display_name}."
    );
    if spoken {
        prompt.push_str(
            " Reply in English, in one to three short spoken sentences. No Markdown and no lists. The user may have spoken Polish; understand them and answer in English.",
        );
    }
    prompt
}
