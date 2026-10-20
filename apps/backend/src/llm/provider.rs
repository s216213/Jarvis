use async_trait::async_trait;
use serde::Serialize;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub supports_chat: bool,
    /// Which provider listed this model. Stamped by the runtime pool.
    #[serde(default)]
    pub provider: String,
}

#[derive(Debug, Clone)]
pub struct ChatTurn {
    pub role: String,
    pub content: String,
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn list_models(&self) -> AppResult<Vec<ModelInfo>>;
    async fn complete(&self, model: &str, messages: &[ChatTurn]) -> AppResult<String>;
}

/// Higher tuple sorts first. Used to order a provider's chat models in the pool.
pub(crate) fn model_rank(id: &str) -> (i32, i32, String) {
    let lower = id.to_lowercase();
    let nums: Vec<i32> = lower
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse().ok())
        .collect();
    let major = nums.first().copied().unwrap_or(0);
    let minor = nums.get(1).copied().unwrap_or(0);
    let boost = if lower.contains("flash") {
        2
    } else if lower.contains("pro") {
        1
    } else {
        0
    };
    (major + boost * 10, minor, lower)
}
