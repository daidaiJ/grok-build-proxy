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
//!   `final` semantics) — held-back text flushes as text, nothing is dropped;
//! - a marker the answer *quotes* inside markdown code (`` `<think>` `` in prose, or a
//!   marker line inside a fenced block) is prose, not a boundary — see [`CodeScan`].

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
pub(crate) struct ThinkTagSplitter {
    in_think: bool,
    buffer: String,
    /// Markdown code context of the visible text emitted so far.
    code: CodeScan,
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
                        self.emit_all_but_partial_marker(&mut out.reasoning, false);
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
                    // Quoted markers become prose: feed the scan over the text before the
                    // candidate first, since whether the candidate is a real marker depends on
                    // the code context at its own offset, then emit it like any other text.
                    self.code.feed(&self.buffer[..pos]);
                    if self.code.in_code() {
                        let quoted_end = pos + len;
                        self.code.feed(&self.buffer[pos..quoted_end]);
                        out.text.push_str(&self.buffer[..quoted_end]);
                        self.buffer.drain(..quoted_end);
                        continue;
                    }
                    out.text.push_str(&self.buffer[..pos]);
                    self.buffer.drain(..pos + len);
                    self.in_think = true;
                    self.think_seen = true;
                }
                None => {
                    self.emit_all_but_partial_marker(&mut out.text, true);
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
                self.code.feed(&self.buffer);
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
    ///
    /// `track_code` is set for the text channel: only visible text advances the
    /// markdown scan, since reasoning never reaches the reader.
    fn emit_all_but_partial_marker(&mut self, out: &mut String, track_code: bool) {
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
        if track_code {
            self.code.feed(&self.buffer[..cut]);
        }
        out.push_str(&self.buffer[..cut]);
        self.buffer.drain(..cut);
    }
}

/// Markdown code context of the visible-text channel, used to keep quoted markers as prose.
///
/// Markers the model *quotes* while documenting the protocol — `` `<think>` `` in an answer,
/// or a marker line inside a fenced block — are not reasoning boundaries. Honoring one reroutes
/// the rest of the answer onto the reasoning channel, where the UI folds it into a collapsed
/// "Thought for Xs" block and the visible answer stops mid-sentence (observed 6 times across
/// 3 sessions, every one a marker wrapped in backticks). qwen-code's XML tool-call fallback
/// skips fenced blocks for the same reason: quoted protocol text is documentation.
///
/// Only the two forms a model writes matter: inline code spans (backtick runs) and
/// backtick/tilde fences. Spans are tracked per line, so a stray unmatched backtick blinds only
/// the rest of its own line; fences last until their closing fence line. Deliberately not
/// covered: other quoting forms (`"<think>"`, `**<think>**`), which stay honored as markers.
#[derive(Debug, Default)]
struct CodeScan {
    /// Open fence: (fence byte, opening run length).
    fence: Option<(u8, usize)>,
    /// Length of the backtick run that opened the inline span we are inside.
    span: Option<usize>,
    /// Leading spaces of the current line, capped past a fence's 3-space allowance.
    lead_spaces: usize,
    /// Content other than leading spaces has appeared on the current line.
    line_content: bool,
    /// Fence-byte run still open at the end of the text fed so far. Runs arrive split
    /// across deltas (a fence opening as `"``"` + `"`"`), so a run is only interpreted
    /// once something other than its own byte follows it.
    run: Option<Run>,
}

/// A backtick/tilde run: `at_line_start` records whether it was the line's first content
/// behind at most 3 spaces, which is what makes 3+ of them a fence rather than prose.
#[derive(Debug, Clone, Copy)]
struct Run {
    byte: u8,
    len: usize,
    at_line_start: bool,
}

impl CodeScan {
    /// Whether the scan stands inside code at the current offset, counting the run still
    /// in progress: an unflushed run is code either way — it opens a span, or is a fence
    /// it may still grow into.
    fn in_code(&self) -> bool {
        match self.run {
            Some(run) => self.code_after_run(run),
            None => self.fence.is_some() || self.span.is_some(),
        }
    }

    /// Advance over text emitted on the visible-text channel.
    fn feed(&mut self, text: &str) {
        for byte in text.bytes() {
            if matches!(byte, b'`' | b'~') {
                if matches!(self.run, Some(run) if run.byte == byte) {
                    if let Some(run) = &mut self.run {
                        run.len += 1;
                    }
                } else {
                    self.flush_run();
                    self.run = Some(Run {
                        byte,
                        len: 1,
                        at_line_start: !self.line_content && self.lead_spaces <= 3,
                    });
                }
                continue;
            }
            self.flush_run();
            match byte {
                b'\n' => {
                    self.lead_spaces = 0;
                    self.line_content = false;
                    // A span cannot survive its line: an unmatched backtick only blinds
                    // the text up to the break, never the rest of the answer.
                    self.span = None;
                }
                b' ' | b'\t' if !self.line_content => {
                    self.lead_spaces = (self.lead_spaces + 1).min(4);
                }
                _ => self.line_content = true,
            }
        }
    }

