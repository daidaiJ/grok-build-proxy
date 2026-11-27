/// Default auto-compact threshold (% of context window) when no source sets it.
pub const DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT: u8 = 85;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompactionToolChoice {
    #[default]
    Auto,
    None,
}

impl std::str::FromStr for CompactionToolChoice {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "none" => Ok(Self::None),
            _ => Err(()),
        }
    }
}

pub(crate) const ENV_COMPACTION_TOOL_CHOICE: &str = "GROK_COMPACTION_TOOL_CHOICE";

// LOCAL: idle-wait auto-compaction knobs. The toggle reads
// `[compaction] idle_wait_compact`; the numeric bounds are env-only so a
// long-running session can be tuned without a config rewrite.
pub(crate) const ENV_IDLE_WAIT_COMPACT: &str = "GROK_IDLE_WAIT_COMPACT";
pub(crate) const ENV_IDLE_WAIT_COMPACT_AFTER_SECS: &str = "GROK_IDLE_WAIT_COMPACT_AFTER_SECS";
pub(crate) const ENV_IDLE_WAIT_EXPECTED_CALLS: &str = "GROK_IDLE_WAIT_EXPECTED_CALLS";
pub(crate) const ENV_IDLE_WAIT_MIN_PROMPT_TOKENS: &str = "GROK_IDLE_WAIT_MIN_PROMPT_TOKENS";
pub(crate) const ENV_IDLE_WAIT_MIN_NET_TOKENS: &str = "GROK_IDLE_WAIT_MIN_NET_TOKENS";
pub(crate) const ENV_IDLE_WAIT_COOLDOWN_SECS: &str = "GROK_IDLE_WAIT_COOLDOWN_SECS";
pub(crate) const ENV_IDLE_WAIT_MIN_GROWTH_TOKENS: &str = "GROK_IDLE_WAIT_MIN_GROWTH_TOKENS";

pub(crate) const DEFAULT_IDLE_WAIT_AFTER_SECS: u64 = 270;
pub(crate) const DEFAULT_IDLE_WAIT_EXPECTED_CALLS: u64 = 4;
pub(crate) const DEFAULT_IDLE_WAIT_MIN_PROMPT_TOKENS: u64 = 100_000;
pub(crate) const DEFAULT_IDLE_WAIT_MIN_NET_TOKENS: u64 = 50_000;
pub(crate) const DEFAULT_IDLE_WAIT_COOLDOWN_SECS: u64 = 1_800;
pub(crate) const DEFAULT_IDLE_WAIT_MIN_GROWTH_TOKENS: u64 = 16_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IdleWaitCompactSettings {
    pub enabled: bool,
    pub after: std::time::Duration,
    pub expected_future_calls: u64,
    pub min_prompt_tokens: u64,
    pub min_net_tokens: u64,
    pub cooldown: std::time::Duration,
    pub min_growth_tokens: u64,
}

impl IdleWaitCompactSettings {
    /// Wait path disabled; used by the existing threshold-idle wrapper and its
    /// tests so they keep their pre-wait behavior.
    pub(crate) fn disabled() -> Self {
        Self {
            enabled: false,
            after: std::time::Duration::from_secs(DEFAULT_IDLE_WAIT_AFTER_SECS),
            expected_future_calls: DEFAULT_IDLE_WAIT_EXPECTED_CALLS,
            min_prompt_tokens: DEFAULT_IDLE_WAIT_MIN_PROMPT_TOKENS,
            min_net_tokens: DEFAULT_IDLE_WAIT_MIN_NET_TOKENS,
            cooldown: std::time::Duration::from_secs(DEFAULT_IDLE_WAIT_COOLDOWN_SECS),
            min_growth_tokens: DEFAULT_IDLE_WAIT_MIN_GROWTH_TOKENS,
        }
    }
}

fn env_bool(key: &str) -> Option<bool> {
    let raw = std::env::var(key).ok()?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .unwrap_or(default)
}

