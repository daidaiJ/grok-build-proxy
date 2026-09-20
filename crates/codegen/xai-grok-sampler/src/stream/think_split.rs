//! LOCAL(deepseek-compat): streaming splitter for inline `<think>`/`<thinking>` reasoning markers.
//!
//! Some OpenAI-compatible endpoints route a reasoning model's thinking through the
//! `reasoning_content` field but still emit the closing marker as `content` at the
//! reasoning→answer boundary (opencode #34126 shape), and other gateways inline the
//! whole `<think>...</think>` block into `content` with no reasoning field at all.
//! Both shapes leak raw markers into the assistant text unless `content` deltas are
//! re-classified as they arrive.
//!
//! The splitter is a two-phase (text / think) state machine fed one delta at a time.
//! It borrows its cross-chunk algorithm from vercel/ai's `extractReasoningMiddleware`
//! and qwen-code's `TaggedThinkingParser`:
//!
//! - a buffer tail that is a proper prefix of any marker is held back until the next
//!   delta resolves it (handles `<thi` + `nk>` splits across chunks);
//! - a lone closing marker while the reasoning field was seen and no answer text has
//!   flowed yet is a boundary artifact and is dropped, never shown (the user-visible
//!   leak); the same marker after real text has been emitted is preserved verbatim,
//!   because at that point it is ordinary content;
//! - an unclosed think block at end of stream flushes as reasoning (qwen-code
//!   `final` semantics) — held-back text flushes as text, nothing is dropped.

/// Markers recognized on the content channel. DeepSeek/Qwen emit `<think>`/`</think>`;
/// MiniMax and some Qwen templates use the longer `<thinking>` variants.
const OPEN_TAGS: [&str; 2] = ["<think>", "<thinking>"];
const CLOSE_TAGS: [&str; 2] = ["</think>", "</thinking>"];
const ALL_TAGS: [&str; 4] = ["<think>", "<thinking>", "</think>", "</thinking>"];
/// Longest marker (`</thinking>`) bounds the cross-chunk holdback window.
const MAX_TAG_LEN: usize = 11;

/// One `feed`/`finish` result: what should surface on each channel.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Split {
    pub text: String,
    pub reasoning: String,
}

/// Cross-chunk classifier for content deltas carrying inline think markers.
#[derive(Debug, Default)]
pub(crate) struct ThinkTagSplitter {    in_think: bool,
    buffer: String,
    saw_reasoning_field: bool,
    /// True once non-whitespace answer text has been emitted. Whitespace-only
    /// emissions don't count: some gateways emit a bare `\n\n` content chunk
    /// before the boundary marker, and the boundary rule must still hold there.
    text_emitted: bool,
    think_seen: bool,
}

impl ThinkTagSplitter {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Record that a `reasoning_content`-style field delta arrived on this response.
    /// Arms the boundary rule: a lone closing marker on the content channel right
    /// after field reasoning is the field's end marker, not answer text.
    pub(crate) fn note_reasoning_field(&mut self) {
        self.saw_reasoning_field = true;
    }

    /// Classify one content delta into (text, reasoning) output.
    pub(crate) fn feed(&mut self, delta: &str) -> Split {
        self.buffer.push_str(delta);
        let mut out = Split::default();

        while !self.buffer.is_empty() {
            if self.in_think {
                match find_earliest(&self.buffer, &CLOSE_TAGS) {
                    Some((pos, len)) => {
                        out.reasoning.push_str(&self.buffer[..pos]);
                        self.buffer.drain(..pos + len);
                        self.in_think = false;
                    }
                    None => {
                        self.emit_all_but_partial_marker(&mut out.reasoning);
                        break;
                    }
                }
                continue;
            }

            // Boundary artifact: the reasoning field just ended and the first
            // content is a closing marker (alone, or leading a chunk that also
            // carries the first answer text). Drop the marker itself.
            if self.boundary_pending() {
                if CLOSE_TAGS.contains(&self.buffer.trim()) {
                    self.buffer.clear();
                    continue;
                }
                if let Some(len) = CLOSE_TAGS
                    .iter()
                    .find(|t| self.buffer.starts_with(**t))
                    .map(|t| t.len())
                {
                    self.buffer.drain(..len);
                    continue;
                }
            }

            match find_earliest(&self.buffer, &OPEN_TAGS) {
                Some((pos, len)) => {
                    out.text.push_str(&self.buffer[..pos]);
                    self.buffer.drain(..pos + len);
                    self.in_think = true;
                    self.think_seen = true;
                }
                None => {
                    self.emit_all_but_partial_marker(&mut out.text);
                    break;
                }
            }
        }

        if out.text.chars().any(|c| !c.is_whitespace()) {
            self.text_emitted = true;
        }
        out
    }

