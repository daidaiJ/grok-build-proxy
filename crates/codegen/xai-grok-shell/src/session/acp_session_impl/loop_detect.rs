//! Client-side loop detectors that cover the cases `IdenticalToolCallRun` cannot.
//!
//! Consecutive identical tool steps already get a system-reminder nudge and a hard
//! stop in `turn.rs` (action stationarity). This module adds:
//! - **ABAB alternating tool steps** (A,B,A,B,…), which evade the consecutive-equal
//!   counter. Same reminder-then-stop contract.
//! - **Content / thinking streaks** across sampling steps in one turn. xAI's
//!   `doom_loop` SSE is not available on BYOK; without this, a model that chants
//!   the same paragraph or replays the same reasoning block runs until max tokens.
//!
//! Detection is turn-scoped. Cross-turn persistence (MiMo loop-streak recovery)
//! is left out: it rewrites history and would have to join the prefix-invariant
//! tests.

use std::collections::VecDeque;
use std::hash::{Hash, Hasher};

/// Complete AB cycles before the reminder. 3 cycles = 6 steps (A B A B A B).
pub(super) const NUDGE_AFTER_ALTERNATING_CYCLES: u32 = 3;
/// Complete AB cycles before the silent hard stop.
pub(super) const HARD_STOP_ALTERNATING_CYCLES: u32 = 4;
/// Consecutive identical assistant texts before the reminder.
pub(super) const NUDGE_AFTER_CONTENT_STREAK: u32 = 3;
/// Consecutive identical assistant texts before the silent hard stop.
pub(super) const HARD_STOP_CONTENT_STREAK: u32 = 5;
/// Ignore (and reset) streaks shorter than this. Short "ok"/"done" replies are
/// not a chant, and encrypted-only reasoning items yield an empty string.
pub(super) const MIN_STREAK_CHARS: usize = 50;

const _: () = assert!(NUDGE_AFTER_ALTERNATING_CYCLES < HARD_STOP_ALTERNATING_CYCLES);
const _: () = assert!(NUDGE_AFTER_CONTENT_STREAK < HARD_STOP_CONTENT_STREAK);

pub(super) const ALTERNATING_NUDGE: &str = "You have been alternating between the same two \
     tool calls without making progress. Stop this A/B pattern. Take a different action, \
     or tell the user what is blocking you. This turn will be halted automatically if the \
     alternation continues.";

pub(super) const CONTENT_NUDGE: &str = "Your last few replies repeated the same text. Stop \
     repeating that block. If you are stuck, ask the user for guidance instead of restating \
     the same answer. This turn will be halted automatically if the repetition continues.";

pub(super) const THINKING_NUDGE: &str = "Your reasoning has been repeating the same block. \
     Stop looping on that thought. Take a new action or summarize the blocker for the user. \
     This turn will be halted automatically if the repetition continues.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LoopKind {
    Alternating,
    Content,
    Thinking,
}

impl LoopKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Alternating => "alternating_tool_calls",
            Self::Content => "content_streak",
            Self::Thinking => "thinking_streak",
        }
    }

    pub(super) fn reminder(self) -> &'static str {
        match self {
            Self::Alternating => ALTERNATING_NUDGE,
            Self::Content => CONTENT_NUDGE,
            Self::Thinking => THINKING_NUDGE,
        }
    }
}

/// Sliding-window ABAB detector over per-step tool signatures.
#[derive(Default)]
pub(super) struct AlternatingToolCallRun {
    recent: VecDeque<u64>,
    nudged: bool,
    stop_noted: bool,
}

impl AlternatingToolCallRun {
    pub(super) fn observe(&mut self, signature: &str) {
        let hash = hash_text(signature);
        self.recent.push_back(hash);
        let cap = (HARD_STOP_ALTERNATING_CYCLES as usize).saturating_mul(2);
        while self.recent.len() > cap {
            self.recent.pop_front();
        }
    }

