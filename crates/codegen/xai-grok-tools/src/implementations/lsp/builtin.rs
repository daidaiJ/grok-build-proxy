//! Builtin language servers for common languages, detected on PATH.
//!
//! Well-known servers (rust-analyzer / gopls / the pyright family / jdtls /
//! clangd) are merged beneath user/project/plugin config so a fresh checkout
//! works with zero `lsp.json`. Detection only stats PATH candidates — it never
//! spawns the server binary (the qwen-code lesson: probes must not execute
//! config-controlled commands; these are hardcoded names, and spawn still
//! belongs to the manager).

use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;

use super::config::LspServerConfig;
use crate::types::config_source::ConfigSource;

/// One well-known server: how to spawn it and which file extensions it claims.
struct BuiltinServer {
    name: &'static str,
    command: &'static str,
    args: &'static [&'static str],
    /// Extension (with dot) → LSP language id.
    extensions: &'static [(&'static str, &'static str)],
}

/// Ordered alternatives per language: the first candidate whose command is
/// found on PATH wins and the rest are not probed. Config always wins over a
/// builtin (see [`merge_builtin_servers`]).
const BUILTIN_SLOTS: &[&[BuiltinServer]] = &[
    &[BuiltinServer {
        name: "rust-analyzer",
        command: "rust-analyzer",
        args: &[],
        extensions: &[(".rs", "rust")],
    }],
    &[BuiltinServer {
        name: "gopls",
        command: "gopls",
        args: &[],
        extensions: &[(".go", "go")],
    }],
    &[
        BuiltinServer {
            name: "pyright",
            command: "pyright-langserver",
            args: &["--stdio"],
            extensions: &[(".py", "python"), (".pyi", "python")],
        },
        BuiltinServer {
            name: "basedpyright",
            command: "basedpyright-langserver",
            args: &["--stdio"],
            extensions: &[(".py", "python"), (".pyi", "python")],
        },
        BuiltinServer {
            name: "pylsp",
            command: "pylsp",
            args: &[],
            extensions: &[(".py", "python"), (".pyi", "python")],
        },
    ],
    &[BuiltinServer {
        name: "jdtls",
        command: "jdtls",
        args: &[],
        extensions: &[(".java", "java")],
    }],
    &[BuiltinServer {
        // .h 走 "cpp"：clangd 由伴随的 .c/.cpp 推断方言，header 的 language id
        // 只是提示（VS Code clangd 扩展同样把 .h 映射为 cpp）。
        name: "clangd",
        command: "clangd",
        args: &[],
        extensions: &[
            (".c", "c"),
            (".h", "cpp"),
            (".cpp", "cpp"),
            (".cc", "cpp"),
            (".cxx", "cpp"),
            (".hpp", "cpp"),
            (".hh", "cpp"),
            (".hxx", "cpp"),
        ],
    }],
];