    /// Flush at end of stream. An unclosed think block becomes reasoning;
    /// a held-back partial marker becomes text. Never drops bytes.
    pub(crate) fn finish(&mut self) -> Split {
        let mut out = Split::default();
        if !self.buffer.is_empty() {
            if self.in_think {
                out.reasoning.push_str(&self.buffer);
            } else {
                out.text.push_str(&self.buffer);
            }
            self.buffer.clear();
        }
        out
    }

    /// The reasoning→answer boundary is still open: field reasoning was seen and
    /// no inline think block has started and no answer text has flowed yet.
    fn boundary_pending(&self) -> bool {
        self.saw_reasoning_field && !self.text_emitted && !self.think_seen
    }

    /// Emit everything that cannot be part of a marker, holding back only a tail
    /// that might complete one with the next delta.
    fn emit_all_but_partial_marker(&mut self, out: &mut String) {
        let buf_len = self.buffer.len();
        // The whole buffer could still become a marker (e.g. `<thi`): hold it all.
        if ALL_TAGS
            .iter()
            .any(|t| t.len() > buf_len && t.starts_with(self.buffer.as_str()))
        {
            return;
        }
        // Otherwise hold the longest suffix that is a proper prefix of any marker.
        let mut cut = buf_len;
        for n in (1..MAX_TAG_LEN).rev() {
            if n >= buf_len {
                continue;
            }
            let start = buf_len - n;
            // A marker always begins with ASCII `<`, so a start offset inside a
            // multi-byte codepoint cannot begin one — and slicing there panics.
            if !self.buffer.is_char_boundary(start) {
                continue;
            }
            if ALL_TAGS
                .iter()
                .any(|t| t.starts_with(&self.buffer[start..]))
            {
                cut = start;
                break;
            }
        }
        out.push_str(&self.buffer[..cut]);
        self.buffer.drain(..cut);
    }
}