    /// Longest complete AB cycle count on a suffix of the window, or 0 if none.
    /// The suffix is required because a leading unrelated step must not hide ABABAB.
    pub(super) fn cycle_count(&self) -> u32 {
        let min = (NUDGE_AFTER_ALTERNATING_CYCLES as usize).saturating_mul(2);
        let max = (HARD_STOP_ALTERNATING_CYCLES as usize).saturating_mul(2);
        let n = self.recent.len();
        if n < min {
            return 0;
        }
        for cycles in (NUDGE_AFTER_ALTERNATING_CYCLES..=HARD_STOP_ALTERNATING_CYCLES).rev() {
            let need = (cycles as usize).saturating_mul(2);
            if n < need || need > max {
                continue;
            }
            let start = n - need;
            if is_abab(self.recent.iter().skip(start).copied()) {
                return cycles;
            }
        }
        0
    }

    pub(super) fn take_nudge(&mut self) -> bool {
        let fire = self.cycle_count() >= NUDGE_AFTER_ALTERNATING_CYCLES && !self.nudged;
        self.nudged |= fire;
        fire
    }

    pub(super) fn should_hard_stop(&self) -> bool {
        self.cycle_count() >= HARD_STOP_ALTERNATING_CYCLES
    }

    /// First time the hard-stop threshold is held this run. Record-only mode
    /// uses this so `/stats` gets one stop row instead of one per later step.
    pub(super) fn take_stop(&mut self) -> bool {
        let fire = self.should_hard_stop() && !self.stop_noted;
        self.stop_noted |= fire;
        fire
    }
}

/// Consecutive identical text (assistant visible content, or reasoning) in one turn.
#[derive(Default)]
pub(super) struct TextStreak {
    last_hash: Option<u64>,
    run_len: u32,
    nudged: bool,
    stop_noted: bool,
}

impl TextStreak {
    pub(super) fn observe(&mut self, text: &str) -> u32 {
        let trimmed = text.trim();
        if trimmed.chars().count() < MIN_STREAK_CHARS {
            self.reset();
            return 0;
        }
        let hash = hash_text(trimmed);
        if self.last_hash == Some(hash) {
            self.run_len += 1;
        } else {
            self.run_len = 1;
            self.last_hash = Some(hash);
            self.nudged = false;
            self.stop_noted = false;
        }
        self.run_len
    }

    pub(super) fn run_len(&self) -> u32 {
        self.run_len
    }

    pub(super) fn take_nudge(&mut self) -> bool {
        let fire = self.run_len >= NUDGE_AFTER_CONTENT_STREAK && !self.nudged;
        self.nudged |= fire;
        fire
    }

    pub(super) fn should_hard_stop(&self) -> bool {
        self.run_len >= HARD_STOP_CONTENT_STREAK
    }

    pub(super) fn take_stop(&mut self) -> bool {
        let fire = self.should_hard_stop() && !self.stop_noted;
        self.stop_noted |= fire;
        fire
    }

    fn reset(&mut self) {
        self.last_hash = None;
        self.run_len = 0;
        self.nudged = false;
        self.stop_noted = false;
    }
}

/// Append a ledger row. Returns whether the turn should abort (config/env/slash).
pub(super) fn record_crossing(
    session_id: &str,
    model_id: &str,
    kind: &str,
    phase: xai_grok_tools::loop_detect_ledger::LoopDetectPhase,
    run_len: u32,
) -> bool {
    let abort = crate::util::config::resolve_loop_detect_abort();
    let aborted = abort
        && matches!(
            phase,
            xai_grok_tools::loop_detect_ledger::LoopDetectPhase::Stop
        );
    xai_grok_tools::loop_detect_ledger::record(
        &xai_grok_config::grok_home(),
        session_id,
        model_id,
        kind,
        phase,
        run_len,
        aborted,
    );
    abort
}

