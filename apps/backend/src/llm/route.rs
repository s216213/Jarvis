use super::provider::ChatTurn;

/// What the next reply has to do. New signals belong in [`profile_turn`] and [`score_ready`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskKind {
    /// Short chat. Prefer a fast model and keep stronger quotas for harder turns.
    Light,
    /// Code, reasoning, or a long thread. Prefer a stronger model that can hold the prompt.
    Heavy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnProfile {
    pub kind: TaskKind,
    pub estimated_tokens: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct QuotaHint {
    pub rpm: u32,
    pub rpd: u32,
    pub tpm: u32,
}

/// Recent local usage plus a cooldown learned from a real provider error.
#[derive(Clone, Copy, Debug)]
pub struct SlotUsage {
    pub rpm_used: u32,
    pub rpd_used: u32,
    pub tpm_used: u32,
    pub cooldown_left_sec: u64,
}

#[derive(Clone, Debug)]
pub struct Pick {
    pub index: usize,
    pub score: i32,
}

#[derive(Clone, Debug)]
pub struct Plan {
    pub picks: Vec<Pick>,
    /// Set when every model is blocked by quota or cooldown.
    pub retry_after_sec: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ModelClass {
    Fast,
    Mid,
    Strong,
}

pub fn profile_turn(messages: &[ChatTurn]) -> TurnProfile {
    let estimated_tokens = estimate_tokens(messages);
    let latest_user = messages
        .iter()
        .rev()
        .find(|turn| turn.role == "user")
        .map(|turn| turn.content.as_str())
        .unwrap_or("");
    let kind = if estimated_tokens >= 2_000 || is_heavy_text(latest_user) {
        TaskKind::Heavy
    } else {
        TaskKind::Light
    };
    TurnProfile {
        kind,
        estimated_tokens,
    }
}

pub fn quota_hint(provider: &str, model: &str) -> QuotaHint {
    let id = model.to_lowercase();
    match provider {
        // Groq free tier, published per model (checked Oct 2026). A real 429 overrides this.
        "groq" => {
            if id.contains("safeguard") {
                QuotaHint {
                    rpm: 3,
                    rpd: 1_000,
                    tpm: 2_000,
                }
            } else {
                QuotaHint {
                    rpm: 30,
                    rpd: 1_000,
                    tpm: 8_000,
                }
            }
        }
        // Gemini AI Studio free tier is per model and moves. These are conservative hints.
        "gemini" => {
            if id.contains("pro") {
                QuotaHint {
                    rpm: 5,
                    rpd: 100,
                    tpm: 250_000,
                }
            } else if id.contains("lite") {
                QuotaHint {
                    rpm: 15,
                    rpd: 1_000,
                    tpm: 250_000,
                }
            } else {
                QuotaHint {
                    rpm: 10,
                    rpd: 250,
                    tpm: 250_000,
                }
            }
        }
        _ => QuotaHint {
            rpm: 5,
            rpd: 100,
            tpm: 8_000,
        },
    }
}

pub fn plan(profile: &TurnProfile, providers: &[&str], models: &[&str], usage: &[SlotUsage]) -> Plan {
    let mut picks = Vec::new();
    let mut retry_after_sec = u64::MAX;

    for index in 0..models.len() {
        let hint = quota_hint(providers[index], models[index]);
        let used = usage.get(index).copied().unwrap_or(SlotUsage {
            rpm_used: 0,
            rpd_used: 0,
            tpm_used: 0,
            cooldown_left_sec: 0,
        });
        match availability(profile, models[index], hint, used) {
            Availability::Ready(score) => picks.push(Pick { index, score }),
            Availability::Blocked { retry_after_sec: wait } => {
                retry_after_sec = retry_after_sec.min(wait.max(1));
            }
        }
    }

    picks.sort_by(|a, b| {
        b.score.cmp(&a.score).then_with(|| {
            super::provider::model_rank(models[b.index]).cmp(&super::provider::model_rank(models[a.index]))
        })
    });

    Plan {
        retry_after_sec: if picks.is_empty() {
            if retry_after_sec == u64::MAX {
                60
            } else {
                retry_after_sec
            }
        } else {
            0
        },
        picks,
    }
}

enum Availability {
    Ready(i32),
    Blocked { retry_after_sec: u64 },
}

fn availability(profile: &TurnProfile, model: &str, hint: QuotaHint, usage: SlotUsage) -> Availability {
    if usage.cooldown_left_sec > 0 {
        return Availability::Blocked {
            retry_after_sec: usage.cooldown_left_sec,
        };
    }
    if usage.rpd_used >= hint.rpd {
        return Availability::Blocked {
            retry_after_sec: 3_600,
        };
    }
    if usage.rpm_used >= hint.rpm {
        return Availability::Blocked {
            retry_after_sec: 60,
        };
    }
    if usage.tpm_used.saturating_add(profile.estimated_tokens) > hint.tpm {
        return Availability::Blocked {
            retry_after_sec: 60,
        };
    }
    Availability::Ready(score_ready(profile, model_class(model), hint, usage))
}

/// Task fit outweighs spare quota, so a fresh strong model does not steal an easy turn.
fn score_ready(profile: &TurnProfile, class: ModelClass, hint: QuotaHint, usage: SlotUsage) -> i32 {
    let mut score = match (profile.kind, class) {
        (TaskKind::Light, ModelClass::Fast) => 100,
        (TaskKind::Light, ModelClass::Mid) => 60,
        (TaskKind::Light, ModelClass::Strong) => 20,
        (TaskKind::Heavy, ModelClass::Strong) => 100,
        (TaskKind::Heavy, ModelClass::Mid) => 70,
        (TaskKind::Heavy, ModelClass::Fast) => 25,
    };
    score += rpm_headroom(hint.rpm, usage.rpm_used);
    score += rpd_headroom(hint.rpd, usage.rpd_used);
    if hint.tpm / 4 >= profile.estimated_tokens {
        score += 10;
    }
    score
}

fn rpm_headroom(limit: u32, used: u32) -> i32 {
    if limit == 0 {
        return 0;
    }
    let left = limit.saturating_sub(used);
    ((left.saturating_mul(40)) / limit) as i32
}

fn rpd_headroom(limit: u32, used: u32) -> i32 {
    if limit == 0 {
        return 0;
    }
    let left = limit.saturating_sub(used);
    ((left.saturating_mul(20)) / limit) as i32
}

fn model_class(id: &str) -> ModelClass {
    let lowered = id.to_lowercase();
    let parts: Vec<&str> = lowered
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .collect();
    let has = |marker: &str| parts.iter().any(|part| *part == marker);
    if has("120b") || has("70b") || has("pro") || has("reason") || has("scout") {
        ModelClass::Strong
    } else if has("lite") || has("mini") || has("instant") || has("8b") || has("20b") || has("small")
    {
        ModelClass::Fast
    } else {
        ModelClass::Mid
    }
}

fn is_heavy_text(text: &str) -> bool {
    if text.contains("```") || text.chars().count() > 500 {
        return true;
    }
    let lower = text.to_lowercase();
    const MARKERS: &[&str] = &[
        "kod",
        "code",
        "debug",
        "refaktor",
        "refactor",
        "implement",
        "wyjaśnij",
        "wyjasnij",
        "wytłumacz",
        "wytlumacz",
        "porównaj",
        "porownaj",
        "analiz",
        "dlaczego",
        "why ",
        "how does",
        "architekt",
        "zaprojektuj",
        "napisz funkc",
        "napisz klas",
        "stack trace",
        "krok po kroku",
    ];
    MARKERS.iter().any(|marker| lower.contains(marker))
}

fn estimate_tokens(messages: &[ChatTurn]) -> u32 {
    let chars: u32 = messages
        .iter()
        .map(|turn| turn.content.chars().count() as u32)
        .sum();
    (chars / 4).max(1)
}

#[cfg(test)]
mod tests {
    use super::{plan, profile_turn, ModelClass, SlotUsage, TaskKind, model_class};
    use crate::llm::provider::ChatTurn;

    fn turn(role: &str, content: &str) -> ChatTurn {
        ChatTurn {
            role: role.into(),
            content: content.into(),
        }
    }

    fn fresh() -> SlotUsage {
        SlotUsage {
            rpm_used: 0,
            rpd_used: 0,
            tpm_used: 0,
            cooldown_left_sec: 0,
        }
    }

    #[test]
    fn short_chat_is_light_and_code_is_heavy() {
        let light = profile_turn(&[turn("user", "Cześć")]);
        assert_eq!(light.kind, TaskKind::Light);

        let heavy = profile_turn(&[turn("user", "Napisz funkcję sortującą liczby")]);
        assert_eq!(heavy.kind, TaskKind::Heavy);
    }

    #[test]
    fn long_thread_is_heavy_even_without_keywords() {
        let history = vec![turn("user", &"a".repeat(9_000))];
        let profile = profile_turn(&history);
        assert_eq!(profile.kind, TaskKind::Heavy);
        assert!(profile.estimated_tokens >= 2_000);
    }

    #[test]
    fn model_class_does_not_treat_120b_as_20b() {
        assert_eq!(model_class("openai/gpt-oss-120b"), ModelClass::Strong);
        assert_eq!(model_class("openai/gpt-oss-20b"), ModelClass::Fast);
        assert_eq!(model_class("gemini-2.5-flash"), ModelClass::Mid);
        assert_eq!(model_class("gemini-2.5-pro"), ModelClass::Strong);
    }

    #[test]
    fn light_turn_prefers_fast_model_with_quota() {
        let profile = profile_turn(&[turn("user", "Dzień dobry")]);
        let providers = ["groq", "groq", "gemini"];
        let models = ["openai/gpt-oss-20b", "openai/gpt-oss-120b", "gemini-2.5-pro"];
        let usage = [fresh(), fresh(), fresh()];
        let plan = plan(&profile, &providers, &models, &usage);
        assert_eq!(plan.picks[0].index, 0);
    }

    #[test]
    fn heavy_turn_prefers_strong_model() {
        let profile = profile_turn(&[turn("user", "Wyjaśnij, dlaczego ten deadlock powstaje")]);
        let providers = ["groq", "groq"];
        let models = ["openai/gpt-oss-20b", "openai/gpt-oss-120b"];
        let plan = plan(&profile, &providers, &models, &[fresh(), fresh()]);
        assert_eq!(plan.picks[0].index, 1);
    }

    #[test]
    fn long_prompt_skips_groq_tpm_and_uses_gemini() {
        let profile = profile_turn(&[turn("user", &"z".repeat(40_000))]);
        let providers = ["groq", "gemini"];
        let models = ["openai/gpt-oss-120b", "gemini-2.5-pro"];
        let plan = plan(&profile, &providers, &models, &[fresh(), fresh()]);
        assert_eq!(plan.picks.len(), 1);
        assert_eq!(plan.picks[0].index, 1);
    }

    #[test]
    fn exhausted_rpm_is_skipped_until_the_window_moves() {
        let profile = profile_turn(&[turn("user", "Cześć")]);
        let mut used = fresh();
        used.rpm_used = 30;
        let plan = plan(
            &profile,
            &["groq"],
            &["openai/gpt-oss-20b"],
            &[used],
        );
        assert!(plan.picks.is_empty());
        assert_eq!(plan.retry_after_sec, 60);
    }

    #[test]
    fn cooldown_blocks_even_when_local_quota_remains() {
        let profile = profile_turn(&[turn("user", "Cześć")]);
        let mut used = fresh();
        used.cooldown_left_sec = 12;
        let plan = plan(
            &profile,
            &["gemini"],
            &["gemini-2.5-flash"],
            &[used],
        );
        assert!(plan.picks.is_empty());
        assert_eq!(plan.retry_after_sec, 12);
    }
}