fn idle_wait_compact_from_toml(value: Option<&toml::Value>) -> Option<bool> {
    value?
        .get("compaction")?
        .get("idle_wait_compact")?
        .as_bool()
}

/// Resolve the idle-wait compaction settings. Precedence for the toggle:
/// `GROK_IDLE_WAIT_COMPACT`, then `[compaction] idle_wait_compact`, default on.
/// The numeric bounds are env-only with conservative defaults.
pub(crate) fn resolve_idle_wait_compact_settings() -> IdleWaitCompactSettings {
    let config = crate::config::load_effective_config().ok();
    let enabled = env_bool(ENV_IDLE_WAIT_COMPACT)
        .or_else(|| idle_wait_compact_from_toml(config.as_ref()))
        .unwrap_or(true);
    IdleWaitCompactSettings {
        enabled,
        after: std::time::Duration::from_secs(
            env_u64(
                ENV_IDLE_WAIT_COMPACT_AFTER_SECS,
                DEFAULT_IDLE_WAIT_AFTER_SECS,
            )
            .clamp(30, 3_600),
        ),
        expected_future_calls: env_u64(
            ENV_IDLE_WAIT_EXPECTED_CALLS,
            DEFAULT_IDLE_WAIT_EXPECTED_CALLS,
        )
        .clamp(1, 64),
        min_prompt_tokens: env_u64(
            ENV_IDLE_WAIT_MIN_PROMPT_TOKENS,
            DEFAULT_IDLE_WAIT_MIN_PROMPT_TOKENS,
        )
        .max(1),
        min_net_tokens: env_u64(
            ENV_IDLE_WAIT_MIN_NET_TOKENS,
            DEFAULT_IDLE_WAIT_MIN_NET_TOKENS,
        )
        .max(1),
        cooldown: std::time::Duration::from_secs(
            env_u64(ENV_IDLE_WAIT_COOLDOWN_SECS, DEFAULT_IDLE_WAIT_COOLDOWN_SECS).clamp(60, 86_400),
        ),
        min_growth_tokens: env_u64(
            ENV_IDLE_WAIT_MIN_GROWTH_TOKENS,
            DEFAULT_IDLE_WAIT_MIN_GROWTH_TOKENS,
        ),
    }
}

pub(crate) const ENV_REPLAY_DIET: &str = "GROK_REPLAY_DIET";

fn replay_diet_from_toml(value: Option<&toml::Value>) -> Option<bool> {
    value?
        .get("compaction")?
        .get("replay_diet")?
        .as_bool()
}

/// `GROK_REPLAY_DIET` wins over `[compaction] replay_diet`; default on.
fn resolve_replay_diet_enabled(from_env: Option<bool>, from_toml: Option<bool>) -> bool {
    from_env.or(from_toml).unwrap_or(true)
}

/// Resolve the replay-diet policy once per session. Thresholds stay env-only;
/// the config-file tier controls the master switch.
pub(crate) fn resolve_replay_diet_policy() -> xai_chat_state::ReplayDietPolicy {
    let config = crate::config::load_effective_config().ok();
    let enabled = resolve_replay_diet_enabled(
        env_bool(ENV_REPLAY_DIET),
        replay_diet_from_toml(config.as_ref()),
    );
    let mut policy = xai_chat_state::ReplayDietPolicy::from_env();
    policy.enabled = enabled;
    policy
}

pub(crate) fn resolve_compaction_tool_choice_from(
    env: Option<&str>,
    config: Option<&str>,
    remote: Option<&str>,
) -> CompactionToolChoice {
    env.and_then(|s| s.parse().ok())
        .or_else(|| config.and_then(|s| s.parse().ok()))
        .or_else(|| remote.and_then(|s| s.parse().ok()))
        .unwrap_or_default()
}

pub(crate) const ENV_AUTO_COMPACT_THRESHOLD_PERCENT: &str = "GROK_AUTO_COMPACT_THRESHOLD_PERCENT";

