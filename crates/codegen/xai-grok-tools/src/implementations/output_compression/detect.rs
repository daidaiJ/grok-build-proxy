//! Content-type detection, simplified from headroom/only-cc-lite.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentKind {
    JsonArray,
    JsonObject,
    Diff,
    Search,
    Logs,
    Other,
}

impl ContentKind {
    pub(crate) fn strategy_name(self) -> &'static str {
        match self {
            Self::JsonArray | Self::JsonObject => "json",
            Self::Diff => "diff",
            Self::Search => "search",
            Self::Logs => "logs",
            Self::Other => "generic",
        }
    }
}

pub(crate) fn detect(content: &str) -> ContentKind {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return ContentKind::Other;
    }
    if trimmed.starts_with('[') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if value.is_array() {
                return ContentKind::JsonArray;
            }
        }
    }
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if value.is_object() {
                return ContentKind::JsonObject;
            }
        }
    }
    if looks_like_diff(content) {
        return ContentKind::Diff;
    }
    if looks_like_search(content) {
        return ContentKind::Search;
    }
    if looks_like_logs(content) {
        return ContentKind::Logs;
    }
    ContentKind::Other
}

fn looks_like_diff(content: &str) -> bool {
    let mut headers = 0u32;
    for line in content.lines().take(500) {
        if line.starts_with("diff --git")
            || line.starts_with("diff --combined ")
            || line.starts_with("diff --cc ")
            || line.starts_with("--- a/")
            || line.starts_with("+++ b/")
            || (line.starts_with("@@ ") && line.contains("@@"))
        {
            headers += 1;
        }
        if headers >= 1 {
            return true;
        }
    }
    false
}

fn looks_like_search(content: &str) -> bool {
    let mut matching = 0u32;
    let mut non_empty = 0u32;
    for line in content.lines().take(100) {
        if line.trim().is_empty() {
            continue;
        }
        non_empty += 1;
        if is_search_line(line) {
            matching += 1;
        }
    }
    non_empty > 0 && matching * 10 >= non_empty * 3
}

fn is_search_line(line: &str) -> bool {
    // grep -n: path:line:rest — path has no spaces at the start.
    let bytes = line.as_bytes();
    if bytes.is_empty() || bytes[0].is_ascii_whitespace() {
        return false;
    }
    let Some(colon) = line.find(':') else {
        return false;
    };
    if colon == 0 {
        return false;
    }
    let rest = &line[colon + 1..];
    let digits = rest.bytes().take_while(|b| b.is_ascii_digit()).count();
    digits > 0 && rest.as_bytes().get(digits) == Some(&b':')
}

fn looks_like_logs(content: &str) -> bool {
    let mut hits = 0u32;
    let mut non_empty = 0u32;
    for line in content.lines().take(200) {
        if line.trim().is_empty() {
            continue;
        }
        non_empty += 1;
        if log_hit(line) {
            hits += 1;
        }
    }
    non_empty > 0 && hits * 10 >= non_empty
}

fn log_hit(line: &str) -> bool {
    has_word(line, "ERROR")
        || has_word(line, "error")
        || has_word(line, "FAIL")
        || has_word(line, "FAILED")
        || has_word(line, "WARN")
        || has_word(line, "WARNING")
        || has_word(line, "INFO")
        || has_word(line, "DEBUG")
        || line.contains("Traceback (most recent call last)")
        || line.contains("npm ERR!")
        || line.contains("error[E")
        || line.starts_with("===")
        || line.starts_with("---")
        || line.contains("Compiling ")
}

pub(crate) fn has_word(line: &str, word: &str) -> bool {
    let bytes = line.as_bytes();
    let needle = word.as_bytes();
    if needle.is_empty() || bytes.len() < needle.len() {
        return false;
    }
    for i in 0..=bytes.len() - needle.len() {
        if &bytes[i..i + needle.len()] == needle {
            let left_ok = i == 0 || !is_word_byte(bytes[i - 1]);
            let right_ok =
                i + needle.len() == bytes.len() || !is_word_byte(bytes[i + needle.len()]);
            if left_ok && right_ok {
                return true;
            }
        }
    }
    false
}

fn is_word_byte(b: u8) -> bool {
    matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_json_array() {
        assert_eq!(detect(r#"[{"id":1},{"id":2}]"#), ContentKind::JsonArray);
    }

    #[test]
    fn detects_json_object() {
        assert_eq!(
            detect(r#"{"stdout":"x","exit_code":0}"#),
            ContentKind::JsonObject
        );
    }

    #[test]
    fn detects_diff() {
        let content =
            "diff --git a/foo.rs b/foo.rs\n--- a/foo.rs\n+++ b/foo.rs\n@@ -1,2 +1,3 @@\n+hi\n";
        assert_eq!(detect(content), ContentKind::Diff);
    }

    #[test]
    fn detects_search() {
        let content = "src/main.rs:12:fn main() {}\nsrc/lib.rs:4:pub fn x() {}\n";
        assert_eq!(detect(content), ContentKind::Search);
    }

    #[test]
    fn detects_logs() {
        let content = "[INFO] start\n[ERROR] boom\n[WARN] deprecated\nFAILED test_one\n";
        assert_eq!(detect(content), ContentKind::Logs);
    }

    #[test]
    fn word_boundary_does_not_overfire() {
        assert!(!has_word("errorless", "error"));
        assert!(has_word("ERROR: boom", "ERROR"));
    }
}