/// Executable-name suffixes tried when probing a bare command name. Windows
/// resolves through PATHEXT (CreateProcess semantics); elsewhere the bare name.
#[cfg(windows)]
fn probe_suffixes() -> Vec<String> {
    let from_env = std::env::var("PATHEXT")
        .ok()
        .map(|v| {
            v.split(';')
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty());
    from_env.unwrap_or_else(|| {
        [".com", ".exe", ".bat", ".cmd"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    })
}

#[cfg(not(windows))]
fn probe_suffixes() -> Vec<String> {
    vec![String::new()]
}

/// Resolve a command name against explicit directories. A command containing a
/// path separator is taken as a path (exists-check only). Returns the probed
/// file so callers can log where a server was found.
pub fn probe_command(dirs: &[PathBuf], command: &str) -> Option<PathBuf> {
    if command.contains('/') || command.contains('\\') {
        let p = PathBuf::from(command);
        return p.is_file().then_some(p);
    }
    for dir in dirs {
        for suffix in &probe_suffixes() {
            let candidate = dir.join(format!("{command}{suffix}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Detect builtin servers on the process PATH.
pub fn detect_builtin_servers() -> BTreeMap<String, (LspServerConfig, ConfigSource)> {
    let dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    detect_builtin_servers_in(&dirs)
}

/// Detect builtin servers against explicit directories (tests inject a fake PATH).
pub fn detect_builtin_servers_in(
    dirs: &[PathBuf],
) -> BTreeMap<String, (LspServerConfig, ConfigSource)> {
    let mut out = BTreeMap::new();
    for slot in BUILTIN_SLOTS {
        for candidate in *slot {
            let Some(path) = probe_command(dirs, candidate.command) else {
                continue;
            };
            let cfg = LspServerConfig {
                command: candidate.command.to_string(),
                args: candidate.args.iter().map(|s| s.to_string()).collect(),
                extensions: candidate
                    .extensions
                    .iter()
                    .map(|(ext, lang)| (ext.to_string(), lang.to_string()))
                    .collect(),
                ..LspServerConfig::default()
            };
            tracing::info!(
                server = candidate.name,
                path = %path.display(),
                "builtin LSP server detected on PATH"
            );
            out.insert(candidate.name.to_string(), (cfg, ConfigSource::Builtin));
            break;
        }
    }
    out
}

/// Insert detected builtins beneath user/project/plugin config: a builtin is
/// skipped when its name is already defined or when any configured server
/// already claims one of its file extensions — config fills intent, builtins
/// fill gaps.
pub fn merge_builtin_servers(
    sourced: &mut BTreeMap<String, (LspServerConfig, ConfigSource)>,
    builtins: BTreeMap<String, (LspServerConfig, ConfigSource)>,
) {
    let claimed: HashSet<String> = sourced
        .values()
        .flat_map(|(cfg, _)| cfg.extensions.keys().cloned())
        .collect();
    for (name, (cfg, source)) in builtins {
        if sourced.contains_key(&name) {
            continue;
        }
        if cfg.extensions.keys().any(|ext| claimed.contains(ext)) {
            continue;
        }
        sourced.insert(name, (cfg, source));
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LspServerConfig, detect_builtin_servers_in, merge_builtin_servers, probe_command,
    };
    use crate::types::config_source::ConfigSource;
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    fn sourced_from(entries: &[(&str, &[(&str, &str)])]) -> BTreeMap<String, (LspServerConfig, ConfigSource)> {
        let mut m = BTreeMap::new();
        for (name, exts) in entries {
            let cfg = LspServerConfig {
                extensions: exts
                    .iter()
                    .map(|(e, l)| (e.to_string(), l.to_string()))
                    .collect(),
                ..Default::default()
            };
            m.insert(
                name.to_string(),
                (cfg, ConfigSource::User { path: PathBuf::new() }),
            );
        }
        m
    }

    fn builtin_map(entries: &[(&str, &[(&str, &str)])]) -> BTreeMap<String, (LspServerConfig, ConfigSource)> {
        let mut m = BTreeMap::new();
        for (name, exts) in entries {
            let cfg = LspServerConfig {
                extensions: exts
                    .iter()
                    .map(|(e, l)| (e.to_string(), l.to_string()))
                    .collect(),
                ..Default::default()
            };
            m.insert(name.to_string(), (cfg, ConfigSource::Builtin));
        }
        m
    }

    /// A file whose name matches how this platform probes bare commands
    /// (`rust-analyzer.exe` on Windows, `rust-analyzer` elsewhere).
    fn probe_named(dir: &Path, name: &str) -> PathBuf {
        let suffix = super::probe_suffixes()
            .into_iter()
            .next()
            .unwrap_or_default();
        let p = dir.join(format!("{name}{suffix}"));
        std::fs::write(&p, b"").unwrap();
        p
    }

    #[test]
    fn probe_finds_first_match_across_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir(&a).unwrap();
        std::fs::create_dir(&b).unwrap();
        let in_b = probe_named(&b, "gopls");
        let dirs = vec![a, b];
        assert_eq!(probe_command(&dirs, "gopls"), Some(in_b));
        assert_eq!(probe_command(&dirs, "missing"), None);
    }

    /// Per-language regression locks (CC #16084 planted rust-analyzer detection
    /// on Windows; one test per builtin slot so a broken candidate cannot slip
    /// through unnoticed).
    #[test]
    fn detects_rust_analyzer_and_maps_rs() {
        let tmp = tempfile::tempdir().unwrap();
        probe_named(tmp.path(), "rust-analyzer");
        let detected = detect_builtin_servers_in(&[tmp.path().to_path_buf()]);
        let (cfg, _) = detected.get("rust-analyzer").expect("rust slot detected");
        assert_eq!(cfg.extensions.get(".rs").map(String::as_str), Some("rust"));
    }

    #[test]
    fn detects_gopls_and_maps_go() {
        let tmp = tempfile::tempdir().unwrap();
        probe_named(tmp.path(), "gopls");
        let detected = detect_builtin_servers_in(&[tmp.path().to_path_buf()]);
        let (cfg, _) = detected.get("gopls").expect("go slot detected");
        assert_eq!(cfg.extensions.get(".go").map(String::as_str), Some("go"));
    }

    #[test]
    fn detects_basedpyright_as_python_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        // 只播种第二顺位候选：锁住 python 槽位的降级行为。
        probe_named(tmp.path(), "basedpyright-langserver");
        let detected = detect_builtin_servers_in(&[tmp.path().to_path_buf()]);
        let (cfg, _) = detected.get("basedpyright").expect("python fallback detected");
        assert_eq!(cfg.extensions.get(".py").map(String::as_str), Some("python"));
        assert_eq!(cfg.extensions.get(".pyi").map(String::as_str), Some("python"));
    }

    #[test]
    fn detects_jdtls_and_maps_java() {
        let tmp = tempfile::tempdir().unwrap();
        probe_named(tmp.path(), "jdtls");
        let detected = detect_builtin_servers_in(&[tmp.path().to_path_buf()]);
        let (cfg, _) = detected.get("jdtls").expect("java slot detected");
        assert_eq!(cfg.extensions.get(".java").map(String::as_str), Some("java"));
    }

    #[test]
    fn detects_clangd_and_maps_c_family() {
        let tmp = tempfile::tempdir().unwrap();
        probe_named(tmp.path(), "clangd");
        let detected = detect_builtin_servers_in(&[tmp.path().to_path_buf()]);
        let (cfg, _) = detected.get("clangd").expect("c-family slot detected");
        assert_eq!(cfg.extensions.get(".c").map(String::as_str), Some("c"));
        for header in [".h", ".cpp", ".cc", ".hpp"] {
            assert_eq!(
                cfg.extensions.get(header).map(String::as_str),
                Some("cpp"),
                "{header} maps to cpp"
            );
        }
    }

    /// Windows probes bare commands through PATHEXT-style suffixes
    /// (CreateProcess semantics): `gopls.exe` is found, an extensionless
    /// `clangd` is not — mirroring what actually launches. The returned path
    /// carries PATHEXT's own casing (`.EXE`), and NTFS does not care.
    #[cfg(windows)]
    #[test]
    fn windows_probe_follows_pathext_suffixes() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = tmp.path().join("gopls.exe");
        std::fs::write(&exe, b"").unwrap();
        let dirs = [tmp.path().to_path_buf()];
        let found = probe_command(&dirs, "gopls").expect("gopls.exe found via PATHEXT");
        assert!(
            found.to_string_lossy().eq_ignore_ascii_case(&exe.to_string_lossy()),
            "found {}, expected {}",
            found.display(),
            exe.display()
        );

        let bare = tmp.path().join("clangd");
        std::fs::write(&bare, b"").unwrap();
        assert_eq!(
            probe_command(&dirs, "clangd"),
            None,
            "an extensionless file is not an executable on Windows"
        );
    }

    #[test]
    fn probe_treats_separated_command_as_path() {
        let tmp = tempfile::tempdir().unwrap();
        let file = probe_named(tmp.path(), "gopls");
        assert_eq!(
            probe_command(&[], &file.to_string_lossy()),
            Some(file.clone())
        );
        let missing = tmp.path().join("nope");
        assert_eq!(probe_command(&[], &missing.to_string_lossy()), None);
    }

    #[test]
    fn detect_picks_one_server_per_slot_in_priority_order() {
        let tmp = tempfile::tempdir().unwrap();
        probe_named(tmp.path(), "pylsp");
        // pyright 优先于 pylsp：两者都在时取前者。
        probe_named(tmp.path(), "pyright-langserver");
        probe_named(tmp.path(), "rust-analyzer");
        let detected = detect_builtin_servers_in(&[tmp.path().to_path_buf()]);
        assert!(detected.contains_key("rust-analyzer"));
        assert!(detected.contains_key("pyright"), "priority candidate wins");
        assert!(!detected.contains_key("pylsp"));
        assert!(!detected.contains_key("gopls"));
        assert!(detected
            .values()
            .all(|(_, source)| matches!(source, ConfigSource::Builtin)));
    }

    #[test]
    fn merge_fills_name_and_extension_gaps() {
        let builtins = builtin_map(&[
            ("rust-analyzer", &[(".rs", "rust")]),
            ("gopls", &[(".go", "go")]),
            ("pyright", &[(".py", "python"), (".pyi", "python")]),
        ]);

        // 同名：config 定义压过 builtin（rust-analyzer 保持 config 来源），
        // 其余 builtin 照常补位。
        let mut sourced = sourced_from(&[("rust-analyzer", &[(".rs", "rust")])]);
        merge_builtin_servers(&mut sourced, builtins.clone());
        assert_eq!(sourced.len(), 3);
        assert!(matches!(
            sourced.get("rust-analyzer").unwrap().1,
            ConfigSource::User { .. }
        ));
        assert!(matches!(
            sourced.get("gopls").unwrap().1,
            ConfigSource::Builtin
        ));

        // 扩展被占：.rs/.go 已被 config 认领 → 对应 builtin 不进；python 未被
        // 认领 → 照进。
        let mut sourced = sourced_from(&[
            ("rust-analyzer", &[(".rs", "rust")]),
            ("custom-go", &[(".go", "go")]),
        ]);
        merge_builtin_servers(&mut sourced, builtins.clone());
        assert!(!sourced.contains_key("gopls"));
        assert!(sourced.contains_key("pyright"));

        // 空配置：全部进。
        let mut sourced = sourced_from(&[]);
        merge_builtin_servers(&mut sourced, builtins);
        assert_eq!(sourced.len(), 3);
    }
}
