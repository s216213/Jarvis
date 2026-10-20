//! Free-tier pool. Jarvis picks one model per turn from every configured provider.
//! The choice follows the task, each model's free-tier quota, prompt size, and
//! cooldowns learned from provider errors. A failed call falls through to the
//! next best fit. The admin route `auto : auto` keeps that choice. Any other
//! provider or model forces the pool. `LLM_MODEL` only seeds the first saved route.

pub mod gemini;
pub mod groq;
pub mod limits;
pub mod provider;
pub mod route;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tokio::sync::RwLock;

use crate::{
    config::Config,
    error::{AppError, AppResult},
};

use gemini::GeminiProvider;
use groq::GroqProvider;
use provider::{model_rank, ChatTurn, LlmProvider, ModelInfo};
use route::{SlotUsage, plan, profile_turn};

const MAX_MODELS_PER_PROVIDER: usize = 8;
#[derive(Clone, Debug, Serialize)]
pub struct ActiveModel {
    pub provider: String,
    pub model: String,
    pub source: String, // routed | auto | forced
}

/// Admin choice. `auto` / `auto` leaves task routing on.
#[derive(Clone, Debug, Serialize)]
pub struct RoutePreference {
    pub provider: String,
    pub model: String,
}

impl RoutePreference {
    pub fn automatic() -> Self {
        Self {
            provider: "auto".into(),
            model: "auto".into(),
        }
    }

