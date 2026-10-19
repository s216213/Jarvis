pub mod gemini;
pub mod limits;
pub mod provider;

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use crate::{
    config::Config,
    error::{AppError, AppResult},
};

use gemini::{GeminiProvider, pick_latest_model};
use provider::{ChatTurn, LlmProvider, ModelInfo};

#[derive(Clone, Debug)]
pub struct ActiveModel {
    pub provider: String,
    pub model: String,
    pub source: String, // auto | pinned
}

pub struct LlmRuntime {
    provider: Arc<dyn LlmProvider>,
    pinned: Option<String>,
    cache: RwLock<ModelCache>,
}

struct ModelCache {
    active: Option<ActiveModel>,
    models: Vec<ModelInfo>,
    fetched_at: Option<Instant>,
}

impl LlmRuntime {
    pub async fn from_config(config: &Config) -> AppResult<Self> {
        let provider: Arc<dyn LlmProvider> = match config.llm_provider.as_str() {
            "gemini" => {
                let key = config
                    .gemini_api_key
                    .clone()
                    .ok_or_else(|| AppError::BadRequest("GEMINI_API_KEY missing".into()))?;
                Arc::new(GeminiProvider::new(key))
            }
            "groq" | "openrouter" => {
                return Err(AppError::BadRequest(format!(
                    "LLM_PROVIDER={} is not configured yet",
                    config.llm_provider
                )));
            }
            other => {
                return Err(AppError::BadRequest(format!(
                    "unknown LLM_PROVIDER={other}"
                )));
            }
        };

        let runtime = Self {
            provider,
            pinned: config.llm_model.clone(),
            cache: RwLock::new(ModelCache {
                active: None,
                models: vec![],
                fetched_at: None,
            }),
        };
        runtime.refresh().await?;
        Ok(runtime)
    }

    pub fn provider_name(&self) -> &'static str {
        self.provider.name()
    }

    pub async fn refresh(&self) -> AppResult<ActiveModel> {
        let models = self.provider.list_models().await?;
        let active = if let Some(pin) = &self.pinned {
            ActiveModel {
                provider: self.provider.name().into(),
                model: pin.clone(),
                source: "pinned".into(),
            }
        } else {
            let picked = pick_latest_model(models.clone()).ok_or_else(|| {
                AppError::Upstream("no chat-capable models returned by provider".into())
            })?;
            ActiveModel {
                provider: self.provider.name().into(),
                model: picked.id,
                source: "auto".into(),
            }
        };

        tracing::info!(
            provider = %active.provider,
            model = %active.model,
            source = %active.source,
            "LLM model resolved"
        );

        let mut cache = self.cache.write().await;
        cache.models = models;
        cache.active = Some(active.clone());
        cache.fetched_at = Some(Instant::now());
        Ok(active)
    }

    async fn ensure_fresh(&self) -> AppResult<ActiveModel> {
        {
            let cache = self.cache.read().await;
            if let (Some(active), Some(at)) = (&cache.active, cache.fetched_at) {
                if at.elapsed() < Duration::from_secs(6 * 3600) {
                    return Ok(active.clone());
                }
            }
        }
        self.refresh().await
    }

    pub async fn status(&self) -> AppResult<(ActiveModel, Vec<ModelInfo>)> {
        let active = self.ensure_fresh().await?;
        let models = self.cache.read().await.models.clone();
        Ok((active, models))
    }

    pub async fn complete(&self, messages: &[ChatTurn]) -> AppResult<(String, ActiveModel)> {
        let active = self.ensure_fresh().await?;
        let reply = self.provider.complete(&active.model, messages).await?;
        Ok((reply, active))
    }
}
