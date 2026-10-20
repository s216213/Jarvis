use axum::{Json, extract::{Multipart, State}};
use chrono::Utc;
use serde::Serialize;
use uuid::Uuid;

use crate::{
    auth::middleware::AuthUser,
    error::{AppError, AppResult},
    host::{apps, exec},
    llm::{limits, provider::ChatTurn},
    models::{
        flag,
        message::{Message, SendMessageRequest},
    },
    speech,
    state::AppState,
};

#[derive(Serialize)]
pub struct AppLaunch {
    app: String,
    /// `opened` on the API host, `failed` there, or `client` for the desktop app.
    status: String,
    error: Option<String>,
}

#[derive(Serialize)]
pub struct SendMessageResponse {
    user_message: Message,
    assistant_message: Message,
    model: String,
    launches: Vec<AppLaunch>,
}

#[derive(Serialize)]
pub struct VoiceResponse {
    user_message: Message,
    assistant_message: Message,
    model: String,
    audio_base64: Option<String>,
    audio_mime: Option<String>,
    speech_error: Option<String>,
    launches: Vec<AppLaunch>,
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

    let turn = complete_turn(&state, &auth, &content, false, body.device_fingerprint.as_deref()).await?;
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
    let started = std::time::Instant::now();
    let content = speech::transcribe(&stt, &upload.wav).await?;
    tracing::info!(
        ms = started.elapsed().as_millis() as u64,
        "voice transcribe"
    );
    if content.chars().count() > state.config.llm_max_input_chars {
        return Err(AppError::BadRequest(format!(
            "content exceeds LLM_MAX_INPUT_CHARS ({})",
            state.config.llm_max_input_chars
        )));
    }

    let thought = std::time::Instant::now();
    let turn = complete_turn(
        &state,
        &auth,
        &content,
        true,
        upload.device_fingerprint.as_deref(),
    )
    .await?;
    tracing::info!(ms = thought.elapsed().as_millis() as u64, "voice reply");
    let voiced = std::time::Instant::now();
    match speech::synthesize(&tts, &turn.assistant_message.content, &upload.voice).await {
        Ok(wav) => {
            tracing::info!(ms = voiced.elapsed().as_millis() as u64, "voice synthesize");
            Ok(Json(VoiceResponse {
                user_message: turn.user_message,
                assistant_message: turn.assistant_message,
                model: turn.model,
                audio_base64: Some(speech::encode_wav(&wav)),
                audio_mime: Some("audio/wav".into()),
                speech_error: None,
                launches: turn.launches,
            }))
        }
        Err(err) => {
            tracing::warn!("kokoro synthesis failed after the reply was saved: {err}");
            Ok(Json(VoiceResponse {
                user_message: turn.user_message,
                assistant_message: turn.assistant_message,
                model: turn.model,
                audio_base64: None,
                audio_mime: None,
                speech_error: Some("Synteza mowy niedostępna.".into()),
                launches: turn.launches,
            }))
        }
    }
}

