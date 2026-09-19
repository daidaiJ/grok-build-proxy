//! Experimental streaming scrub for `<think>` / `<thinking>` tag leakage.
//!
//! Some chat templates leak the model's raw thinking into the `content` stream
//! wrapped in `<think>`/`<thinking>` tags. Left alone, the tags render as
//! visible text in the TUI and persist into history. This is a port of
//! qwen-code's `taggedThinkingParser.ts` (production-hardened; its test suite
//! is ported alongside), with the same semantics:
//!
//! - Tags are matched case-insensitively, with a binary mode toggle and
//!   intentional cross-matching (`<think>…</thinking>` closes).
//! - The toggle applies anywhere in the stream: a `<think>` seen mid-text
//!   switches to the thinking channel too. Trade-off, accepted upstream and
//!   here: a response that *discusses* the tag literally gets swallowed into
//!   the reasoning channel from the first mention on. That is why the whole
//!   scrubber sits behind the experimental `thinking_tag_scrub` flag.
//! - A tail that could still complete into a tag is held back (bounded by the
//!   longest tag) so a tag split across chunk boundaries is classified
//!   correctly; [`Self::finish`] flushes it per current mode (qwen's `final`).
//! - Close tags seen while not in thinking mode are literal text (qwen parity).

/// Longest tag across open/close variants (`</thinking>`).
const MAX_TAG_LEN: usize = 11;

const OPEN_TAGS: [&str; 2] = ["<think>", "<thinking>"];
const CLOSE_TAGS: [&str; 2] = ["</think>", "</thinking>"];

/// Splits a text-channel delta stream into `(visible, thinking)` pairs.
#[derive(Debug)]
pub struct ThinkingTagScrubber {
    inside_thinking: bool,
    /// Tail not yet classified: it may still complete into a tag.
    pending: String,
}

impl Default for ThinkingTagScrubber {
    fn default() -> Self {
        Self::new()
    }
}

impl ThinkingTagScrubber {
    pub fn new() -> Self {
        Self {
            inside_thinking: false,
            pending: String::new(),
        }
    }

    /// Feed one text-channel delta; returns `(visible_text, thinking_text)`.
    pub fn feed(&mut self, delta: &str) -> (String, String) {
        let mut buf = std::mem::take(&mut self.pending);
        buf.push_str(delta);
        let mut visible = String::new();
        let mut thinking = String::new();

        loop {
            let (emit, tags) = if self.inside_thinking {
                (&mut thinking, &CLOSE_TAGS)
            } else {
                (&mut visible, &OPEN_TAGS)
            };
            match find_longest_tag(&buf, tags) {
                Some((idx, len)) => {
                    emit.push_str(&buf[..idx]);
                    buf.drain(..idx + len);
                    self.inside_thinking = !self.inside_thinking;
                }
                None => {
                    let hold = partial_tag_suffix_len(&buf, tags);
                    let emit_to = buf.len() - hold;
                    emit.push_str(&buf[..emit_to]);
                    buf.drain(..emit_to);
                    break;
                }
            }
        }

        self.pending = buf;
        (visible, thinking)
    }

    /// Flush the held-back tail at stream end: an incomplete open tag in text
    /// mode stays visible text, an unclosed thinking block (and its partial
    /// close tag) stays on the thinking channel. The scrubber is spent after.
    pub fn finish(&mut self) -> (String, String) {
        let buf = std::mem::take(&mut self.pending);
        let out = if self.inside_thinking {
            (String::new(), buf)
        } else {
            (buf, String::new())
        };
        self.inside_thinking = false;
        out
    }
}

/// Length of the longest tag variant matching at the start of `text`, if any.
fn match_longest_tag_at(text: &str, tags: &[&str]) -> Option<usize> {
    tags.iter()
        .filter(|tag| {
            // `is_char_boundary` guards multi-byte text that would panic on the slice.
            text.len() >= tag.len()
                && text.is_char_boundary(tag.len())
                && text[..tag.len()].eq_ignore_ascii_case(tag)
        })
        .map(|tag| tag.len())
        .max()
}

/// First occurrence of any tag in `text`, longest variant winning at each
/// position (so `</thinking>` is not split into `</think>` + `ing>`).
fn find_longest_tag(text: &str, tags: &[&str]) -> Option<(usize, usize)> {
    for (idx, _) in text.char_indices().chain([(text.len(), '\0')]) {
        if let Some(len) = match_longest_tag_at(&text[idx..], tags) {
            return Some((idx, len));
        }
    }
    None
}

