//! Captures token-level timing from streaming inference responses: TTFB, TTLB, and inter-token latency (ITL) statistics.

use std::time::Instant;

use serde::{Deserialize, Serialize};

/// Returns (p50, p99, max, mean, sum) from a slice of sorted values.
/// Panics if `sorted` is empty.
pub fn compute_percentiles(sorted: &[u64]) -> (u64, u64, u64, u64, u64) {
    let len = sorted.len();
    assert!(len > 0, "Cannot compute percentiles from empty slice");

    let p50 = sorted[len / 2];
    let p99_idx = ((len as f64 * 0.99).ceil() as usize)
        .saturating_sub(1)
        .min(len - 1);
    let p99 = sorted[p99_idx];
    let max = sorted[len - 1];
    let sum: u64 = sorted.iter().sum();
    let mean = sum / len as u64;

    (p50, p99, max, mean, sum)
}

/// Per-response inference latency metrics computed from chunk timestamps.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InferenceLatencyStats {
    /// Time to the first streamed output token (text, reasoning, or tool-call arguments) in ms.
    /// LOCAL: reasoning / tool-argument deltas count, so a tool-only or thinking-first
    /// turn still lands a sample instead of leaving the status-line perf segment blank.
    pub time_to_first_token_ms: Option<u64>,
    /// Measured at stream exhaustion, not at the last content chunk, so it includes trailing metadata chunks.
    pub time_to_last_byte_ms: u64,
    /// Number of content chunks received
    pub chunk_count: u32,
    /// Inter-token latency intervals (raw data for session aggregation)
    pub itl_intervals_ms: Vec<u64>,
    pub itl_p50_ms: Option<u64>,
    pub itl_p99_ms: Option<u64>,
    pub itl_max_ms: Option<u64>,
    pub itl_mean_ms: Option<u64>,
    /// Total request attempts (`1` means no retries); set by the retry loop on success.
    pub attempts: u32,
}

impl InferenceLatencyStats {
    pub fn record_on_span(&self, span: &tracing::Span) {
        if let Some(ttfb) = self.time_to_first_token_ms {
            span.record("ttfb_ms", ttfb);
        }
        span.record("ttlb_ms", self.time_to_last_byte_ms);
        span.record("chunk_count", self.chunk_count);
        if let Some(p50) = self.itl_p50_ms {
            span.record("itl_p50_ms", p50);
        }
        if let Some(p99) = self.itl_p99_ms {
            span.record("itl_p99_ms", p99);
        }
    }