    pub fn is_automatic(&self) -> bool {
        self.provider == "auto" && self.model == "auto"
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PoolStatus {
    pub provider: String,
    pub model: String,
    pub source: String,
    pub route_provider: String,
    pub route_model: String,
    pub route_note: String,
    pub providers: Vec<String>,
    pub slots: Vec<ActiveModel>,
    pub models: Vec<ModelInfo>,
}

pub struct LlmRuntime {
    providers: Vec<Arc<dyn LlmProvider>>,
    preference: RwLock<RoutePreference>,
    cache: RwLock<PoolCache>,
    health: RwLock<HashMap<String, SlotHealth>>,
}

#[derive(Clone)]
struct Slot {
    provider_index: usize,
    model: String,
}

#[derive(Default)]
struct SlotHealth {
    hits: Vec<(Instant, u32)>,
    cooldown_until: Option<Instant>,
}

impl SlotHealth {
    fn usage(&mut self, now: Instant) -> SlotUsage {
        let day = Duration::from_secs(24 * 3600);
        let minute = Duration::from_secs(60);
        self.hits
            .retain(|(at, _)| now.saturating_duration_since(*at) < day);
        let mut rpm_used: u32 = 0;
        let mut tpm_used: u32 = 0;
        for (at, tokens) in &self.hits {
            if now.saturating_duration_since(*at) < minute {
                rpm_used += 1;
                tpm_used = tpm_used.saturating_add(*tokens);
            }
        }
        let cooldown_left_sec = self
            .cooldown_until
            .and_then(|until| until.checked_duration_since(now))
            .map(|left| left.as_secs().max(1))
            .unwrap_or(0);
        SlotUsage {
            rpm_used,
            rpd_used: self.hits.len() as u32,
            tpm_used,
            cooldown_left_sec,
        }
    }

    fn record(&mut self, now: Instant, tokens: u32) {
        self.hits.push((now, tokens));
    }

    fn cool_down(&mut self, now: Instant, secs: u64) {
        let until = now + Duration::from_secs(secs.max(1));
        if self.cooldown_until.is_none_or(|current| until > current) {
            self.cooldown_until = Some(until);
        }
    }
}

struct PoolCache {
    slots: Vec<Slot>,
    models: Vec<ModelInfo>,
    provider_summary: String,
    model_summary: String,
    source: String,
    route_note: String,
    fetched_at: Option<Instant>,
}

impl LlmRuntime {
    pub async fn from_config(config: &Config, preference: RoutePreference) -> AppResult<Self> {
        let mut providers: Vec<Arc<dyn LlmProvider>> = Vec::new();
        for name in &config.llm_providers {
            let provider: Arc<dyn LlmProvider> = match name.as_str() {
                "gemini" => {
                    let key = config.gemini_api_key.clone().ok_or_else(|| {
                        AppError::BadRequest("GEMINI_API_KEY missing".into())
                    })?;
                    Arc::new(GeminiProvider::new(key))
                }
                "groq" => {
                    let key = config
                        .groq_api_key
                        .clone()
                        .ok_or_else(|| AppError::BadRequest("GROQ_API_KEY missing".into()))?;
                    Arc::new(GroqProvider::new(key))
                }
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
            };
            providers.push(provider);
        }

        if providers.is_empty() {
            return Err(AppError::BadRequest("no LLM providers configured".into()));
        }

        let runtime = Self {
            providers,
            preference: RwLock::new(preference),
            cache: RwLock::new(PoolCache {
                slots: Vec::new(),
                models: Vec::new(),
                provider_summary: String::new(),
                model_summary: String::new(),
                source: "auto".into(),
                route_note: String::new(),
                fetched_at: None,
            }),
            health: RwLock::new(HashMap::new()),
        };
        runtime.refresh().await?;
        Ok(runtime)
    }

    pub async fn refresh(&self) -> AppResult<PoolStatus> {
        let mut groups: Vec<Vec<Slot>> = Vec::new();
        let mut models: Vec<ModelInfo> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        for (index, provider) in self.providers.iter().enumerate() {
            match provider.list_models().await {
                Ok(list) => {
                    let mut chat: Vec<ModelInfo> = list
                        .into_iter()
                        .filter(|m| m.supports_chat)
                        .map(|mut model| {
                            if model.provider.is_empty() {
                                model.provider = provider.name().into();
                            }
                            model
                        })
                        .collect();
                    chat.sort_by(|a, b| model_rank(&b.id).cmp(&model_rank(&a.id)));
                    if chat.is_empty() {
                        errors.push(format!("{}: no chat models", provider.name()));
                        continue;
                    }
                    groups.push(
                        chat.iter()
                            .map(|model| Slot {
                                provider_index: index,
                                model: model.id.clone(),
                            })
                            .collect(),
                    );
                    models.extend(chat);
                }
                Err(err) => {
                    tracing::warn!(provider = provider.name(), error = %err, "LLM model list failed");
                    errors.push(format!("{}: {err}", provider.name()));
                }
            }
        }

        if groups.is_empty() {
            return Err(AppError::Upstream(format!(
                "no chat models available ({})",
                errors.join("; ")
            )));
        }

        let preference = self.preference.read().await.clone();
        let provider_names: Vec<&str> = self.providers.iter().map(|provider| provider.name()).collect();
        let (slots, route_note) = match choose_slots(&groups, &provider_names, &preference) {
            Ok(slots) if !slots.is_empty() => (slots, String::new()),
            _ => {
                let capped = groups
                    .iter()
                    .map(|group| {
                        let mut copy = group.clone();
                        copy.truncate(MAX_MODELS_PER_PROVIDER);
                        copy
                    })
                    .collect();
                (
                    interleave(capped),
                    format!(
                        "route {} : {} is not available",
                        preference.provider, preference.model
                    ),
                )
            }
        };
        if slots.is_empty() {
            return Err(AppError::Upstream(format!(
                "no chat models available ({})",
                errors.join("; ")
            )));
        }

        let source = if !route_note.is_empty() {
            if slots.len() > 1 { "routed" } else { "auto" }
        } else if !preference.is_automatic() {
            "forced"
        } else if slots.len() > 1 {
            "routed"
        } else {
            "auto"
        };
        let provider_summary = provider_summary(&slots, &self.providers);
        let model_summary = model_summary(&slots);

        tracing::info!(
            providers = %provider_summary,
            models = slots.len(),
            source,
            route = %format!("{} : {}", preference.provider, preference.model),
            "LLM pool resolved"
        );

        let status = self.view_from(
            &slots,
            &models,
            &provider_summary,
            &model_summary,
            source,
            &preference,
            &route_note,
        );
        let mut cache = self.cache.write().await;
        cache.slots = slots;
        cache.models = models;
        cache.provider_summary = provider_summary;
        cache.model_summary = model_summary;
        cache.source = source.into();
        cache.route_note = route_note;
        cache.fetched_at = Some(Instant::now());
        Ok(status)
    }

    pub async fn apply_preference(&self, next: RoutePreference) -> AppResult<PoolStatus> {
        let previous = self.preference.read().await.clone();
        *self.preference.write().await = next;
        match self.refresh().await {
            Ok(status) if status.route_note.is_empty() => Ok(status),
            Ok(status) => {
                *self.preference.write().await = previous;
                self.refresh().await?;
                Err(AppError::BadRequest(status.route_note))
            }
            Err(err) => {
                *self.preference.write().await = previous;
                let _ = self.refresh().await;
                Err(err)
            }
        }
    }

    async fn ensure_fresh(&self) -> AppResult<()> {
        let stale = {
            let cache = self.cache.read().await;
            if cache.fetched_at.is_some_and(|at| at.elapsed() < Duration::from_secs(6 * 3600))
                && !cache.slots.is_empty()
            {
                return Ok(());
            }
            !cache.slots.is_empty()
        };
        match self.refresh().await {
            Ok(_) => Ok(()),
            Err(err) if stale => {
                tracing::warn!(error = %err, "LLM refresh failed, keeping the previous pool");
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub async fn status(&self) -> AppResult<PoolStatus> {
        self.ensure_fresh().await?;
        let preference = self.preference.read().await.clone();
        let cache = self.cache.read().await;
        Ok(self.view_from(
            &cache.slots,
            &cache.models,
            &cache.provider_summary,
            &cache.model_summary,
            &cache.source,
            &preference,
            &cache.route_note,
        ))
    }

    pub async fn complete(&self, messages: &[ChatTurn]) -> AppResult<(String, ActiveModel)> {
        self.ensure_fresh().await?;
        let (slots, source) = {
            let cache = self.cache.read().await;
            (cache.slots.clone(), cache.source.clone())
        };
        if slots.is_empty() {
            return Err(AppError::Upstream("LLM pool is empty".into()));
        }

        let profile = profile_turn(messages);
        let names: Vec<&str> = slots
            .iter()
            .map(|slot| self.providers[slot.provider_index].name())
            .collect();
        let model_ids: Vec<&str> = slots.iter().map(|slot| slot.model.as_str()).collect();
        let usage = {
            let mut health = self.health.write().await;
            let now = Instant::now();
            (0..slots.len())
                .map(|index| {
                    health
                        .entry(slot_key(names[index], model_ids[index]))
                        .or_default()
                        .usage(now)
                })
                .collect::<Vec<_>>()
        };

        let route_plan = plan(&profile, &names, &model_ids, &usage);

        if route_plan.picks.is_empty() {
            return Err(AppError::RateLimited {
                message: "Every free-tier model is at its quota".into(),
                retry_after_sec: route_plan.retry_after_sec,
                scope: "provider".into(),
            });
        }

        let mut last_err: Option<AppError> = None;
        for pick in &route_plan.picks {
            let slot = &slots[pick.index];
            let provider = &self.providers[slot.provider_index];
            match provider.complete(&slot.model, messages).await {
                Ok(reply) => {
                    self.remember(&provider.name(), &slot.model, profile.estimated_tokens.saturating_add(256), None)
                        .await;
                    tracing::info!(
                        task = ?profile.kind,
                        tokens = profile.estimated_tokens,
                        provider = provider.name(),
                        model = %slot.model,
                        score = pick.score,
                        "LLM routed"
                    );
                    return Ok((
                        reply,
                        ActiveModel {
                            provider: provider.name().into(),
                            model: slot.model.clone(),
                            source,
                        },
                    ));
                }
                Err(err) => {
                    let cooldown = match &err {
                        AppError::RateLimited { retry_after_sec, .. } => *retry_after_sec,
                        _ => 20,
                    };
                    self.remember(&provider.name(), &slot.model, profile.estimated_tokens, Some(cooldown))
                        .await;
                    tracing::warn!(
                        task = ?profile.kind,
                        provider = provider.name(),
                        model = %slot.model,
                        error = %err,
                        "LLM candidate failed, trying the next fit"
                    );
                    last_err = Some(err);
                }
            }
        }

        Err(last_err.unwrap_or_else(|| AppError::Upstream("LLM pool failed".into())))
    }

    async fn remember(&self, provider: &str, model: &str, tokens: u32, cooldown_sec: Option<u64>) {
        let mut health = self.health.write().await;
        let entry = health.entry(slot_key(provider, model)).or_default();
        let now = Instant::now();
        entry.record(now, tokens);
        if let Some(secs) = cooldown_sec {
            entry.cool_down(now, secs);
        }
    }

    fn view_from(
        &self,
        slots: &[Slot],
        models: &[ModelInfo],
        provider_summary: &str,
        model_summary: &str,
        source: &str,
        preference: &RoutePreference,
        route_note: &str,
    ) -> PoolStatus {
        let active_slots = slots
            .iter()
            .map(|slot| ActiveModel {
                provider: self.providers[slot.provider_index].name().into(),
                model: slot.model.clone(),
                source: source.into(),
            })
            .collect();
        PoolStatus {
            provider: provider_summary.into(),
            model: model_summary.into(),
            source: source.into(),
            route_provider: preference.provider.clone(),
            route_model: preference.model.clone(),
            route_note: route_note.into(),
            providers: self.providers.iter().map(|provider| provider.name().into()).collect(),
            slots: active_slots,
            models: models.to_vec(),
        }
    }
}

fn choose_slots(
    groups: &[Vec<Slot>],
    provider_names: &[&str],
    preference: &RoutePreference,
) -> Result<Vec<Slot>, ()> {
    if preference.model != "auto" {
        let mut found = Vec::new();
        for (index, group) in groups.iter().enumerate() {
            if preference.provider != "auto" && provider_names[index] != preference.provider {
                continue;
            }
            for slot in group {
                if slot.model == preference.model {
                    found.push(slot.clone());
                }
            }
        }
        return if found.is_empty() { Err(()) } else { Ok(found) };
    }

    if preference.provider != "auto" {
        let Some(index) = provider_names.iter().position(|name| *name == preference.provider) else {
            return Err(());
        };
        let Some(group) = groups.get(index) else {
            return Err(());
        };
        if group.is_empty() {
            return Err(());
        }
        let mut chosen = group.clone();
        chosen.truncate(MAX_MODELS_PER_PROVIDER);
        return Ok(chosen);
    }

    let capped = groups
        .iter()
        .map(|group| {
            let mut copy = group.clone();
            copy.truncate(MAX_MODELS_PER_PROVIDER);
            copy
        })
        .collect();
    Ok(interleave(capped))
}

fn provider_summary(slots: &[Slot], providers: &[Arc<dyn LlmProvider>]) -> String {
    let mut names = Vec::new();
    for slot in slots {
        let name = providers[slot.provider_index].name();
        if !names.iter().any(|existing: &String| existing == name) {
            names.push(name.to_string());
        }
    }
    names.join(" · ")
}

fn model_summary(slots: &[Slot]) -> String {
    match slots {
        [] => "brak".into(),
        [only] => only.model.clone(),
        many => format!("{} {}", many.len(), plural_modele(many.len())),
    }
}

fn slot_key(provider: &str, model: &str) -> String {
    format!("{provider}\n{model}")
}

fn plural_modele(n: usize) -> &'static str {
    let n10 = n % 10;
    let n100 = n % 100;
    if (2..5).contains(&n10) && !(12..15).contains(&n100) {
        "modele"
    } else {
        "modeli"
    }
}

fn interleave(groups: Vec<Vec<Slot>>) -> Vec<Slot> {
    let max = groups.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = Vec::new();
    for index in 0..max {
        for group in &groups {
            if let Some(slot) = group.get(index) {
                out.push(slot.clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{choose_slots, interleave, plural_modele, RoutePreference, Slot};

    fn slot(provider_index: usize, model: &str) -> Slot {
        Slot {
            provider_index,
            model: model.into(),
        }
    }

    #[test]
    fn pool_alternates_providers() {
        let mixed = interleave(vec![
            vec![slot(0, "gemini-a"), slot(0, "gemini-b")],
            vec![slot(1, "groq-a"), slot(1, "groq-b"), slot(1, "groq-c")],
        ]);
        let ids: Vec<_> = mixed.iter().map(|s| s.model.as_str()).collect();
        assert_eq!(
            ids,
            ["gemini-a", "groq-a", "gemini-b", "groq-b", "groq-c"]
        );
    }

    #[test]
    fn automatic_route_keeps_the_pool() {
        let groups = vec![
            vec![slot(0, "gemini-a"), slot(0, "gemini-b")],
            vec![slot(1, "groq-a")],
        ];
        let chosen = choose_slots(&groups, &["gemini", "groq"], &RoutePreference::automatic()).unwrap();
        let ids: Vec<_> = chosen.iter().map(|item| item.model.as_str()).collect();
        assert_eq!(ids, ["gemini-a", "groq-a", "gemini-b"]);
    }

    #[test]
    fn forced_provider_drops_the_other_one() {
        let groups = vec![
            vec![slot(0, "gemini-a")],
            vec![slot(1, "groq-a"), slot(1, "groq-b")],
        ];
        let preference = RoutePreference {
            provider: "groq".into(),
            model: "auto".into(),
        };
        let chosen = choose_slots(&groups, &["gemini", "groq"], &preference).unwrap();
        let ids: Vec<_> = chosen.iter().map(|item| item.model.as_str()).collect();
        assert_eq!(ids, ["groq-a", "groq-b"]);
    }

    #[test]
    fn forced_model_is_a_single_slot() {
        let groups = vec![
            vec![slot(0, "gemini-a"), slot(0, "gemini-b")],
            vec![slot(1, "groq-a")],
        ];
        let preference = RoutePreference {
            provider: "gemini".into(),
            model: "gemini-b".into(),
        };
        let chosen = choose_slots(&groups, &["gemini", "groq"], &preference).unwrap();
        assert_eq!(chosen.len(), 1);
        assert_eq!(chosen[0].model, "gemini-b");
    }

    #[test]
    fn unknown_force_is_rejected() {
        let groups = vec![vec![slot(0, "gemini-a")]];
        let preference = RoutePreference {
            provider: "auto".into(),
            model: "missing".into(),
        };
        assert!(choose_slots(&groups, &["gemini"], &preference).is_err());
    }

    #[test]
    fn polish_model_plural() {
        assert_eq!(plural_modele(2), "modele");
        assert_eq!(plural_modele(4), "modele");
        assert_eq!(plural_modele(5), "modeli");
        assert_eq!(plural_modele(12), "modeli");
        assert_eq!(plural_modele(22), "modele");
    }
}