/// Whether `text` is a strict prefix of some tag (case-insensitive). Only
/// meaningful for text shorter than the longest tag. `text` itself is never
/// sliced here (only compared whole), and the tags are ASCII, so the
/// `tag[..text.len()]` slice cannot split a character.
fn is_prefix_of_any_tag(text: &str, tags: &[&str]) -> bool {
    if text.len() >= MAX_TAG_LEN {
        return false;
    }
    tags.iter()
        .any(|tag| tag.len() > text.len() && tag[..text.len()].eq_ignore_ascii_case(text))
}

/// Bytes at the tail of `text` that could still complete into a tag.
fn partial_tag_suffix_len(text: &str, tags: &[&str]) -> usize {
    let mut start = text.len().saturating_sub(MAX_TAG_LEN - 1);
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    for i in start..text.len() {
        if !text.is_char_boundary(i) {
            continue;
        }
        if is_prefix_of_any_tag(&text[i..], tags) {
            return text.len() - i;
        }
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ported from qwen-code `taggedThinkingParser.test.ts` (Apache-2.0):
    /// chunked variants of its one-shot cases plus its streaming-specific
    /// partial-tag, final-flush, and truncated-stream cases.
    struct Capture {
        visible: String,
        thinking: String,
    }

    fn feed_chunks(chunks: &[&str]) -> Capture {
        let mut s = ThinkingTagScrubber::new();
        let mut capture = Capture {
            visible: String::new(),
            thinking: String::new(),
        };
        for chunk in chunks {
            let (v, t) = s.feed(chunk);
            capture.visible.push_str(&v);
            capture.thinking.push_str(&t);
        }
        let (v, t) = s.finish();
        capture.visible.push_str(&v);
        capture.thinking.push_str(&t);
        capture
    }

    fn feed_one(text: &str) -> Capture {
        feed_chunks(&[text])
    }

    // ── Basic parsing (qwen: basic) ──────────────────────────

    #[test]
    fn plain_text_unchanged() {
        let c = feed_one("hello world");
        assert_eq!(c.visible, "hello world");
        assert_eq!(c.thinking, "");
    }

    #[test]
    fn think_wrapped_content_is_thought() {
        let c = feed_one("<think>reasoning</think>answer");
        assert_eq!(c.thinking, "reasoning");
        assert_eq!(c.visible, "answer");
    }

    #[test]
    fn thinking_wrapped_content_is_thought() {
        let c = feed_one("<thinking>r</thinking>a");
        assert_eq!(c.thinking, "r");
        assert_eq!(c.visible, "a");
    }

    #[test]
    fn mixed_think_and_thinking_usage() {
        let c = feed_one("<think>a</think>b<thinking>c</thinking>d");
        assert_eq!(c.thinking, "ac");
        assert_eq!(c.visible, "bd");
    }

    // ── Case insensitivity (qwen: uppercase / mixed-case) ────

    #[test]
    fn uppercase_and_mixed_case_tags() {
        for input in ["<THINK>a</THINK>b", "<THINKING>a</THINKING>b", "<Think>a</Think>b"] {
            let c = feed_one(input);
            assert_eq!(c.thinking, "a", "{input}");
            assert_eq!(c.visible, "b", "{input}");
        }
    }

    // ── Empty tag content (qwen: empty tags) ─────────────────

    #[test]
    fn empty_tags_produce_nothing() {
        let c = feed_one("before<think></think>after");
        assert_eq!(c.visible, "beforeafter");
        assert_eq!(c.thinking, "");
        let c = feed_one("a<thinking></thinking>b");
        assert_eq!(c.visible, "ab");
    }

    // ── Close tags in text mode are literal (qwen: no open tag) ──

    #[test]
    fn close_tag_without_open_tag_is_literal_text() {
        let c = feed_one("some </think> text");
        assert_eq!(c.visible, "some </think> text");
        let c = feed_one("x </thinking> y");
        assert_eq!(c.visible, "x </thinking> y");
    }

    // ── Partial-tag buffering across chunks (qwen: streaming core) ──

    #[test]
    fn partial_tag_prefix_buffered_across_chunks() {
        let mut s = ThinkingTagScrubber::new();
        let (v, t) = s.feed("pre <thi");
        assert_eq!((v.as_str(), t.as_str()), ("pre ", ""), "hold the partial tag");
        let (v, t) = s.feed("nk>hidden</think>visible");
        assert_eq!((v.as_str(), t.as_str()), ("visible", "hidden"));
    }

    #[test]
    fn chunk_that_is_only_a_partial_tag_prefix() {
        let mut s = ThinkingTagScrubber::new();
        let (v, t) = s.feed("<thi");
        assert_eq!((v.as_str(), t.as_str()), ("", ""));
        let (v, t) = s.feed("nking>thought</thinking>out");
        assert_eq!((v.as_str(), t.as_str()), ("out", "thought"));
    }

    #[test]
    fn partial_close_tag_prefix_in_thinking_mode() {
        let mut s = ThinkingTagScrubber::new();
        let (v, t) = s.feed("<think>content </th");
        assert_eq!(v, "");
        assert_eq!(t, "content ", "partial close tag held back");
        let (v, t) = s.feed("ink> visible");
        assert_eq!(v, " visible");
        assert_eq!(t, "");
    }

    #[test]
    fn tag_split_across_three_chunks() {
        let mut s = ThinkingTagScrubber::new();
        let (v, t) = s.feed("a <th");
        assert_eq!(v, "a ");
        let (v, t) = s.feed("in");
        assert_eq!((v.as_str(), t.as_str()), ("", ""));
        let (v, t) = s.feed("k>hidden</think>b");
        assert_eq!(v, "b");
        assert_eq!(t, "hidden");
    }

    #[test]
    fn close_tag_split_across_chunks() {
        let mut s = ThinkingTagScrubber::new();
        let (v, t) = s.feed("<think>thought</");
        assert_eq!(v, "");
        assert_eq!(t, "thought");
        let (v, t) = s.feed("think>visible");
        assert_eq!(v, "visible");
        assert_eq!(t, "");
    }

    // ── final flush (qwen: final flag) ───────────────────────

    #[test]
    fn unclosed_thinking_flushed_as_thinking_on_finish() {
        let c = feed_one("answer <think>reasoning");
        assert_eq!(c.visible, "answer ");
        assert_eq!(c.thinking, "reasoning");
    }

    #[test]
    fn incomplete_open_tag_preserved_as_text_on_finish() {
        let c = feed_one("text <thi");
        assert_eq!(c.visible, "text <thi");
        assert_eq!(c.thinking, "");
    }

    #[test]
    fn unclosed_partial_close_tag_flushed_as_thinking_on_finish() {
        let c = feed_one("<think>stuff</thi");
        assert_eq!(c.visible, "");
        assert_eq!(c.thinking, "stuff</thi");
    }

    // ── Multiple alternating blocks (qwen: multi-block) ──────

    #[test]
    fn alternating_blocks_across_feed_calls() {
        let mut s = ThinkingTagScrubber::new();
        let (v, t) = s.feed("<think>a</think>");
        assert_eq!((v.as_str(), t.as_str()), ("", "a"));
        let (v, t) = s.feed("b<thinking>c</thinking>d");
        assert_eq!(v, "bd");
        assert_eq!(t, "c");
    }

    // ── Cross-matching (qwen: binary mode toggle) ────────────

    #[test]
    fn cross_matching_both_directions() {
        let c = feed_one("<think>reasoning</thinking>visible");
        assert_eq!(c.thinking, "reasoning");
        assert_eq!(c.visible, "visible");
        let c = feed_one("<thinking>reasoning</think>visible");
        assert_eq!(c.thinking, "reasoning");
        assert_eq!(c.visible, "visible");
    }

    // ── Truncated streams (qwen: stream end flush) ───────────

    #[test]
    fn truncated_after_open_tag_flushes_as_thinking() {
        let c = feed_one("<think>partial response");
        assert_eq!(c.thinking, "partial response");
        assert_eq!(c.visible, "");
    }

    #[test]
    fn truncated_after_second_open_tag_flushes_as_thinking() {
        let c = feed_one("<think>done</think>visible <think>unclosed");
        assert_eq!(c.thinking, "doneunclosed");
        assert_eq!(c.visible, "visible ");
    }

    // ── Whitespace / unicode robustness ──────────────────────

    #[test]
    fn leading_whitespace_before_tag_is_preserved_as_text() {
        let c = feed_one("  \n<think>t</think>out");
        assert_eq!(c.thinking, "t");
        assert_eq!(c.visible, "  \nout");
    }

    #[test]
    fn unicode_survives_chunk_boundaries() {
        let c = feed_chunks(&["<think>思考🚀中", "</think>", "答案🚀"]);
        assert_eq!(c.thinking, "思考🚀中");
        assert_eq!(c.visible, "答案🚀");
    }
}