    /// Apply the run in progress: open/close a fence, or toggle an inline code span.
    fn flush_run(&mut self) {
        let Some(run) = self.run.take() else {
            return;
        };
        self.line_content = true;
        if run.at_line_start && run.len >= 3 {
            match self.fence {
                Some((open, open_len)) if open == run.byte && run.len >= open_len => {
                    self.fence = None;
                    return;
                }
                None => {
                    self.fence = Some((run.byte, run.len));
                    // The fence owns the context now; a span opened on its opening line
                    // (the run itself) must not outlive it.
                    self.span = None;
                    return;
                }
                // A different fence char than the open one is code content, not a fence.
                Some(_) => {}
            }
        }
        if self.fence.is_some() || run.byte != b'`' {
            // Every run inside a fence is code, and `~` is a fence char only.
            return;
        }
        match self.span {
            None => self.span = Some(run.len),
            Some(open) if open == run.len => self.span = None,
            Some(_) => {}
        }
    }

    /// The `in_code` answer for a run that has not been flushed, computed exactly as
    /// [`Self::flush_run`] would leave it.
    fn code_after_run(&self, run: Run) -> bool {
        if run.at_line_start && run.len >= 3 {
            return match self.fence {
                Some((open, open_len)) => !(open == run.byte && run.len >= open_len),
                None => true,
            };
        }
        if self.fence.is_some() {
            return true;
        }
        if run.byte != b'`' {
            return self.span.is_some();
        }
        match self.span {
            None => true,
            Some(open) => open != run.len,
        }
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

    #[test]
    fn quoted_marker_in_inline_code_stays_text() {
        // Live incident (session 01a0d161, MiMo answer): the answer documented a chat-template
        // patch as `` `<think>` ``, the splitter honored the quoted marker, and the rest of the
        // answer — more than a thousand characters, headings and all — surfaced as reasoning,
        // which the TUI rendered as a collapsed "Thought for Xs" block. The visible answer ended
        // on the opening backtick.
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(
            &mut sp,
            &[
                "1. **模板补丁**：强制每轮以 `",
                "<think>",
                "` 开头，修复流式空响应。",
            ],
        );
        assert_eq!(
            s.text,
            "1. **模板补丁**：强制每轮以 `<think>` 开头，修复流式空响应。"
        );
        assert_eq!(s.reasoning, "");
    }

    #[test]
    fn quoted_marker_split_across_chunks_stays_text() {
        // The marker itself arrives split, inside the span opened by the previous delta.
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["以 `<thi", "nk>` 开头"]);
        assert_eq!(s.text, "以 `<think>` 开头");
        assert_eq!(s.reasoning, "");
    }

    #[test]
    fn quoted_thinking_variant_and_other_span_shapes_stay_text() {
        // Second live shape: two spans in one sentence (`reasoning_content`、`<think>`).
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(
            &mut sp,
            &["delta(`reasoning_content`、`<thinking>`)只标记 chunk_has_content。"],
        );
        assert_eq!(
            s.text,
            "delta(`reasoning_content`、`<thinking>`)只标记 chunk_has_content。"
        );
        assert_eq!(s.reasoning, "");
    }

    #[test]
    fn marker_in_fenced_block_stays_text() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(
            &mut sp,
            &["模板补丁：\n```text\n", "<think>\n", "```\n修复完成"],
        );
        assert_eq!(s.text, "模板补丁：\n```text\n<think>\n```\n修复完成");
        assert_eq!(s.reasoning, "");
    }

    #[test]
    fn real_marker_after_code_still_opens_a_think_block() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["用 `x` 之后 <think>hmm</think>结束"]);
        assert_eq!(s.text, "用 `x` 之后 结束");
        assert_eq!(s.reasoning, "hmm");
    }

    #[test]
    fn inline_span_does_not_outlive_its_line() {
        // An unmatched backtick blinds its own line only, so a genuine marker on the
        // next line is still honored.
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["写 `a\n<think>hmm</think>done"]);
        assert_eq!(s.text, "写 `a\ndone");
        assert_eq!(s.reasoning, "hmm");
    }

    #[test]
    fn fence_split_across_deltas_still_opens() {
        // Token-level streaming splits the fence's own backtick run, so the run is only
        // interpreted once a non-fence byte follows it.
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(&mut sp, &["先看下：\n``", "`\n", "<think>\n", "```\n结束"]);
        assert_eq!(s.text, "先看下：\n```\n<think>\n```\n结束");
        assert_eq!(s.reasoning, "");
    }

    #[test]
    fn fence_survives_shorter_inner_run_and_closes_on_matching_fence() {
        let mut sp = ThinkTagSplitter::new();
        let s = feed_all(
            &mut sp,
            &[
                "````\n",
                "<think> inner\n",
                "```\n",
                "<think>still code\n",
                "````\n",
                "<think>x</think>tail",
            ],
        );
        assert_eq!(
            s.text,
            "````\n<think> inner\n```\n<think>still code\n````\ntail"
        );
        assert_eq!(s.reasoning, "x");
    }
}
