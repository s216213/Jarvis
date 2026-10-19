use async_trait::async_trait;
use serde::Serialize;

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub supports_chat: bool,
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