async fn complete_turn(
    state: &AppState,
    auth: &AuthUser,
    content: &str,
    spoken: bool,
    device_fingerprint: Option<&str>,
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

    let apps_enabled = !flag::is_disabled(&state.pool, "apps_enabled").await?;
    let devices = apps::connected_devices(&state.pool, auth.id).await?;
    let sender = device_fingerprint.and_then(|fingerprint| {
        devices
            .iter()
            .find(|device| device.fingerprint.eq_ignore_ascii_case(fingerprint.trim()))
    });
    turns.insert(
        0,
        ChatTurn {
            role: "system".into(),
            content: system_prompt(
                &auth.user.display_name,
                spoken,
                apps_enabled,
                &devices,
                sender,
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

            let (reply, launches, handoff_device) = prepare_reply(
                &state.pool,
                auth.id,
                &reply,
                apps_enabled,
                exec::exec_mode(&state.config) == "host",
                device_fingerprint,
                &devices,
            )
            .await;
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

            if let Some(device_id) = handoff_device {
                if let Err(err) =
                    apps::enqueue_handoff(&state.pool, auth.id, device_id, &assistant_message.content)
                        .await
                {
                    tracing::warn!(error = %err, "handoff queue failed");
                }
            }

            Ok(SendMessageResponse {
                user_message,
                assistant_message,
                model: active.model,
                launches,
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

fn system_prompt(
    display_name: &str,
    spoken: bool,
    apps: bool,
    devices: &[apps::ConnectedDevice],
    sender: Option<&apps::ConnectedDevice>,
) -> String {
    let mut prompt = format!(
        "You are Jarvis, a helpful personal assistant. The user's display name is {display_name}."
    );
    if spoken {
        prompt.push_str(
            " Reply in English, in normal complete sentences, as long as the answer needs. When you enumerate items, use a short markdown list. The user may have spoken Polish; understand them and answer in English.",
        );
    }
    if apps {
        prompt.push_str(
            " When the user asks to open or start an application, open it on their connected clients. End the reply with one line per application, exactly [[launch:NAME]]. NAME is a Windows executable such as notepad, calc, explorer, chrome, msedge, firefox, code, spotify, or wt, an ms-settings: page, an https URL, or a full path to an .exe. Only if they explicitly ask for the OCI VM, the virtual machine, or the server, also emit [[launch-vm:NAME]] with a Linux program name such as firefox, google-chrome, or code, or an absolute Linux path. If they ask only for that machine, emit launch-vm and not launch. No arguments and no shell syntax. Do not emit a launch marker unless they asked to open something.",
        );
    }
    if !devices.is_empty() {
        match sender {
            Some(device) => prompt.push_str(&format!(
                " The user is speaking now from {} \"{}\" system {} fingerprint {}.",
                device.platform, device.name, device.system_name, device.fingerprint
            )),
            None => prompt.push_str(
                " The user is speaking from an unknown client that cannot open programs or local files.",
            ),
        }
        prompt.push_str(" Connected devices:");
        for device in devices {
            prompt.push_str(&format!(
                " {} \"{}\" system {} fingerprint {} ({});",
                device.platform,
                device.name,
                device.system_name,
                device.fingerprint,
                device_can_do(device)
            ));
        }
        let only_here = devices.len() == 1
            && sender.is_some_and(|device| device.fingerprint == devices[0].fingerprint);
        if only_here {
            prompt.push_str(" This is the only connected device. Do not emit a handoff marker.");
        } else {
            prompt.push_str(
                " You decide when this turn must continue on another device. Add [[handoff:FINGERPRINT]] for a connected device that can finish the request, even when the user never says to switch. Do that for a local program, file, folder, desktop, Windows settings, sound on the computer, or any task the current device cannot do. Polish examples that require it from the browser: „otwórz notatnik”, „włącz muzykę na komputerze”, „pokaż pobrane”, „wejdź w ustawienia”. Stay on the current device for questions, explanations, and work it can already finish, such as „jaka jest pogoda” or „wyjaśnij to”. Do not hand off to the device the user is already on. One turn has one destination: if a Windows program must open, that destination is the Windows desktop. Say in one short sentence which device you are moving to. Markers are removed before the user sees or hears the reply.",
            );
        }
    }
    prompt
}

fn device_can_do(device: &apps::ConnectedDevice) -> &'static str {
    if apps::is_windows_desktop(device) {
        "opens Windows programs, local files, and the desktop"
    } else if device.platform == "desktop" {
        "opens local programs and files"
    } else if device.platform == "android" {
        "phone, chat, and local Android apps"
    } else if device.platform == "web" {
        "browser chat only, cannot open programs or local files"
    } else {
        "connected client"
    }
}

async fn prepare_reply(
    pool: &crate::db::DbPool,
    user_id: uuid::Uuid,
    reply: &str,
    apps_enabled: bool,
    on_oci_host: bool,
    device_fingerprint: Option<&str>,
    devices: &[apps::ConnectedDevice],
) -> (String, Vec<AppLaunch>, Option<uuid::Uuid>) {
    let plan = apps::take_launches(reply);
    let mut content = plan.text;
    let planned = if apps_enabled {
        plan.launches
    } else {
        Vec::new()
    };
    let sender = device_fingerprint.and_then(|fingerprint| {
        devices
            .iter()
            .find(|device| device.fingerprint.eq_ignore_ascii_case(fingerprint.trim()))
    });
    let decision = apps::choose_handoff(devices, sender, plan.handoff.as_deref(), &planned);
    let target = decision.target;
    let unresolved_handoff = decision.unresolved;
    let windows_needed = decision.windows_needed;
    let client_launch = planned.iter().any(|item| !item.on_vm);
    let target_id = target.map(|device| device.id);
    let switching = target_id.is_some() && target_id != sender.map(|device| device.id);
    let pin_launch = plan.handoff.is_some()
        || windows_needed
        || (client_launch && !sender.is_some_and(apps::can_launch));

    let mut launches = Vec::with_capacity(planned.len());
    let mut failures = Vec::new();
    let mut queued_client = false;
    for item in planned {
        if item.on_vm {
            match apps::open_on_vm(&item.app, on_oci_host).await {
                Ok(()) => {
                    tracing::info!(app = %item.app, "app launched on the OCI VM");
                    launches.push(AppLaunch {
                        app: item.app,
                        status: "opened".into(),
                        error: None,
                    });
                }
                Err(err) => {
                    tracing::warn!(app = %item.app, error = %err, "OCI app launch failed");
                    failures.push(format!("Nie udało się uruchomić {} na OCI.", item.app));
                    launches.push(AppLaunch {
                        app: item.app,
                        status: "failed".into(),
                        error: Some(err),
                    });
                }
            }
            continue;
        }

        let device_for_launch = if pin_launch { target_id } else { None };
        match apps::enqueue_client(pool, user_id, &item.app, device_for_launch).await {
            Ok(()) => {
                queued_client = true;
                tracing::info!(app = %item.app, "app queued for connected clients");
                launches.push(AppLaunch {
                    app: item.app,
                    status: "client".into(),
                    error: None,
                });
            }
            Err(err) => {
                tracing::warn!(app = %item.app, error = %err, "app queue failed");
                failures.push(format!("Nie udało się przekazać {} do klientów.", item.app));
                launches.push(AppLaunch {
                    app: item.app,
                    status: "failed".into(),
                    error: Some(err.to_string()),
                });
            }
        }
    }

    if unresolved_handoff {
        failures.push("Nie znalazłem tego urządzenia wśród podpiętych.".into());
    }
    if queued_client && target_id.is_none() {
        match apps::capable_clients(pool, user_id).await {
            Ok(0) => failures.push(
                "Żaden podpięty klient nie może otworzyć programu. Uruchom aplikację desktopową."
                    .into(),
            ),
            Ok(_) => {}
            Err(err) => tracing::warn!(error = %err, "client count failed"),
        }
    }
    if windows_needed && target_id.is_none() {
        failures.push(
            "Żaden podpięty desktop Windows nie może dokończyć tego programu.".into(),
        );
    }
    if switching {
        if let Some(device) = target {
            if !content.to_lowercase().contains(&device.name.to_lowercase()) {
                failures.push(format!("Przełączam się na {}.", device.name));
            }
        }
    }

    if !failures.is_empty() {
        if !content.is_empty() {
            content.push_str("\n\n");
        }
        content.push_str(&failures.join(" "));
    }
    if content.trim().is_empty() {
        content = if launches.is_empty() {
            "Nie mam odpowiedzi.".into()
        } else {
            let names = launches
                .iter()
                .map(|launch| launch.app.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            format!("Uruchamiam {names}.")
        };
    }
    let handoff_device = if switching { target_id } else { None };
    (content, launches, handoff_device)
}
