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
}