/// Precedence (highest first): env `GROK_AUTO_COMPACT_THRESHOLD_PERCENT` user TOML `[model.<id>].auto_compact_threshold_percent` (`cfg.config_models`, the merge of user and managed `[model.<id>]` sections) user TOML `[session].auto_compact_threshold_percent` remote settings per-model `ModelInfo.auto_compact_threshold_percent` (kept out of `ConfigModelOverride::apply` so the user and remote per-model tiers stay distinct) remote settings global `RemoteSettings.auto_compact_threshold_percent` default `DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT`
pub(crate) fn resolve_auto_compact_threshold_percent(
    cfg: &crate::agent::config::Config,
    model_id: &str,
    model: Option<&crate::agent::config::ModelInfo>,
) -> u8 {
    resolve_auto_compact_threshold_percent_from_tiers(
        cfg.config_models
            .get(model_id)
            .and_then(|m| m.auto_compact_threshold_percent),
        cfg.session.auto_compact_threshold_percent,
        model.and_then(|m| m.auto_compact_threshold_percent),
        cfg.remote_settings
            .as_ref()
            .and_then(|r| r.auto_compact_threshold_percent),
    )
}

/// [`resolve_auto_compact_threshold_percent`] for callers without a `Config`, e.g. subagent spawn paths that pass the parent's tiers explicitly.
/// There the per-model tier uses the subagent's resolved model id, not the parent's.
pub(crate) fn resolve_auto_compact_threshold_percent_from_tiers(
    user_per_model: Option<u8>,
    user_global: Option<u8>,
    gb_per_model: Option<u8>,
    gb_global: Option<u8>,
) -> u8 {
    fn clamp_env(raw: i64) -> Option<u8> {
        if (0..=100).contains(&raw) {
            Some(raw as u8)
        } else {
            tracing::debug!(
                source = "env",
                value = raw,
                "auto_compact_threshold_percent out of range 0..=100; ignoring"
            );
            None
        }
    }
    let from_env = || -> Option<u8> {
        std::env::var(ENV_AUTO_COMPACT_THRESHOLD_PERCENT)
            .ok()
            .and_then(|s| s.parse::<i64>().ok())
            .and_then(clamp_env)
    };

    from_env()
        .or(user_per_model)
        .or(user_global)
        .or(gb_per_model)
        .or(gb_global)
        .unwrap_or(DEFAULT_AUTO_COMPACT_THRESHOLD_PERCENT)
}

/// Fleet p99 of successful compactions is ~181s (≈225s at 400K+ input).
/// So 300s clears the legit tail with margin while cutting a runaway from the ~600s deadline.
pub const DEFAULT_COMPACTION_WALL_CLOCK_BUDGET_SECS: u64 = 300;

/// Below this, a configured budget is almost certainly a misconfig (fleet success p99 ~181s); logged at `warn`, not clamped.
const COMPACTION_WALL_CLOCK_BUDGET_WARN_SECS: u64 = 120;

const ENV_COMPACTION_WALL_CLOCK_BUDGET_SECS: &str = "GROK_COMPACTION_WALL_CLOCK_SECS";

/// Precedence: env `GROK_COMPACTION_WALL_CLOCK_SECS`, then remote `RemoteSettings.compaction_wall_clock_budget_secs`, then the client default. `0` **disables** it.
/// Low values are warned, not clamped: any "safe" clamp (e.g. 30s) would itself cut legit compactions, trading one silent failure for another. Ops own the value.
pub(crate) fn resolve_compaction_wall_clock_budget_secs(gb_global: Option<u64>) -> u64 {
    let from_env = std::env::var(ENV_COMPACTION_WALL_CLOCK_BUDGET_SECS)
        .ok()
        .and_then(|s| s.trim().parse::<u64>().ok());
    let resolved = from_env
        .or(gb_global)
        .unwrap_or(DEFAULT_COMPACTION_WALL_CLOCK_BUDGET_SECS);
    if resolved > 0 && resolved < COMPACTION_WALL_CLOCK_BUDGET_WARN_SECS {
        tracing::warn!(
            budget_secs = resolved,
            "compaction wall-clock budget {resolved}s is below {COMPACTION_WALL_CLOCK_BUDGET_WARN_SECS}s \
             and may cut legitimate compactions (fleet success p99 ~181s); set 0 to disable"
        );
    }
    resolved
}

