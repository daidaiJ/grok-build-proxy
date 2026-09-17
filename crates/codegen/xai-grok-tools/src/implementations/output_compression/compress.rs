//! Headroom-style compressors for new tool results only.

use serde_json::{Map, Value};

use super::detect::{ContentKind, detect, has_word};
use super::runtime::{
    SessionCompressionPolicy, ToolOutputCompressionRuntime, record_no_win, record_skip, record_win,
};
use super::store;
use crate::types::output::ToolOutput;
use crate::util::truncate::estimate_tokens;

/// Compress a newly produced tool-result. Pass the session-start snapshot so
/// later config reloads cannot rewrite this conversation's prefix.
pub(crate) fn maybe_compress_prompt(
    output: &ToolOutput,
    prompt_text: String,
    session: Option<&SessionCompressionPolicy>,
) -> String {
    let Some(rt) = session.map(|p| &p.0).filter(|rt| rt.enabled) else {
        return prompt_text;
    };
    if !in_scope(output, &rt.scope) {
        return prompt_text;
    }
    if matches!(output, ToolOutput::MCP(m) if m.is_error) {
        record_skip();
        return prompt_text;
    }
    compress_text(&prompt_text, &rt)
}

fn in_scope(output: &ToolOutput, scope: &[String]) -> bool {
    scope.iter().any(|name| match name.as_str() {
        "bash" => matches!(output, ToolOutput::Bash(_)),
        "mcp" => matches!(output, ToolOutput::MCP(_)),
        _ => false,
    })
}

fn compress_text(prompt_text: &str, rt: &ToolOutputCompressionRuntime) -> String {
    let original_tokens = estimate_tokens(prompt_text) as u64;
    if original_tokens < rt.min_input_tokens as u64 {
        record_skip();
        return prompt_text.to_string();
    }

    let (prefix, body) = split_protected_prefix(prompt_text);
    let kind = detect(body);
    if !rt.strategy_allowed(kind.strategy_name()) && kind != ContentKind::Other {
        record_skip();
        return prompt_text.to_string();
    }

    let compressed_body = match kind {
        ContentKind::JsonArray => compress_json_array(body, rt),
        ContentKind::JsonObject => compress_json_object(body, rt),
        ContentKind::Logs => compress_logs(body, rt),
        ContentKind::Search => compress_search(body, rt),
        ContentKind::Diff => compress_diff(body, rt),
        ContentKind::Other => compress_generic(body, rt),
    };

    let mut candidate = if prefix.is_empty() {
        compressed_body
    } else {
        format!("{prefix}\n{compressed_body}")
    };

    let mut with_marker = candidate.clone();
    if rt.ccr_enabled {
        let h = store::compute_key(prompt_text);
        with_marker.push('\n');
        with_marker.push_str(&store::marker_for(&h));
        with_marker.push('\n');
        with_marker.push_str("Original stored. Call expand_output with this hash to retrieve it.");
        if (estimate_tokens(&with_marker) as u64) < original_tokens {
            if store::put(prompt_text, rt).is_some() {
                candidate = with_marker;
            }
        }
    }

    let new_tokens = estimate_tokens(&candidate) as u64;
    if new_tokens >= original_tokens {
        record_no_win(original_tokens, new_tokens.saturating_sub(original_tokens));
        return prompt_text.to_string();
    }
    record_win(original_tokens, original_tokens.saturating_sub(new_tokens));
    candidate
}

/// Keep bash `exit: N` / `exit: killed (...)` as a lossless header.
fn split_protected_prefix(text: &str) -> (&str, &str) {
    let Some((first, rest)) = text.split_once('\n') else {
        return ("", text);
    };
    if first.starts_with("exit:") {
        (first, rest)
    } else {
        ("", text)
    }
}

fn compress_json_array(body: &str, rt: &ToolOutputCompressionRuntime) -> String {
    let Ok(Value::Array(items)) = serde_json::from_str::<Value>(body.trim()) else {
        return compress_generic(body, rt);
    };
    let total = items.len();
    if total <= 6 {
        return body.to_string();
    }
    let head = 3.min(total);
    let tail = 2.min(total.saturating_sub(head));
    let mut kept: Vec<Value> = items.iter().take(head).cloned().collect();
    if tail > 0 {
        kept.extend(items.iter().rev().take(tail).cloned().rev());
    }
    let fields = items
        .iter()
        .find_map(|v| v.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()))
        .unwrap_or_default();
    let preview = serde_json::to_string(&kept).unwrap_or_else(|_| "[]".into());
    format!(
        "JSON array: {total} items, showing {} (first {head} + last {tail}). Fields: {}\n{preview}",
        kept.len(),
        if fields.is_empty() {
            "(mixed)".into()
        } else {
            fields.join(", ")
        }
    )
}