fn hash_text(text: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

fn is_abab(hashes: impl IntoIterator<Item = u64>) -> bool {
    let v: Vec<u64> = hashes.into_iter().collect();
    if v.len() < 2 || v.len() % 2 != 0 {
        return false;
    }
    let a = v[0];
    let b = v[1];
    if a == b {
        return false;
    }
    v.iter()
        .enumerate()
        .all(|(i, h)| *h == if i % 2 == 0 { a } else { b })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alternating_needs_three_cycles_to_nudge_and_four_to_stop() {
        let mut run = AlternatingToolCallRun::default();
        for i in 0..5 {
            run.observe(if i % 2 == 0 { "A" } else { "B" });
            assert!(!run.take_nudge(), "must not nudge before 3 cycles; i={i}");
            assert!(!run.should_hard_stop());
        }
        run.observe("B");
        assert_eq!(run.cycle_count(), 3);
        assert!(run.take_nudge());
        assert!(!run.take_nudge());
        assert!(!run.should_hard_stop());
        run.observe("A");
        run.observe("B");
        assert_eq!(run.cycle_count(), 4);
        assert!(run.should_hard_stop());
    }

    #[test]
    fn alternating_ignores_aaa_and_abc() {
        let mut aaa = AlternatingToolCallRun::default();
        for _ in 0..8 {
            aaa.observe("A");
        }
        assert_eq!(aaa.cycle_count(), 0);
        assert!(!aaa.take_nudge());

        let mut abc = AlternatingToolCallRun::default();
        for key in ["A", "B", "C", "A", "B", "C", "A", "B"] {
            abc.observe(key);
        }
        assert_eq!(abc.cycle_count(), 0);
        assert!(!abc.take_nudge());
    }

    #[test]
    fn alternating_still_fires_after_a_leading_unrelated_step() {
        let mut run = AlternatingToolCallRun::default();
        run.observe("X");
        for key in ["A", "B", "A", "B", "A", "B"] {
            run.observe(key);
        }
        assert_eq!(run.cycle_count(), 3);
        assert!(run.take_nudge());
    }

    #[test]
    fn alternating_breaks_when_the_pair_changes() {
        let mut run = AlternatingToolCallRun::default();
        for key in ["A", "B", "A", "B", "A", "B"] {
            run.observe(key);
        }
        assert!(run.take_nudge());
        run.observe("C");
        assert_eq!(run.cycle_count(), 0);
        assert!(!run.should_hard_stop());
    }

    #[test]
    fn content_streak_ignores_short_text_and_nudges_then_stops() {
        let mut streak = TextStreak::default();
        assert_eq!(streak.observe("ok"), 0);
        let block = "x".repeat(MIN_STREAK_CHARS);
        assert_eq!(streak.observe(&block), 1);
        assert_eq!(streak.observe(&block), 2);
        assert!(!streak.take_nudge());
        assert_eq!(streak.observe(&block), 3);
        assert!(streak.take_nudge());
        assert!(!streak.take_nudge());
        assert_eq!(streak.observe(&block), 4);
        assert!(!streak.should_hard_stop());
        assert_eq!(streak.observe(&block), 5);
        assert!(streak.should_hard_stop());
    }

    #[test]
    fn content_streak_resets_on_a_different_block() {
        let mut streak = TextStreak::default();
        let a = "a".repeat(MIN_STREAK_CHARS);
        let b = "b".repeat(MIN_STREAK_CHARS);
        streak.observe(&a);
        streak.observe(&a);
        streak.observe(&b);
        assert_eq!(streak.run_len(), 1);
        assert!(!streak.take_nudge());
    }

    #[test]
    fn content_streak_resets_on_short_interruption() {
        let mut streak = TextStreak::default();
        let a = "a".repeat(MIN_STREAK_CHARS);
        streak.observe(&a);
        streak.observe(&a);
        streak.observe("ok");
        assert_eq!(streak.run_len(), 0);
        streak.observe(&a);
        assert_eq!(streak.run_len(), 1);
    }

    #[test]
    fn take_stop_fires_once_while_the_threshold_holds() {
        let mut streak = TextStreak::default();
        let block = "x".repeat(MIN_STREAK_CHARS);
        for _ in 0..HARD_STOP_CONTENT_STREAK {
            streak.observe(&block);
        }
        assert!(streak.take_stop());
        assert!(!streak.take_stop());
        streak.observe(&"y".repeat(MIN_STREAK_CHARS));
        streak.observe(&block);
        for _ in 1..HARD_STOP_CONTENT_STREAK {
            streak.observe(&block);
        }
        assert!(streak.take_stop());
    }
}
