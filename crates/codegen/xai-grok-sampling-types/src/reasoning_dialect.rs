//! OpenAI-compatible reasoning wire-key dialects.
//!
//! The Chat Completions ecosystem never standardized a wire field for
//! reasoning/thinking content. Three names circulate in the wild:
//!
//! - `reasoning_content` — DeepSeek's original convention; Moonshot Kimi, most
//!   OpenAI-compatible gateways, pre-rename vLLM.
//! - `reasoning_details` — OpenRouter (array-shaped on the wire; the inbound
//!   scan only accepts string values, so today it can only be pinned explicitly,
//!   never detected).
//! - `reasoning` — OpenAI's GPT-OSS guidance; current vLLM renamed to this
//!   (vllm-project/vllm#27752) and its request side accepts ONLY this name
//!   (vllm-project/vllm#38488).
//!
//! The strategy (ported from kimi-code's `kosong/src/providers/reasoning-key.ts`):
//! inbound the stream layer scans the known keys in [`KNOWN_REASONING_WIRE_KEYS`]
//! priority order and records which one actually carried reasoning; outbound the
//! sampler replays the learned dialect — "reply in the dialect the peer spoke" —
//! instead of hard-coding vendor maps. That stays robust against arbitrary
//! OpenAI-compatible gateways and vLLM version drift. Detection never clears: a
//! response without reasoning keeps the last learned dialect, and a peer that
//! switches dialects mid-session is adapted to on its next observation.

use std::collections::HashMap;
use std::sync::RwLock;

/// The reasoning wire keys known to the dialect scanner, in inbound scan
/// priority order. The first entry doubles as the default outbound dialect
/// before any observation.
pub const KNOWN_REASONING_WIRE_KEYS: [&str; 3] = [
    "reasoning_content",
    "reasoning_details",
    "reasoning",
];

/// The dialect assumed before any observation: the de facto `reasoning_content`,
/// so OpenAI-compatible reasoners (DeepSeek, Qwen, One API gateways) work out of
/// the box.
pub const DEFAULT_REASONING_DIALECT: ReasoningDialect = ReasoningDialect::ReasoningContent;

/// Which wire field a peer uses to carry reasoning content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningDialect {
    /// DeepSeek-style `reasoning_content` (the de facto default).
    ReasoningContent,
    /// OpenRouter `reasoning_details` (array-shaped; serialized as a plain
    /// string today — the same shape kimi-code replays — until an endpoint
    /// demands the structured form).
    ReasoningDetails,
    /// GPT-OSS / current-vLLM `reasoning`.
    Reasoning,
}

impl ReasoningDialect {
    /// The wire field name this dialect reads/writes.
    pub fn wire_key(self) -> &'static str {
        match self {
            Self::ReasoningContent => "reasoning_content",
            Self::ReasoningDetails => "reasoning_details",
            Self::Reasoning => "reasoning",
        }
    }

    /// Resolve a wire key to its dialect; `None` for unknown keys (forward
    /// compatible: a new dialect added to [`KNOWN_REASONING_WIRE_KEYS`] without
    /// a variant here is skipped by the scanner rather than misclassified).
    pub fn from_wire_key(key: &str) -> Option<Self> {
        match key {
            "reasoning_content" => Some(Self::ReasoningContent),
            "reasoning_details" => Some(Self::ReasoningDetails),
            "reasoning" => Some(Self::Reasoning),
            _ => None,
        }
    }
}

/// Per-endpoint memory of which reasoning dialect the peer speaks.
///
/// Keyed by model id; the memory instance is meant to live on the per-endpoint
/// sampling client, so the scope is (endpoint, model) — the dialect is a
/// property of the endpoint, not of one request. Cheap to share: reads and
/// writes are lock-guarded map ops on a single `Arc`.
#[derive(Debug, Default)]
pub struct ReasoningDialectMemory {
    inner: RwLock<HashMap<String, ReasoningDialect>>,
}

impl ReasoningDialectMemory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record the dialect observed on an inbound response. Last write wins, so
    /// a peer that switches dialects is adapted to immediately.
    pub fn observe(&self, model: &str, dialect: ReasoningDialect) {
        if let Ok(mut map) = self.inner.write()
            && map.get(model) != Some(&dialect)
        {
            map.insert(model.to_owned(), dialect);
        }
    }

    /// The dialect to serialize reasoning into for this model.
    pub fn learned(&self, model: &str) -> ReasoningDialect {
        self.inner
            .read()
            .ok()
            .and_then(|map| map.get(model).copied())
            .unwrap_or(DEFAULT_REASONING_DIALECT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_keys_round_trip() {
        for key in KNOWN_REASONING_WIRE_KEYS {
            let dialect = ReasoningDialect::from_wire_key(key).expect("known key resolves");
            assert_eq!(dialect.wire_key(), key);
        }
        assert_eq!(ReasoningDialect::from_wire_key("thoughts"), None);
    }

    #[test]
    fn memory_defaults_to_reasoning_content_and_learns_last_write() {
        let memory = ReasoningDialectMemory::new();
        assert_eq!(memory.learned("m"), ReasoningDialect::ReasoningContent);

        memory.observe("m", ReasoningDialect::Reasoning);
        assert_eq!(memory.learned("m"), ReasoningDialect::Reasoning);

        // A response without reasoning must not clear the learned dialect, and
        // a dialect switch is picked up on its next observation.
        memory.observe("m", ReasoningDialect::ReasoningContent);
        assert_eq!(memory.learned("m"), ReasoningDialect::ReasoningContent);

        // Per-model isolation.
        assert_eq!(memory.learned("other"), ReasoningDialect::ReasoningContent);
    }
}
