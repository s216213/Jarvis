use std::sync::Arc;

use tokio::sync::{Mutex, Semaphore};

use crate::{config::Config, db::DbPool, host::HostMonitor, llm::LlmRuntime};

#[derive(Clone)]
pub struct AppState {
    pub pool: DbPool,
    pub config: Arc<Config>,
    pub llm: Arc<LlmRuntime>,
    pub llm_inflight: Arc<Semaphore>,
    pub login_limiter: Arc<Mutex<LoginLimiter>>,
    pub host: Arc<HostMonitor>,
}

/// Simple in-memory login rate limiter (per IP key).
pub struct LoginLimiter {
    hits: std::collections::HashMap<String, Vec<std::time::Instant>>,
}

impl LoginLimiter {
    pub fn new() -> Self {
        Self {
            hits: std::collections::HashMap::new(),
        }
    }

    pub fn check(&mut self, key: &str, max_per_minute: usize) -> bool {
        let now = std::time::Instant::now();
        let window = std::time::Duration::from_secs(60);
        let entry = self.hits.entry(key.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < window);
        if entry.len() >= max_per_minute {
            return false;
        }
        entry.push(now);
        true
    }
}
