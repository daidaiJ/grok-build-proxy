//! LOCAL: 模型组（跨供应商降级链）运行时——进程级冷却登记与条目选择。
//!
//! 设计见 `docs-local/subagent-model-group-todo.md`（G1/G2）。v1 降级语义是
//! spawn 级：条目失败（provider-unavailable 类错误终止一次任务尝试）后进入
//! 进程级冷却，后续 spawn/子代理解析跳过冷却条目顺延到下一可用条目；冷却过期
//! 自动恢复首选。会话中途换条目（首字节前 failover）留作 v2。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::agent::config::{ModelGroupConfig, ModelGroupEntry};

/// 进程级模型组运行态：组名 → 条目下标 → 冷却截止时刻。
/// 全局单例；子代理 spawn 解析与失败标记共用。
#[derive(Default)]
pub struct ModelGroupRuntime {
    cooldown_until: Mutex<HashMap<String, HashMap<usize, Instant>>>,
}

impl ModelGroupRuntime {
    pub fn global() -> &'static ModelGroupRuntime {
        static RUNTIME: OnceLock<ModelGroupRuntime> = OnceLock::new();
        RUNTIME.get_or_init(ModelGroupRuntime::default)
    }

    /// 标记条目进入冷却。已在冷却的条目取更晚的截止时刻。
    pub fn mark_cooldown(&self, group: &str, entry_idx: usize, cooldown_minutes: u64) {
        let mut guard = self.cooldown_until.lock().expect("model group runtime lock");
        let state = guard.entry(group.to_string()).or_default();
        let until = Instant::now() + Duration::from_secs(cooldown_minutes.max(1) * 60);
        state
            .entry(entry_idx)
            .and_modify(|existing| *existing = (*existing).max(until))
            .or_insert(until);
    }

    /// 条目是否在冷却中。过期条目顺带清除（懒回收）。
    pub fn is_cooling(&self, group: &str, entry_idx: usize) -> bool {
        let mut guard = self.cooldown_until.lock().expect("model group runtime lock");
        let Some(state) = guard.get_mut(group) else {
            return false;
        };
        match state.get(&entry_idx) {
            Some(until) if *until > Instant::now() => true,
            Some(_) => {
                state.remove(&entry_idx);
                false
            }
            None => false,
        }
    }
}

/// 组解析的纯选择核：按序返回第一个不在冷却中的条目下标。
/// 全部条目都在冷却时返回 `None`（消费方 fall through 到既有优先级）。
pub fn pick_entry_idx(entries: &[ModelGroupEntry], group: &str) -> Option<usize> {
    entries.iter().enumerate().find_map(|(idx, _)| {
        (!ModelGroupRuntime::global().is_cooling(group, idx)).then_some(idx)
    })
}

/// 一次组解析的绑定记录：子代理用了哪个组的哪个条目；失败时据此标记冷却。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelGroupBinding {
    pub group: String,
    pub entry_idx: usize,
    pub cooldown_minutes: u64,
}

impl ModelGroupBinding {
    pub fn from_config(group: &str, entry_idx: usize, group_config: &ModelGroupConfig) -> Self {
        Self {
            group: group.to_string(),
            entry_idx,
            cooldown_minutes: group_config.cooldown_minutes(),
        }
    }
}

/// 任务失败错误串是否为「供应商不可用」类（保守签名，只有高置信才标冷却）。
/// 400/401/403 一律不匹配：配置错或内容错换供应商大概率同样失败，且会掩盖真问题。
pub fn error_is_provider_unavailable(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    const SIGNATURES: [&str; 12] = [
        "429",
        "rate limit",
        "rate-limit",
        "ratelimit",
        "too many requests",
        "quota",
        " 500",
        " 502",
        " 503",
        " 504",
        "connection",
        "timeout",
    ];
    SIGNATURES.iter().any(|sig| lower.contains(sig))
}

#[cfg(test)]
#[path = "model_group_tests.rs"]
mod tests;