fn compress_json_object(body: &str, rt: &ToolOutputCompressionRuntime) -> String {
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(body.trim()) else {
        return compress_generic(body, rt);
    };
    let mut out = Map::new();
    for (key, value) in &map {
        let protect = rt.protect.iter().any(|p| p == key);
        if protect {
            out.insert(key.clone(), value.clone());
            continue;
        }
        match value {
            Value::String(s) if estimate_tokens(s) >= rt.min_input_tokens / 4 => {
                out.insert(key.clone(), Value::String(compress_text_inner(s, rt)));
            }
            Value::Array(_) => {
                let raw = value.to_string();
                out.insert(key.clone(), Value::String(compress_json_array(&raw, rt)));
            }
            other => {
                out.insert(key.clone(), other.clone());
            }
        }
    }
    serde_json::to_string(&Value::Object(out)).unwrap_or_else(|_| body.to_string())
}

fn compress_text_inner(text: &str, rt: &ToolOutputCompressionRuntime) -> String {
    match detect(text) {
        ContentKind::Logs => compress_logs(text, rt),
        ContentKind::Search => compress_search(text, rt),
        ContentKind::Diff => compress_diff(text, rt),
        ContentKind::JsonArray => compress_json_array(text, rt),
        _ => compress_generic(text, rt),
    }
}

fn compress_logs(body: &str, rt: &ToolOutputCompressionRuntime) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() < 30 {
        return body.to_string();
    }
    let mut keep = vec![false; lines.len()];
    let mut warn_seen = 0usize;
    let mut in_stack = false;
    let mut stack_kept = 0usize;
    let mut stacks = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if i < 8 || i + rt.keep_tail_lines >= lines.len() {
            keep[i] = true;
        }
        let is_error = has_word(line, "ERROR")
            || has_word(line, "error")
            || has_word(line, "FAIL")
            || has_word(line, "FAILED")
            || has_word(line, "FATAL")
            || has_word(line, "CRITICAL")
            || line.contains("error[E");
        let is_warn = has_word(line, "WARN") || has_word(line, "WARNING");
        let is_summary = line.starts_with("===")
            || line.starts_with("---")
            || line.starts_with("TOTAL")
            || line.starts_with("Test ")
            || line.contains("test session");
        if is_error || is_summary {
            keep[i] = true;
            mark_context(&mut keep, i, 2);
        }
        if is_warn {
            if warn_seen < 5 {
                keep[i] = true;
            }
            warn_seen += 1;
        }
        let stack_start = line.contains("Traceback (most recent call last)")
            || line.trim_start().starts_with("at ")
            || (line.trim_start().starts_with("--> ") && line.contains(':'));
        if stack_start {
            in_stack = true;
            stacks += 1;
            stack_kept = 0;
        }
        if in_stack {
            if stacks <= 3 && stack_kept < 20 {
                keep[i] = true;
                stack_kept += 1;
            }
            if line.trim().is_empty() && stack_kept > 1 && !line.contains("Traceback") {
                // keep going for chained python traces
            }
            let rust_or_js_end = !line.trim_start().starts_with("at ")
                && !line.trim_start().starts_with("--> ")
                && !line.trim_start().starts_with("File ")
                && !line.is_empty()
                && stack_kept > 1
                && !line.contains("Traceback");
            if rust_or_js_end && !line.starts_with([' ', '\t']) {
                in_stack = false;
            }
            if stack_kept >= 20 {
                in_stack = false;
            }
        }
    }
    let selected: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, l)| *l)
        .collect();
    let omitted = lines.len().saturating_sub(selected.len());
    if omitted == 0 {
        return body.to_string();
    }
    let mut out = selected.join("\n");
    out.push_str(&format!("\n[{omitted} lines omitted]"));
    out
}

fn mark_context(keep: &mut [bool], idx: usize, ctx: usize) {
    let lo = idx.saturating_sub(ctx);
    let hi = (idx + ctx + 1).min(keep.len());
    for slot in keep.iter_mut().take(hi).skip(lo) {
        *slot = true;
    }
}

