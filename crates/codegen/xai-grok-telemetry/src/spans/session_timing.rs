//! Per-process lanes for the session-create / session-load waterfall.
//!
//! Timers are recorded by the layer that owns the work (agent builder, spawn, persistence) and read once by
//! `session/new` / `session/load` when the response is ready. Timers do not carry a session id, so a process that
//! creates two sessions concurrently can interleave lanes; the TUI creates one session per turn, and a mixed sample
//! is still a better signal than a missing breakdown. Keep the map bounded: this is a diagnostic surface, not a sink.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Bounded so a timer with a dynamic name can never grow this map without limit.
const MAX_LANES: usize = 256;

static LANES: LazyLock<Mutex<BTreeMap<String, u64>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Starts a fresh waterfall. Called at the entry of `session/new` and `session/load`.
pub fn begin() {
    LANES.lock().unwrap_or_else(|e| e.into_inner()).clear();
    ACTIVE.store(true, Ordering::Relaxed);
}

/// Records a duration lane; later writes replace earlier ones (last known phase timing wins).
pub fn record(key: &str, elapsed: Duration) {
    record_ms(key, duration_ms(elapsed));
}

/// Records a raw millisecond value, also used for counts (`skills.count`, `skills.disabled_count`).
pub fn record_ms(key: &str, value: u64) {
    if !ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    let mut lanes = LANES.lock().unwrap_or_else(|e| e.into_inner());
    if lanes.len() < MAX_LANES || lanes.contains_key(key) {
        lanes.insert(key.to_owned(), value);
    }
}

/// Reads a lane without consuming the waterfall.
pub fn get(key: &str) -> Option<u64> {
    LANES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(key)
        .copied()
}

/// Drains the waterfall so the next `begin()` cannot inherit a previous session's lanes.
pub fn take() -> BTreeMap<String, u64> {
    ACTIVE.store(false, Ordering::Relaxed);
    let mut lanes = LANES.lock().unwrap_or_else(|e| e.into_inner());
    std::mem::take(&mut *lanes)
}

/// Routes an `InstrumentationTimer` into the waterfall. `startup.*` and `session.*` names are normalized to the
/// shared key space, so the active/idle naming split (`startup.agent_build.x` vs `session.agent_build.x`) does not
/// produce two different lanes for the same work.
pub fn record_timer(name: &str, elapsed: Duration) {
    let key = name
        .strip_prefix("startup.")
        .or_else(|| name.strip_prefix("session."))
        .unwrap_or(name);
    if key == name {
        return;
    }
    record(key, elapsed);
}

/// A scope timer for call sites that only need one lane.
pub struct Timing {
    key: &'static str,
    started: Instant,
}

impl Timing {
    pub fn new(key: &'static str) -> Self {
        Self {
            key,
            started: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

impl Drop for Timing {
    fn drop(&mut self) {
        record(self.key, self.started.elapsed());
    }
}

pub fn timing(key: &'static str) -> Timing {
    Timing::new(key)
}

/// Floor to whole milliseconds, saturating instead of wrapping on absurd durations.
pub fn duration_ms(elapsed: Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}
