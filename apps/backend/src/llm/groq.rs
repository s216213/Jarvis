use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::{AppError, AppResult};

use super::provider::{ChatTurn, LlmProvider, ModelInfo};

const BASE: &str = "https://api.groq.com/openai/v1";

pub struct GroqProvider {
    api_key: String,
    client: reqwest::Client,
}

impl GroqProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ListModelsResponse {
    data: Option<Vec<GroqModel>>,
}

#[derive(Debug, Deserialize)]
struct GroqModel {
    id: String,
    #[serde(default)]
    active: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Option<Vec<Choice>>,
    error: Option<GroqError>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Option<ChoiceMessage>,
}

#[derive(Debug, Deserialize)]
struct ChoiceMessage {
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GroqError {
    message: Option<String>,
}

/// Speech, guard, and TTS models are not chat completions.
pub(crate) fn is_chat_model(id: &str) -> bool {
    let lower = id.to_lowercase();
    const BLOCK: &[&str] = &[
        "whisper",
        "orpheus",
        "prompt-guard",
        "playai",
        "canopylabs",
        "tts",
    ];
    !BLOCK.iter().any(|needle| lower.contains(needle))
}

#[async_trait]
impl LlmProvider for GroqProvider {
    fn name(&self) -> &'static str {
        "groq"
    }

    async fn list_models(&self) -> AppResult<Vec<ModelInfo>> {
        let resp = self
            .client
            .get(format!("{BASE}/models"))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| AppError::Upstream(format!("groq list: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Upstream(format!(
                "groq list HTTP {status}: {text}"
            )));
        }

        let body: ListModelsResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Upstream(format!("groq list json: {e}")))?;

        let models = body
            .data
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.active.unwrap_or(true) && is_chat_model(&m.id))
            .map(|m| ModelInfo {
                display_name: m.id.clone(),
                id: m.id,
                supports_chat: true,
                provider: "groq".into(),
            })
            .collect();

        Ok(models)
    }

    async fn complete(&self, model: &str, messages: &[ChatTurn]) -> AppResult<String> {
        let messages_json: Vec<_> = messages
            .iter()
            .map(|turn| {
                let role = match turn.role.as_str() {
                    "system" | "assistant" | "user" => turn.role.as_str(),
                    _ => "user",
                };
                json!({ "role": role, "content": turn.content })
            })
            .collect();

        let body = json!({
            "model": model,
            "messages": messages_json,
        });

        let resp = self
            .client
            .post(format!("{BASE}/chat/completions"))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| AppError::Upstream(format!("groq complete: {e}")))?;

        if resp.status().as_u16() == 429 {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(30);
            return Err(AppError::RateLimited {
                message: "Groq rate limited".into(),
                retry_after_sec: retry_after.min(60),
                scope: "provider".into(),
            });
        }

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| AppError::Upstream(format!("groq body: {e}")))?;

        if !status.is_success() {
            return Err(AppError::Upstream(format!("groq HTTP {status}: {text}")));
        }

        let parsed: ChatResponse = serde_json::from_str(&text)
            .map_err(|e| AppError::Upstream(format!("groq parse: {e}; body={text}")))?;

        if let Some(err) = parsed.error {
            return Err(AppError::Upstream(format!(
                "groq error: {}",
                err.message.unwrap_or_else(|| "unknown".into())
            )));
        }

        let reply = parsed
            .choices
            .unwrap_or_default()
            .into_iter()
            .filter_map(|c| c.message)
            .filter_map(|m| m.content)
            .collect::<Vec<_>>()
            .join("\n");

        if reply.trim().is_empty() {
            return Err(AppError::Upstream("groq returned empty reply".into()));
        }

        Ok(reply)
    }
}

#[cfg(test)]
mod tests {
    use super::is_chat_model;

    #[test]
    fn chat_models_skip_audio_and_guard() {
        assert!(is_chat_model("openai/gpt-oss-120b"));
        assert!(is_chat_model("qwen/qwen3.8-27b"));
        assert!(!is_chat_model("whisper-large-v3"));
        assert!(!is_chat_model("whisper-large-v3-turbo"));
        assert!(!is_chat_model("meta-llama/llama-prompt-guard-2-22m"));
        assert!(!is_chat_model("canopylabs/orpheus-v1-english"));
    }
}