    /// `request_sent_at` - `Instant::now()` captured right before the HTTP request is issued.
    /// LOCAL(perf): TTFT anchors here, not at stream start. Gateways that withhold response
    /// headers until the first token is ready flush headers and the first SSE event
    /// back-to-back, so a stream-start anchor collapses TTFT to ~0ms and the shell's
    /// zero-sample filter then hides it; the pre-headers queue/prefill window is part of
    /// the user-perceived first-token wait.
    /// `stream_start` - `Instant::now()` captured before initiating the stream (first poll, ≈ headers arrival); TTLB keeps this reference so the delivery window stays comparable with prior samples.
    /// `chunk_timestamps` - `Instant` recorded on each output-token-bearing chunk (text, reasoning, or tool-call arguments).
    /// `stream_end` - `Instant::now()` captured after the stream is fully exhausted (after trailing metadata/`[DONE]` chunks). Used for TTLB.
    pub fn from_timestamps(
        request_sent_at: Instant,
        stream_start: Instant,
        chunk_timestamps: &[Instant],
        stream_end: Instant,
    ) -> Self {
        let ttlb = stream_end.duration_since(stream_start).as_millis() as u64;

        if chunk_timestamps.is_empty() {
            return Self {
                time_to_last_byte_ms: ttlb,
                ..Default::default()
            };
        }

        let ttfb = chunk_timestamps[0].duration_since(request_sent_at);

        let intervals: Vec<u64> = chunk_timestamps
            .windows(2)
            .map(|w| w[1].duration_since(w[0]).as_millis() as u64)
            .collect();

        let (itl_p50, itl_p99, itl_max, itl_mean) = if intervals.is_empty() {
            (None, None, None, None)
        } else {
            let mut sorted = intervals.clone();
            sorted.sort_unstable();
            let (p50, p99, max, mean, _sum) = compute_percentiles(&sorted);
            (Some(p50), Some(p99), Some(max), Some(mean))
        };

        Self {
            time_to_first_token_ms: Some(ttfb.as_millis() as u64),
            time_to_last_byte_ms: ttlb,
            chunk_count: u32::try_from(chunk_timestamps.len()).unwrap_or(u32::MAX),
            itl_intervals_ms: intervals,
            itl_p50_ms: itl_p50,
            itl_p99_ms: itl_p99,
            itl_max_ms: itl_max,
            itl_mean_ms: itl_mean,
            attempts: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn offset(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn test_empty_timestamps() {
        let start = Instant::now();
        let end = start + Duration::from_millis(500);

        let stats = InferenceLatencyStats::from_timestamps(start, start, &[], end);

        assert_eq!(stats.time_to_first_token_ms, None);
        assert_eq!(stats.time_to_last_byte_ms, 500);
        assert_eq!(stats.chunk_count, 0);
        assert_eq!(stats.itl_p50_ms, None);
        assert_eq!(stats.itl_p99_ms, None);
        assert_eq!(stats.itl_max_ms, None);
        assert_eq!(stats.itl_mean_ms, None);
    }

    #[test]
    fn test_single_chunk() {
        let start = Instant::now();
        let chunks = vec![offset(start, 100)];
        let end = offset(start, 200);

        let stats = InferenceLatencyStats::from_timestamps(start, start, &chunks, end);

        assert_eq!(stats.time_to_first_token_ms, Some(100));
        assert_eq!(stats.time_to_last_byte_ms, 200);
        assert_eq!(stats.chunk_count, 1);
        // One chunk yields no intervals, so there are no ITL stats
        assert_eq!(stats.itl_p50_ms, None);
        assert_eq!(stats.itl_p99_ms, None);
        assert_eq!(stats.itl_max_ms, None);
        assert_eq!(stats.itl_mean_ms, None);
    }

    #[test]
    fn test_two_chunks() {
        let start = Instant::now();
        let chunks = vec![offset(start, 100), offset(start, 150)];
        let end = offset(start, 200);

        let stats = InferenceLatencyStats::from_timestamps(start, start, &chunks, end);

        assert_eq!(stats.time_to_first_token_ms, Some(100));
        assert_eq!(stats.time_to_last_byte_ms, 200);
        assert_eq!(stats.chunk_count, 2);
        // One 50ms interval between the two chunks
        assert_eq!(stats.itl_p50_ms, Some(50));
        assert_eq!(stats.itl_p99_ms, Some(50));
        assert_eq!(stats.itl_max_ms, Some(50));
        assert_eq!(stats.itl_mean_ms, Some(50));
    }

    #[test]
    fn test_many_chunks() {
        let start = Instant::now();
        // 11 chunks: intervals are [10, 20, 30, 40, 50, 60, 70, 80, 90, 100]
        let chunks: Vec<Instant> = (0..11)
            .scan(100u64, |acc, i| {
                let t = *acc;
                *acc += (i + 1) * 10;
                Some(offset(start, t))
            })
            .collect();
        let end = offset(start, 1000);

        let stats = InferenceLatencyStats::from_timestamps(start, start, &chunks, end);

        assert_eq!(stats.time_to_first_token_ms, Some(100));
        assert_eq!(stats.time_to_last_byte_ms, 1000);
        assert_eq!(stats.chunk_count, 11);

        // p50 takes intervals[len / 2] = intervals[5]
        assert_eq!(stats.itl_p50_ms, Some(60));
        // p99_idx is ceil(10 * 0.99) - 1 = 9, so p99 reads intervals[9]
        assert_eq!(stats.itl_p99_ms, Some(100));
        assert_eq!(stats.itl_max_ms, Some(100));
        // The ten intervals sum to 550
        assert_eq!(stats.itl_mean_ms, Some(55));
    }

    #[test]
    fn test_p99_does_not_overflow() {
        let start = Instant::now();
        // 101 chunks give 100 intervals (indices 0..99)
        let chunks: Vec<Instant> = (0..101).map(|i| offset(start, 100 + i * 10)).collect();
        let end = offset(start, 2000);

        let stats = InferenceLatencyStats::from_timestamps(start, start, &chunks, end);

        assert_eq!(stats.chunk_count, 101);
        // All 100 intervals are 10ms
        // p99_idx is ceil(100 * 0.99) - 1 = 99, which stays in bounds
        assert_eq!(stats.itl_p99_ms, Some(10));
        assert_eq!(stats.itl_max_ms, Some(10));
        assert_eq!(stats.itl_p50_ms, Some(10));
        assert_eq!(stats.itl_mean_ms, Some(10));
    }

    #[test]
    fn test_ttlb_uses_stream_end_not_last_chunk() {
        let start = Instant::now();
        let chunks = vec![offset(start, 100), offset(start, 200)];
        // stream_end is 500ms after start, well past the last chunk at 200ms
        let end = offset(start, 500);

        let stats = InferenceLatencyStats::from_timestamps(start, start, &chunks, end);

        assert_eq!(stats.time_to_last_byte_ms, 500);
        assert_eq!(stats.time_to_first_token_ms, Some(100));
    }

    /// LOCAL(perf): TTFT counts from the request being issued, TTLB from stream start.
    /// A gateway that sits 400ms on queue/prefill before flushing headers together with
    /// the first event must still report a first-token wait that includes that window.
    #[test]
    fn ttft_anchors_to_request_sent_at_not_stream_start() {
        let sent = Instant::now();
        // 400ms of queue/prefill before the headers arrive (stream start)
        let stream_start = offset(sent, 400);
        // first token 100ms after the headers
        let chunks = vec![offset(sent, 500)];
        let end = offset(sent, 1200);

        let stats = InferenceLatencyStats::from_timestamps(sent, stream_start, &chunks, end);

        assert_eq!(stats.time_to_first_token_ms, Some(500));
        assert_eq!(stats.time_to_last_byte_ms, 800);
    }
}