fn compress_search(body: &str, rt: &ToolOutputCompressionRuntime) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() < 20 {
        return body.to_string();
    }
    let mut by_file: Vec<(String, Vec<&str>)> = Vec::new();
    for line in &lines {
        let file = line.split(':').next().unwrap_or("").to_string();
        if let Some(entry) = by_file.iter_mut().find(|(f, _)| *f == file) {
            if entry.1.len() < 3 {
                entry.1.push(*line);
            } else {
                entry.1.push(""); // counted but not kept
            }
        } else {
            by_file.push((file, vec![*line]));
        }
    }
    let total = lines.len();
    let mut out = Vec::new();
    out.push(format!(
        "Search results: {total} lines across {} files (showing up to 3 hits/file)",
        by_file.len()
    ));
    for (_file, hits) in by_file.iter().take(40) {
        for hit in hits.iter().filter(|h| !h.is_empty()).take(3) {
            out.push((*hit).to_string());
        }
    }
    if by_file.len() > 40 {
        out.push(format!("[{} files omitted]", by_file.len() - 40));
    }
    let _ = rt;
    out.join("\n")
}

fn compress_diff(body: &str, rt: &ToolOutputCompressionRuntime) -> String {
    let lines: Vec<&str> = body.lines().collect();
    if lines.len() < 40 {
        return body.to_string();
    }
    let mut keep = vec![false; lines.len()];
    let mut change_kept = 0usize;
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("diff ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("@@")
            || line.starts_with("index ")
        {
            keep[i] = true;
            continue;
        }
        let is_change = (line.starts_with('+') && !line.starts_with("+++"))
            || (line.starts_with('-') && !line.starts_with("---"));
        if is_change && change_kept < 80 {
            keep[i] = true;
            change_kept += 1;
        }
        if i + rt.keep_tail_lines >= lines.len() {
            keep[i] = true;
        }
    }
    let selected: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, l)| *l)
        .collect();
    let omitted = lines.len().saturating_sub(selected.len());
    let mut out = selected.join("\n");
    if omitted > 0 {
        out.push_str(&format!("\n[{omitted} diff lines omitted]"));
    }
    out
}