/// Earliest occurrence of any tag in `buf`, preferring the longer tag on a tie
/// (`<think>` vs `<thinking>` can both match only as disjoint strings in
/// practice, but the tie-break keeps the intent explicit).
fn find_earliest(buf: &str, tags: &[&str]) -> Option<(usize, usize)> {
    let mut best: Option<(usize, usize)> = None;
    for tag in tags {
        if let Some(pos) = buf.find(tag) {
            let better = match best {
                None => true,
                Some((bpos, blen)) => pos < bpos || (pos == bpos && tag.len() > blen),
            };
            if better {
                best = Some((pos, tag.len()));
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed_all(sp: &mut ThinkTagSplitter, deltas: &[&str]) -> Split {
        let mut acc = Split::default();
        for d in deltas {
            let s = sp.feed(d);
            acc.text.push_str(&s.text);
            acc.reasoning.push_str(&s.reasoning);
        }
        let tail = sp.finish();
        acc.text.push_str(&tail.text);
        acc.reasoning.push_str(&tail.reasoning);
        acc
    }

    #[test]
    fn marker_free_content_passes_through_byte_identical() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["Hello, ", "world! < > <b>ok"]);
        assert_eq!(s.text, "Hello, world! < > <b>ok");
        assert_eq!(s.reasoning, "");
    }

    #[test]
    fn lone_closing_marker_at_boundary_is_dropped() {
        // opencode #34126 shape: reasoning via field, then a standalone `</think>`
        // content chunk, then the answer.
        let mut sp = ThinkTagSplitter::new();
        sp.note_reasoning_field();
        let s = feed_all(&mut sp, &["</think>", "\n\nAnswer."]);
        assert_eq!(s.text, "\n\nAnswer.");
        assert_eq!(s.reasoning, "");
    }

    #[test]
    fn closing_marker_with_trailing_newlines_is_dropped() {
        let mut sp = ThinkTagSplitter::new();
        sp.note_reasoning_field();
        let s = feed_all(&mut sp, &["</think>\n\n", "Answer."]);
        assert_eq!(s.text, "Answer.");
    }

    #[test]
    fn closing_marker_leading_first_content_chunk_is_dropped() {
        let mut sp = ThinkTagSplitter::new();
        sp.note_reasoning_field();
        let s = feed_all(&mut sp, &["</think>\n\nAnswer."]);
        assert_eq!(s.text, "\n\nAnswer.");
    }

    #[test]
    fn closing_marker_after_real_text_is_preserved() {
        // opencode no-regression: once answer text flows, the marker is ordinary text.
        let mut sp = ThinkTagSplitter::new();
        sp.note_reasoning_field();
        let s = feed_all(&mut sp, &["ok here's the answer </think> done"]);
        assert_eq!(s.text, "ok here's the answer </think> done");
    }

    #[test]
    fn closing_marker_without_field_reasoning_is_preserved() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["before ", "</think>", " after"]);
        assert_eq!(s.text, "before </think> after");
    }

    #[test]
    fn inline_think_block_routes_to_reasoning() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["<think>pondering</think>Final answer"]);
        assert_eq!(s.text, "Final answer");
        assert_eq!(s.reasoning, "pondering");
    }

    #[test]
    fn text_before_think_block_stays_text() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["Sure! <think>hmm</think> Here you go"]);
        assert_eq!(s.text, "Sure!  Here you go");
        assert_eq!(s.reasoning, "hmm");
    }

    #[test]
    fn open_marker_split_across_chunks_is_detected() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["abc<thi", "nk>hidden", " tail</thi", "nk>done"]);
        assert_eq!(s.text, "abcdone");
        assert_eq!(s.reasoning, "hidden tail");
    }

    #[test]
    fn thinking_variants_are_recognized() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["<thinking>a</thinking>b"]);
        assert_eq!(s.text, "b");
        assert_eq!(s.reasoning, "a");
    }

    #[test]
    fn multiple_think_blocks_accumulate_reasoning() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["<think>one</think>mid<think>two</think>end"]);
        assert_eq!(s.text, "midend");
        assert_eq!(s.reasoning, "onetwo");
    }

    #[test]
    fn empty_think_block_is_silently_removed() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["<think></think>answer"]);
        assert_eq!(s.text, "answer");
        assert_eq!(s.reasoning, "");
    }

    #[test]
    fn unclosed_think_block_flushes_as_reasoning() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["<think>never closed"]);
        assert_eq!(s.text, "");
        assert_eq!(s.reasoning, "never closed");
    }

    #[test]
    fn trailing_partial_marker_flushes_as_text() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["value < th", "an 11"]);
        assert_eq!(s.text, "value < than 11");
        assert_eq!(s.reasoning, "");
    }

    #[test]
    fn multibyte_delta_is_not_probed_at_a_non_char_boundary() {
        // CJK deltas: the suffix scan must skip byte offsets inside a codepoint
        // (`这篇` = 6 bytes, probing at byte 1 slices inside '这').
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["这篇"]);
        assert_eq!(s.text, "这篇");
        assert_eq!(s.reasoning, "");
    }

    #[test]
    fn multibyte_text_around_markers_is_byte_intact() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["回答<thi", "nk>想一下</think>", "结束"]);
        assert_eq!(s.text, "回答结束");
        assert_eq!(s.reasoning, "想一下");
    }

    #[test]
    fn whitespace_only_content_does_not_close_the_boundary() {
        // Gateway emits `\n\n` content, then the standalone `</think>`, then text.
        let mut sp = ThinkTagSplitter::new();
        sp.note_reasoning_field();
        let s = feed_all(&mut sp, &["\n\n", "</think>", "Answer"]);
        assert_eq!(s.text, "\n\nAnswer");
    }

    #[test]
    fn no_reasoning_field_means_no_boundary_rule() {
        // Inline `<think>` handled on its own; a later lone `</think>` after text
        // must not be eaten by the boundary rule.
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["<think>x</think>text</think>more"]);
        assert_eq!(s.text, "text</think>more");
        assert_eq!(s.reasoning, "x");
    }
}
