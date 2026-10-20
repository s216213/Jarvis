use crate::error::{AppError, AppResult};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub port: u16,
    pub jwt_secret: String,
    pub cors_origins: Vec<String>,
    pub bootstrap_admin_email: Option<String>,
    pub bootstrap_admin_password: Option<String>,
    /// Providers in the free-tier pool, e.g. `["gemini", "groq"]`.
    pub llm_providers: Vec<String>,
    pub gemini_api_key: Option<String>,
    pub groq_api_key: Option<String>,
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

        let gemini_api_key = std::env::var("GEMINI_API_KEY").ok().filter(|s| !s.is_empty());
        let groq_api_key = std::env::var("GROQ_API_KEY").ok().filter(|s| !s.is_empty());
        let llm_providers = resolve_llm_providers(
            std::env::var("LLM_PROVIDERS").ok().as_deref(),
            std::env::var("LLM_PROVIDER").ok().as_deref(),
            gemini_api_key.is_some(),
            groq_api_key.is_some(),
        )?;

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
            llm_providers,
            gemini_api_key,
            groq_api_key,
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

/// `LLM_PROVIDERS` is an explicit allow-list. When it is empty, every provider
/// that has an API key joins the pool — free-tier quotas are separate, so Jarvis
/// uses them together. `LLM_PROVIDER` only names the provider to require when
/// no key is set yet.
pub(crate) fn resolve_llm_providers(
    explicit: Option<&str>,
    legacy: Option<&str>,
    has_gemini: bool,
    has_groq: bool,
) -> AppResult<Vec<String>> {
    let mut list = if let Some(raw) = explicit.map(str::trim).filter(|s| !s.is_empty()) {
        raw.split(',')
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        let mut discovered = Vec::new();
        if has_gemini {
            discovered.push("gemini".into());
        }
        if has_groq {
            discovered.push("groq".into());
        }
        if discovered.is_empty() {
            let name = legacy
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("gemini")
                .to_lowercase();
            discovered.push(name);
        }
        discovered
    };

    let mut seen = std::collections::HashSet::new();
    list.retain(|name| seen.insert(name.clone()));

    if list.is_empty() {
        return Err(AppError::BadRequest("no LLM providers configured".into()));
    }

    for name in &list {
        match name.as_str() {
            "gemini" if !has_gemini => {
                return Err(AppError::BadRequest(
                    "GEMINI_API_KEY is required for the gemini provider".into(),
                ));
            }
            "groq" if !has_groq => {
                return Err(AppError::BadRequest(
                    "GROQ_API_KEY is required for the groq provider".into(),
                ));
            }
            "gemini" | "groq" => {}
            "openrouter" => {
                return Err(AppError::BadRequest(
                    "LLM provider openrouter is not configured yet".into(),
                ));
            }
            other => {
                return Err(AppError::BadRequest(format!(
                    "unknown LLM provider {other}"
                )));
            }
        }
    }

    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::resolve_llm_providers;

    #[test]
    fn keys_join_the_pool_without_an_allow_list() {
        let providers = resolve_llm_providers(None, Some("gemini"), true, true).unwrap();
        assert_eq!(providers, ["gemini", "groq"]);
    }

    #[test]
    fn explicit_list_can_keep_a_single_provider() {
        let providers =
            resolve_llm_providers(Some("groq"), Some("gemini"), true, true).unwrap();
        assert_eq!(providers, ["groq"]);
    }

    #[test]
    fn missing_key_for_named_provider_is_an_error() {
        let err = resolve_llm_providers(Some("gemini,groq"), None, true, false).unwrap_err();
        assert!(err.to_string().contains("GROQ_API_KEY"));
    }
}