fn compress_generic(body: &str, rt: &ToolOutputCompressionRuntime) -> String {
    let lines: Vec<&str> = body.lines().collect();
    let head = 30usize;
    let tail = rt.keep_tail_lines;
    if lines.len() <= head + tail {
        return body.to_string();
    }
    let omitted = lines.len() - head - tail;
    let mut out = String::new();
    for line in lines.iter().take(head) {
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(&format!("[{omitted} lines omitted]\n"));
    for line in lines
        .iter()
        .rev()
        .take(tail)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        out.push_str(line);
        out.push('\n');
    }
    out.pop();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::output_compression::runtime::{
        CompressionStrategiesSpec, reset_for_tests, snapshot_stats, test_lock,
    };
    use crate::types::output::BashOutput;

    fn enabled_rt(dir: &std::path::Path) -> ToolOutputCompressionRuntime {
        ToolOutputCompressionRuntime {
            enabled: true,
            scope: vec!["bash".into(), "mcp".into()],
            strategies: CompressionStrategiesSpec::Auto,
            min_input_tokens: 20,
            keep_tail_lines: 4,
            protect: vec!["exit_code".into(), "stderr".into()],
            ccr_enabled: true,
            ccr_ttl_secs: 3600,
            ccr_dir: dir.join("ccr"),
            stats_path: dir.join("stats.json"),
        }
    }

    fn bash_output(prompt: &str) -> ToolOutput {
        ToolOutput::Bash(BashOutput {
            output: prompt.as_bytes().to_vec(),
            output_for_prompt: prompt.to_string(),
            exit_code: 1,
            command: "cargo test".into(),
            truncated: false,
            signal: None,
            timed_out: false,
            description: None,
            current_dir: "/tmp".into(),
            output_file: String::new(),
            total_bytes: prompt.len(),
            output_delta: None,
            was_bare_echo: false,
        })
    }

    fn log_blob() -> String {
        let mut s = String::from("exit: 1\n");
        for i in 0..80 {
            s.push_str(&format!("INFO starting worker {i}\n"));
        }
        s.push_str("ERROR explode in worker 7\n");
        s.push_str("Traceback (most recent call last):\n  File \"a.py\", line 1\nValueError: x\n");
        for i in 0..20 {
            s.push_str(&format!("INFO trailing {i}\n"));
        }
        s
    }

    #[test]
    fn disabled_is_noop() {
        let _guard = test_lock();
        let dir = tempfile::tempdir().unwrap();
        let mut rt = enabled_rt(dir.path());
        rt.enabled = false;
        reset_for_tests(rt.clone());
        let blob = log_blob();
        let policy = SessionCompressionPolicy(rt);
        let out = maybe_compress_prompt(&bash_output(&blob), blob.clone(), Some(&policy));
        assert_eq!(out, blob);
    }

    #[test]
    fn no_session_snapshot_is_noop_even_if_process_runtime_is_on() {
        let _guard = test_lock();
        let dir = tempfile::tempdir().unwrap();
        reset_for_tests(enabled_rt(dir.path()));
        let blob = log_blob();
        let out = maybe_compress_prompt(&bash_output(&blob), blob.clone(), None);
        assert_eq!(
            out, blob,
            "without a session snapshot, history must stay untouched"
        );
    }

    #[test]
    fn below_threshold_is_noop() {
        let _guard = test_lock();
        let dir = tempfile::tempdir().unwrap();
        let rt = enabled_rt(dir.path());
        reset_for_tests(rt.clone());
        let policy = SessionCompressionPolicy(rt);
        let text = "exit: 0\nok".to_string();
        let out = maybe_compress_prompt(&bash_output(&text), text.clone(), Some(&policy));
        assert_eq!(out, text);
    }

    #[test]
    fn compresses_logs_and_keeps_exit_and_error() {
        let _guard = test_lock();
        let dir = tempfile::tempdir().unwrap();
        let rt = enabled_rt(dir.path());
        reset_for_tests(rt.clone());
        let policy = SessionCompressionPolicy(rt);
        let blob = log_blob();
        let out = maybe_compress_prompt(&bash_output(&blob), blob.clone(), Some(&policy));
        assert!(out.starts_with("exit: 1\n"), "{out}");
        assert!(out.contains("ERROR explode"), "{out}");
        assert!(out.contains("Traceback"), "{out}");
        assert!(out.contains("lines omitted"), "{out}");
        assert!(out.contains("<<ccr:"), "{out}");
        assert!(
            out.len() < blob.len(),
            "out={} orig={}",
            out.len(),
            blob.len()
        );
        let again = maybe_compress_prompt(&bash_output(&blob), blob.clone(), Some(&policy));
        assert_eq!(
            out, again,
            "same input must compress identically (prefix cache)"
        );
    }

    #[test]
    fn json_object_keeps_exit_code_and_stderr() {
        let _guard = test_lock();
        let dir = tempfile::tempdir().unwrap();
        reset_for_tests(enabled_rt(dir.path()));
        let mut stdout = String::new();
        for i in 0..200 {
            stdout.push_str(&format!("line {i} INFO filler filler filler filler\n"));
        }
        let body = serde_json::json!({
            "stdout": stdout,
            "stderr": "boom",
            "exit_code": 2,
        })
        .to_string();
        let compressed = compress_json_object(&body, &enabled_rt(dir.path()));
        let value: Value = serde_json::from_str(&compressed).unwrap();
        assert_eq!(value["exit_code"], 2);
        assert_eq!(value["stderr"], "boom");
        assert_ne!(value["stdout"], stdout);
    }

    #[test]
    fn json_array_keeps_head_and_tail() {
        let items: Vec<Value> = (0..20)
            .map(|i| serde_json::json!({"id": i, "name": format!("n{i}")}))
            .collect();
        let body = serde_json::to_string(&items).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let out = compress_json_array(&body, &enabled_rt(dir.path()));
        assert!(out.contains("20 items"));
        assert!(out.contains("\"id\":0"));
        assert!(out.contains("\"id\":19"));
    }

    // Ignored by default: prints a yield report across sample outputs and
    // configs, for the keep-or-trim evaluation. Run with:
    //   cargo test -p xai-grok-tools sample_yield -- --ignored --nocapture
    #[test]
    #[ignore]
    fn sample_yield_report() {
        let _guard = test_lock();
        let dir = tempfile::tempdir().unwrap();
        let samples: Vec<(&str, String)> = vec![
            ("build-log", sample_build_log()),
            ("json-array", sample_json_array()),
            ("search", sample_search()),
            ("diff", sample_diff()),
            ("generic-prose", sample_prose()),
            ("short-ok", "exit: 0\n".to_string() + &"ok\n".repeat(10)),
        ];
        let configs: Vec<(&str, ToolOutputCompressionRuntime)> = vec![
            ("auto+ccr tail4", enabled_rt(dir.path())),
            (
                "auto nocc tail4",
                ToolOutputCompressionRuntime {
                    ccr_enabled: false,
                    ..enabled_rt(dir.path())
                },
            ),
            (
                "logs+json only",
                ToolOutputCompressionRuntime {
                    strategies: CompressionStrategiesSpec::List(vec![
                        "logs".into(),
                        "json".into(),
                    ]),
                    ..enabled_rt(dir.path())
                },
            ),
            (
                "auto+ccr tail20",
                ToolOutputCompressionRuntime {
                    keep_tail_lines: 20,
                    ..enabled_rt(dir.path())
                },
            ),
            (
                "min500 (default)",
                ToolOutputCompressionRuntime {
                    min_input_tokens: 500,
                    ..enabled_rt(dir.path())
                },
            ),
        ];

        println!("\n{:<15} {:<18} {:>8} {:>8} {:>7}  note", "sample", "config", "orig", "new", "saved%");
        for (name, text) in &samples {
            let orig = estimate_tokens(text) as u64;
            for (cname, rt) in &configs {
                reset_for_tests(rt.clone());
                let out = compress_text(text, rt);
                let new = estimate_tokens(&out) as u64;
                let saved = if orig > 0 {
                    100.0 * (orig.saturating_sub(new)) as f64 / orig as f64
                } else {
                    0.0
                };
                let stats = snapshot_stats();
                let note = if stats.compressed_calls > 0 {
                    "win"
                } else if stats.no_win_calls > 0 {
                    "no-win"
                } else {
                    "skip"
                };
                println!(
                    "{:<15} {:<18} {:>8} {:>8} {:>6.1}%  {note}",
                    name, cname, orig, new, saved
                );
            }
            println!();
        }
    }

    fn sample_build_log() -> String {
        let mut s = String::from("exit: 1\n");
        for i in 0..120 {
            s.push_str(&format!("   Compiling crate{i} v0.1.{i}\n"));
            if i % 7 == 0 {
                s.push_str(&format!("warning: unused variable `x{i}`\n"));
            }
        }
        s.push_str("error[E0308]: mismatched types\n");
        s.push_str("  --> src/main.rs:42:9\n   |\n42 |     let x: u8 = \"s\";\n   |\n");
        s.push_str("error: could not compile `demo` due to 1 previous error\n");
        s
    }

    fn sample_json_array() -> String {
        let items: Vec<Value> = (0..80)
            .map(|i| {
                serde_json::json!({
                    "path": format!("src/module{i}/file.rs"),
                    "size": 1000 + i * 37,
                    "lines": 40 + i,
                    "mtime": format!("2026-09-1{}T0{}:00:00Z", i % 9, i % 9),
                })
            })
            .collect();
        serde_json::to_string_pretty(&items).unwrap()
    }

    fn sample_search() -> String {
        let mut s = String::new();
        for i in 0..60 {
            s.push_str(&format!(
                "crates/codegen/xai-grok-pager/src/app/mod.rs:{}:    handle_event(Event::Key(key_{i}));\n",
                10 + i
            ));
        }
        s
    }

    fn sample_diff() -> String {
        let mut s = String::new();
        for f in 0..5 {
            s.push_str(&format!("diff --git a/src/f{f}.rs b/src/f{f}.rs\n"));
            s.push_str(&format!("--- a/src/f{f}.rs\n+++ b/src/f{f}.rs\n"));
            for h in 0..6 {
                s.push_str(&format!("@@ -{},7 +{},8 @@\n", h * 10 + 1, h * 10 + 1));
                for l in 0..8 {
                    s.push_str(&format!(" ctx line {} in f{f} hunk{h} with some padding text\n", l));
                    if l == 3 {
                        s.push_str(&format!("+added line {l} in f{f} hunk{h}\n"));
                    }
                }
            }
        }
        s
    }

    fn sample_prose() -> String {
        "Here is a summary of the changes made in this session. \
         The refactor touched the session lifecycle, moved the sampler \
         glue into its own module, and updated the docs. Everything "
            .repeat(30)
    }

    // ===================================================================
    // Mechanism ablation (round-robin): candidate loss-reduction
    // mechanisms prototyped test-local, scored on tokens saved AND
    // structural retention (must-keep needles). Run with:
    //   cargo test -p xai-grok-tools mech_ablation -- --ignored --nocapture
    // ===================================================================

    /// M1: exact-dup JSON crusher — dedup items by canonical hash, keep
    /// unique ones in full, replace repeats with a count marker.
    fn m1_dedup_json(body: &str) -> String {
        let Ok(Value::Array(items)) = serde_json::from_str::<Value>(body.trim()) else {
            return body.to_string();
        };
        if items.len() <= 6 {
            return body.to_string();
        }
        let mut order: Vec<String> = Vec::new();
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for item in &items {
            let key = canonical_key(item);
            *counts.entry(key.clone()).or_insert(0) += 1;
            if counts[&key] == 1 {
                order.push(key);
            }
        }
        let mut out = format!(
            "JSON array: {} items, {} unique. Fields: {}\n",
            items.len(),
            order.len(),
            item_fields(&items)
        );
        for key in &order {
            let n = counts[key];
            let item = items.iter().find(|i| canonical_key(i) == *key).unwrap();
            if n > 1 {
                out.push_str(&format!("{item} // x{n}\n"));
            } else {
                out.push_str(&format!("{item}\n"));
            }
        }
        out
    }

    fn canonical_key(item: &Value) -> String {
        // normalize numeric ids so near-dup rows collapse only on exact shape
        serde_json::to_string(item).unwrap_or_default()
    }

    fn item_fields(items: &[Value]) -> String {
        items
            .iter()
            .find_map(|v| v.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>().join(", ")))
            .unwrap_or_else(|| "(mixed)".into())
    }

    /// M2: adaptive-k — unique-bigram saturation curve (simplified
    /// adaptive_sizer) decides how many array items to keep; anchors
    /// (first 2 / last 2) always kept, middle filled by dedup order.
    fn m2_adaptive_k(body: &str) -> String {
        let Ok(Value::Array(items)) = serde_json::from_str::<Value>(body.trim()) else {
            return body.to_string();
        };
        if items.len() <= 8 {
            return body.to_string();
        }
        let strs: Vec<String> = items.iter().map(|i| i.to_string()).collect();
        let refs: Vec<&str> = strs.iter().map(|s| s.as_str()).collect();
        let k = adaptive_keep(&refs);
        let mut kept: Vec<usize> = Vec::new();
        let anchor_head = 2.min(items.len());
        let anchor_tail = 2.min(items.len().saturating_sub(anchor_head));
        for i in 0..anchor_head {
            kept.push(i);
        }
        let mut seen: std::collections::HashSet<u64> = std::collections::HashSet::new();
        for (i, s) in strs.iter().enumerate() {
            if kept.len() >= k {
                break;
            }
            if kept.contains(&i) {
                continue;
            }
            let h = fnv64(s);
            if seen.insert(h) {
                kept.push(i);
            }
        }
        for i in (items.len() - anchor_tail)..items.len() {
            if !kept.contains(&i) {
                kept.push(i);
            }
        }
        kept.sort_unstable();
        kept.dedup();
        let mut out = format!(
            "JSON array: {} items, adaptive-kept {} (knee of unique-bigram curve). Fields: {}\n",
            items.len(),
            kept.len(),
            item_fields(&items)
        );
        for i in kept {
            out.push_str(&format!("  {}\n", strs[i]));
        }
        out
    }

    /// simplified adaptive_sizer: unique-bigram coverage curve + knee.
    fn adaptive_keep(items: &[&str]) -> usize {
        let n = items.len();
        if n <= 8 {
            return n;
        }
        let mut curve: Vec<usize> = Vec::with_capacity(n);
        let mut uniq: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
        for item in items {
            let words: Vec<&str> = item.split_whitespace().collect();
            for w in words.windows(2) {
                uniq.insert((w[0].to_string(), w[1].to_string()));
            }
            curve.push(uniq.len());
        }
        // Kneedle-style knee: max distance from the diagonal.
        let (y0, y1) = (curve[0] as f64, *curve.last().unwrap() as f64);
        let mut knee = n;
        let mut best_d = 0.0f64;
        for (i, y) in curve.iter().enumerate() {
            let t = i as f64 / (n - 1).max(1) as f64;
            let line_y = y0 + (y1 - y0) * t;
            let d = (line_y - *y as f64).abs();
            if d > best_d {
                best_d = d;
                knee = i + 1;
            }
        }
        if best_d / (y1 - y0).max(1.0) < 0.05 {
            return n; // no saturation: keep all
        }
        (knee + 2).clamp(5, n)
    }

    fn fnv64(s: &str) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in s.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    /// M3: tiered-ranking logs — score every line (headroom tiered
    /// importance), keep by rank under a token budget, not fixed quotas.
    fn m3_tiered_logs(body: &str) -> String {
        let lines: Vec<&str> = body.lines().collect();
        if lines.len() < 30 {
            return body.to_string();
        }
        let scored: Vec<(usize, f32)> = lines
            .iter()
            .enumerate()
            .map(|(i, l)| (i, line_priority(l, i, lines.len())))
            .collect();
        let budget = (lines.len() as f32 * 0.35).max(12.0) as usize;
        let mut ranked = scored.clone();
        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        let mut keep = vec![false; lines.len()];
        let mut kept = 0usize;
        for (i, p) in ranked {
            if kept >= budget {
                break;
            }
            if p <= 0.3 {
                break; // only filler remains
            }
            if !keep[i] {
                keep[i] = true;
                kept += 1;
            }
        }
        render_kept(&lines, &keep)
    }

    fn line_priority(line: &str, idx: usize, total: usize) -> f32 {
        if idx < 2 || idx + 2 >= total {
            return 0.85; // anchors
        }
        if has_word(line, "FATAL") || has_word(line, "CRITICAL") {
            return 0.99;
        }
        if has_word(line, "ERROR") || has_word(line, "error") || line.contains("error[E") {
            return 0.95;
        }
        if line.contains("Traceback")
            || line.trim_start().starts_with("at ")
            || line.trim_start().starts_with("--> ")
            || line.trim_start().starts_with("File ")
            || line.trim_start().starts_with("panic")
            || line.trim_start().starts_with("assert")
        {
            return 0.9;
        }
        if line.contains("test result:")
            || line.contains("FAILED")
            || line.starts_with("===")
            || line.starts_with("---")
            || line.starts_with("TOTAL")
        {
            return 0.8;
        }
        if has_word(line, "WARN") || has_word(line, "WARNING") {
            return 0.7;
        }
        if has_word(line, "INFO") || has_word(line, "DEBUG") || has_word(line, "Compiling") {
            return 0.2;
        }
        0.3
    }

    /// M4: log template-collapse — normalize digits to `#`, collapse
    /// runs of >=3 identical templates into one line with `xN`.
    fn m4_template_collapse(body: &str) -> String {
        let lines: Vec<&str> = body.lines().collect();
        if lines.len() < 30 {
            return body.to_string();
        }
        let mut out: Vec<String> = Vec::new();
        let mut i = 0usize;
        while i < lines.len() {
            let tpl = templ(lines[i]);
            let is_noise = template_priority(&tpl) < 0.5;
            if !is_noise {
                out.push(lines[i].to_string());
                i += 1;
                continue;
            }
            let mut run = 1usize;
            while i + run < lines.len() && templ(lines[i + run]) == tpl {
                run += 1;
            }
            if run >= 3 {
                out.push(format!("{} [x{}]", lines[i].trim(), run));
            } else {
                for k in 0..run {
                    out.push(lines[i + k].to_string());
                }
            }
            i += run;
        }
        if out.len() == lines.len() {
            return body.to_string();
        }
        let omitted = lines.len() - out.len();
        let mut s = out.join("\n");
        s.push_str(&format!("\n[{omitted} repeated lines collapsed]"));
        s
    }

    fn templ(line: &str) -> String {
        let mut t = String::with_capacity(line.len());
        let mut prev_digit = false;
        for ch in line.chars() {
            if ch.is_ascii_digit() {
                if !prev_digit {
                    t.push('#');
                }
                prev_digit = true;
            } else {
                prev_digit = false;
                t.push(ch);
            }
        }
        t
    }

    fn template_priority(tpl: &str) -> f32 {
        if has_word(tpl, "ERROR") || has_word(tpl, "FATAL") || tpl.contains("error[E") {
            0.95
        } else if has_word(tpl, "WARN") || has_word(tpl, "WARNING") {
            0.7
        } else {
            0.2
        }
    }

    fn render_kept(lines: &[&str], keep: &[bool]) -> String {
        let selected: Vec<&str> = lines
            .iter()
            .enumerate()
            .filter(|(i, _)| keep[*i])
            .map(|(_, l)| *l)
            .collect();
        let omitted = lines.len() - selected.len();
        if omitted == 0 {
            return lines.join("\n");
        }
        let mut s = selected.join("\n");
        s.push_str(&format!("\n[{omitted} lines omitted]"));
        s
    }

    #[test]
    #[ignore]
    fn mech_ablation_report() {
        let dir = tempfile::tempdir().unwrap();
        let rt = enabled_rt(dir.path());

        // samples with must-keep needles: (label, text, retention needles)
        let dup_item = serde_json::json!({"path": "src/common.rs", "size": 1024, "lines": 40});
        let mut json_dup: Vec<Value> = Vec::new();
        for i in 0..80 {
            if i % 10 == 9 {
                json_dup.push(serde_json::json!({"path": format!("src/unique{i}.rs"), "size": 500 + i, "lines": i}));
            } else {
                json_dup.push(dup_item.clone());
            }
        }
        let json_dup = serde_json::to_string_pretty(&json_dup).unwrap();
        let json_unique = sample_json_array();
        let logs_fatal = {
            let mut s = String::from("exit: 1\n");
            for i in 0..100 {
                s.push_str(&format!("INFO heartbeat worker {} ok\n", i % 7));
                if i == 50 {
                    s.push_str("FATAL db connection lost\n  at pool.rs:88\n  --> query timed out\n");
                    s.push_str("WARN retrying with backoff\n");
                }
            }
            s
        };
        let logs_repeated = {
            let mut s = String::from("exit: 0\n");
            for i in 0..60 {
                s.push_str(&format!("DEBUG cache hit key={} shard={}\n", i, i % 4));
                if i == 30 {
                    s.push_str("WARN eviction pressure on shard 3\n");
                }
            }
            s
        };
        let diff = sample_diff();
        let search = sample_search();

        let samples: Vec<(&str, String, Vec<&str>)> = vec![
            (
                "json_dup80",
                json_dup,
                vec!["src/common.rs", "x7", "unique9.rs", "80 items"],
            ),
            (
                "json_unique80",
                json_unique,
                vec!["module0", "module79", "mtime"],
            ),
            (
                "logs_fatal_mid",
                logs_fatal,
                vec!["FATAL db connection lost", "pool.rs:88", "backoff", "exit: 1"],
            ),
            (
                "logs_repeated",
                logs_repeated,
                vec!["eviction pressure", "exit: 0"],
            ),
            (
                "diff_big",
                diff,
                vec!["diff --git a/src/f0.rs", "+added line 3 in f0 hunk0", "+++"],
            ),
            (
                "search_big",
                search,
                vec!["mod.rs:10:", "mod.rs:69:"],
            ),
        ];

        // mechanism roster, applied by content kind
        println!(
            "\n{:<16} {:<22} {:>7} {:>7} {:>7}  misses",
            "sample", "mechanism", "orig", "new", "saved%"
        );
        for (name, text, needles) in &samples {
            let orig = estimate_tokens(text) as u64;
            let is_json = name.starts_with("json");
            let prods: Vec<(&str, Box<dyn Fn(&str) -> String + '_>)> = if is_json {
                vec![
                    ("baseline (current)", Box::new(|t| compress_text(t, &rt))),
                    ("M1 dedup", Box::new(m1_dedup_json)),
                    ("M2 adaptive-k", Box::new(m2_adaptive_k)),
                    (
                        "M1+M2 dedup+adaptive",
                        Box::new(|t| m2_adaptive_k(&m1_dedup_json(t))),
                    ),
                ]
            } else if name.starts_with("logs") {
                vec![
                    ("baseline (current)", Box::new(|t| compress_text(t, &rt))),
                    ("M3 tiered-rank", Box::new(m3_tiered_logs)),
                    ("M4 template-collapse", Box::new(m4_template_collapse)),
                    ("M3+M4 collapse+rank", Box::new(|t| m3_tiered_logs(&m4_template_collapse(t)))),
                ]
            } else {
                vec![
                    ("baseline (current)", Box::new(|t| compress_text(t, &rt))),
                ]
            };
            for (mname, f) in &prods {
                let out = f(text);
                let new = estimate_tokens(&out) as u64;
                let saved = 100.0 * orig.saturating_sub(new) as f64 / orig as f64;
                let misses: Vec<&str> = needles
                    .iter()
                    .filter(|n| !out.contains(**n))
                    .copied()
                    .collect();
                println!(
                    "{:<16} {:<22} {:>7} {:>7} {:>6.1}%  {}",
                    name,
                    mname,
                    orig,
                    new,
                    saved,
                    if misses.is_empty() {
                        "retain-all".to_string()
                    } else {
                        format!("LOST: {}", misses.join(", "))
                    }
                );
            }
            println!();
        }
    }
}
