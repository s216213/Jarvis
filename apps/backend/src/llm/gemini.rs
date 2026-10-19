use async_trait::async_trait;
use serde::Deserialize;
use serde_json::json;

use crate::error::{AppError, AppResult};

use super::provider::{ChatTurn, LlmProvider, ModelInfo};

pub struct GeminiProvider {
    api_key: String,
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    fn model_id(raw: &str) -> String {
        raw.strip_prefix("models/").unwrap_or(raw).to_string()
    }
}

#[derive(Debug, Deserialize)]
struct ListModelsResponse {
    models: Option<Vec<GeminiModel>>,
}

#[derive(Debug, Deserialize)]
struct GeminiModel {
    name: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    #[serde(rename = "supportedGenerationMethods")]
    supported_generation_methods: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct GenerateResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<GeminiErrorBody>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: Option<ContentBody>,
}

#[derive(Debug, Deserialize)]
struct ContentBody {
    parts: Option<Vec<PartBody>>,
}

#[derive(Debug, Deserialize)]
struct PartBody {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GeminiErrorBody {
    message: Option<String>,
    code: Option<i64>,
    status: Option<String>,
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    fn name(&self) -> &'static str {
        "gemini"
    }

    async fn list_models(&self) -> AppResult<Vec<ModelInfo>> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models?key={}",
            self.api_key
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| AppError::Upstream(format!("gemini list: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(AppError::Upstream(format!(
                "gemini list HTTP {status}: {text}"
            )));
        }

        let body: ListModelsResponse = resp
            .json()
            .await
            .map_err(|e| AppError::Upstream(format!("gemini list json: {e}")))?;

        let models = body
            .models
            .unwrap_or_default()
            .into_iter()
            .filter_map(|m| {
                let methods = m.supported_generation_methods.unwrap_or_default();
                let supports_chat = methods.iter().any(|x| x == "generateContent");
                if !supports_chat {
                    return None;
                }
                let id = Self::model_id(&m.name);
                // Prefer generative chat models; skip embedding / embedding-like ids
                let lower = id.to_lowercase();
                if lower.contains("embed") || lower.contains("aqa") || lower.contains("tts") {
                    return None;
                }
                Some(ModelInfo {
                    display_name: m.display_name.unwrap_or_else(|| id.clone()),
                    id,
                    supports_chat: true,
                })
            })
            .collect();

        Ok(models)
    }

    async fn complete(&self, model: &str, messages: &[ChatTurn]) -> AppResult<String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={}",
            self.api_key
        );

        let mut contents = Vec::new();
        let mut system_instruction: Option<serde_json::Value> = None;

        for turn in messages {
            if turn.role == "system" {
                system_instruction = Some(json!({
                    "parts": [{ "text": turn.content }]
                }));
                continue;
            }
            let role = if turn.role == "assistant" {
                "model"
            } else {
                "user"
            };
            contents.push(json!({
                "role": role,
                "parts": [{ "text": turn.content }]
            }));
        }

        let mut body = json!({ "contents": contents });
        if let Some(si) = system_instruction {
            body["systemInstruction"] = si;
        }

        let do_request = || async {
            self.client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| AppError::Upstream(format!("gemini complete: {e}")))
        };

        let mut resp = do_request().await?;
        if resp.status().as_u16() == 429 {
            let retry_after = resp
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(2);
            tokio::time::sleep(std::time::Duration::from_secs(retry_after.min(10))).await;
            resp = do_request().await?;
        }

        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| AppError::Upstream(format!("gemini body: {e}")))?;

        if status.as_u16() == 429 {
            return Err(AppError::RateLimited {
                message: "Gemini rate limited".into(),
                retry_after_sec: 30,
                scope: "provider".into(),
            });
        }

        if !status.is_success() {
            return Err(AppError::Upstream(format!(
                "gemini HTTP {status}: {text}"
            )));
        }

        let parsed: GenerateResponse = serde_json::from_str(&text)
            .map_err(|e| AppError::Upstream(format!("gemini parse: {e}; body={text}")))?;

        if let Some(err) = parsed.error {
            return Err(AppError::Upstream(format!(
                "gemini error: {:?}",
                err.message.or(err.status)
            )));
        }

        let reply = parsed
            .candidates
            .unwrap_or_default()
            .into_iter()
            .flat_map(|c| c.content.into_iter())
            .flat_map(|c| c.parts.into_iter())
            .flatten()
            .filter_map(|p| p.text)
            .collect::<Vec<_>>()
            .join("\n");

        if reply.trim().is_empty() {
            return Err(AppError::Upstream("gemini returned empty reply".into()));
        }

        Ok(reply)
    }
}

/// Prefer newest-looking model ids (higher version / flash-latest style).
pub fn pick_latest_model(mut models: Vec<ModelInfo>) -> Option<ModelInfo> {
    if models.is_empty() {
        return None;
    }
    models.sort_by(|a, b| score_model(&b.id).cmp(&score_model(&a.id)));
    models.into_iter().next()
}

fn score_model(id: &str) -> (i32, i32, String) {
    let lower = id.to_lowercase();
    let mut major = 0;
    let mut minor = 0;
    // Extract first two numbers: gemini-2.5-flash -> 2, 5
    let nums: Vec<i32> = lower
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse().ok())
        .collect();
    if let Some(n) = nums.first() {
        major = *n;
    }
    if let Some(n) = nums.get(1) {
        minor = *n;
    }
    let boost = if lower.contains("flash") {
        2
    } else if lower.contains("pro") {
        1
    } else {
        0
    };
    (major + boost * 10, minor, lower)
}