#[cfg(test)]
mod compaction_wall_clock_budget_tests {
    use super::resolve_compaction_wall_clock_budget_secs as resolve;

    // Assumes GROK_COMPACTION_WALL_CLOCK_SECS is unset in the test env.
    #[test]
    fn default_global_disable_and_no_clamp() {
        assert_eq!(resolve(None), 300); // client default
        assert_eq!(resolve(Some(450)), 450); // server global wins
        assert_eq!(resolve(Some(0)), 0); // 0 explicitly disables (no clamp)
        assert_eq!(resolve(Some(5)), 5); // low values pass through (warned, not clamped)
    }
}

#[cfg(test)]
mod compaction_tool_choice_tests {
    use super::{CompactionToolChoice, resolve_compaction_tool_choice_from as resolve};

    #[test]
    fn default_is_auto() {
        assert_eq!(resolve(None, None, None), CompactionToolChoice::Auto);
    }

    #[test]
    fn precedence_env_over_config_over_remote() {
        assert_eq!(
            resolve(Some("none"), Some("auto"), Some("auto")),
            CompactionToolChoice::None
        );
        assert_eq!(
            resolve(None, Some("none"), Some("auto")),
            CompactionToolChoice::None
        );
        assert_eq!(
            resolve(None, None, Some("none")),
            CompactionToolChoice::None
        );
    }

    #[test]
    fn garbage_falls_through() {
        assert_eq!(
            resolve(Some("garbage"), None, Some("none")),
            CompactionToolChoice::None
        );
        assert_eq!(
            resolve(Some("garbage"), Some("also-bad"), None),
            CompactionToolChoice::Auto
        );
    }

    #[test]
    fn from_str_case_insensitive() {
        assert_eq!("AUTO".parse(), Ok(CompactionToolChoice::Auto));
        assert_eq!(" None ".parse(), Ok(CompactionToolChoice::None));
        assert!("required".parse::<CompactionToolChoice>().is_err());
    }
}

#[cfg(test)]
mod replay_diet_toggle_tests {
    use super::{replay_diet_from_toml, resolve_replay_diet_enabled};

    #[test]
    fn reads_compaction_table_toggle() {
        let value: toml::Value =
            toml::from_str("[compaction]\nreplay_diet = false\n").expect("toml parses");
        assert_eq!(replay_diet_from_toml(Some(&value)), Some(false));
    }

    #[test]
    fn missing_toggle_is_none() {
        let value: toml::Value = toml::from_str("[compaction]\n").expect("toml parses");
        assert_eq!(replay_diet_from_toml(Some(&value)), None);
    }

    #[test]
    fn env_wins_and_default_is_on() {
        assert!(!resolve_replay_diet_enabled(Some(false), Some(true)));
        assert!(resolve_replay_diet_enabled(Some(true), Some(false)));
        assert!(!resolve_replay_diet_enabled(None, Some(false)));
        assert!(resolve_replay_diet_enabled(None, None));
    }
}

#[cfg(test)]
mod idle_wait_toggle_tests {
    use super::idle_wait_compact_from_toml;

    #[test]
    fn reads_compaction_table_toggle() {
        let value: toml::Value =
            toml::from_str("[compaction]\nidle_wait_compact = false\n").expect("toml parses");
        assert_eq!(idle_wait_compact_from_toml(Some(&value)), Some(false));
    }

    #[test]
    fn missing_toggle_is_none() {
        let value: toml::Value = toml::from_str("[compaction]\n").expect("toml parses");
        assert_eq!(idle_wait_compact_from_toml(Some(&value)), None);
    }
}
