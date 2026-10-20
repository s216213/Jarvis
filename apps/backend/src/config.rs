use crate::error::{AppError, AppResult};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub jwt_secret: String,
    pub cors_origins: Vec<String>,
    pub bootstrap_admin_email: Option<String>,
    pub bootstrap_admin_password: Option<String>,
    pub llm_provider: String,
    pub gemini_api_key: Option<String>,
    pub llm_model: Option<String>,
    pub llm_user_rpm: u32,
    pub llm_user_daily: u32,
    pub llm_global_rpm: u32,
    pub llm_global_daily: u32,
    pub llm_max_input_chars: usize,
    pub llm_context_messages: usize,
    pub llm_max_inflight: usize,
    /// `host` runs the admin terminal in the OCI VM namespaces (nsenter).
    /// `container` runs it in the API process.
    pub host_exec: String,
}

impl Config {
    pub fn from_env() -> AppResult<Self> {
        let _ = dotenvy::dotenv();

        let database_url = required("DATABASE_URL")?;
        let jwt_secret = required("JWT_SECRET")?;
        if jwt_secret.len() < 16 {
            return Err(AppError::BadRequest(
                "JWT_SECRET must be at least 16 characters".into(),
            ));
        }

        let cors_origins = std::env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000".into())
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let llm_provider = std::env::var("LLM_PROVIDER")
            .unwrap_or_else(|_| "gemini".into())
            .to_lowercase();

        let gemini_api_key = std::env::var("GEMINI_API_KEY").ok().filter(|s| !s.is_empty());
        if llm_provider == "gemini" && gemini_api_key.is_none() {
            return Err(AppError::BadRequest(
                "GEMINI_API_KEY is required when LLM_PROVIDER=gemini".into(),
            ));
        }

        Ok(Self {
            database_url,
            port: parse_or("PORT", 8080),
            jwt_secret,
            cors_origins,
            bootstrap_admin_email: std::env::var("BOOTSTRAP_ADMIN_EMAIL")
                .ok()
                .filter(|s| !s.is_empty()),
            bootstrap_admin_password: std::env::var("BOOTSTRAP_ADMIN_PASSWORD")
                .ok()
                .filter(|s| !s.is_empty()),
            llm_provider,
            gemini_api_key,
            llm_model: std::env::var("LLM_MODEL").ok().filter(|s| !s.is_empty()),
            llm_user_rpm: parse_or("LLM_USER_RPM", 5),
            llm_user_daily: parse_or("LLM_USER_DAILY", 50),
            llm_global_rpm: parse_or("LLM_GLOBAL_RPM", 20),
            llm_global_daily: parse_or("LLM_GLOBAL_DAILY", 200),
            llm_max_input_chars: parse_or("LLM_MAX_INPUT_CHARS", 4000),
            llm_context_messages: parse_or("LLM_CONTEXT_MESSAGES", 20),
            llm_max_inflight: parse_or("LLM_MAX_INFLIGHT", 3),
            host_exec: std::env::var("JARVIS_HOST_EXEC")
                .unwrap_or_else(|_| "container".into())
                .trim()
                .to_lowercase(),
        })
    }
}

fn required(key: &str) -> AppResult<String> {
    std::env::var(key).map_err(|_| AppError::BadRequest(format!("missing env {key}")))
}

fn parse_or<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
