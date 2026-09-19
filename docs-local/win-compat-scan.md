# Windows 兼容性静态扫描报告（生成于 2026-09-19，工具 scripts-local/win_scan.py，勿手改）

verdict 含义：COMPILE-BREAK=未门控 unix import，测试目标在 Windows 编译不过（本机永不跑）；
RISK=风险文件占比>10%，RISK-LIGHT=少量；CLEAN=无特征命中（仍可能有动态耦合，靠抽样兜底）。
feature 门控的测试默认构建不编译，不计入 verdict，单列「feature 名单」供抽样时核对。

## 汇总

| crate | 测试文件数 | #test | verdict | 高危家族 | feature 名单 |
|---|---|---|---|---|---|
| ptyctl | 3 | 15 | RISK | posix-path | - |
| xai-acp-lib | 5 | 17 | CLEAN | - | - |
| xai-agent-lifecycle | 2 | 5 | CLEAN | - | - |
| xai-chat-state | 11 | 365 | CLEAN | - | - |
| xai-circuit-breaker | 12 | 77 | CLEAN | - | - |
| xai-codebase-graph | 6 | 58 | CLEAN | - | - |
| xai-compaction-transcript | 1 | 9 | CLEAN | - | - |
| xai-computer-hub-core | 9 | 68 | CLEAN | - | - |
| xai-computer-hub-mcp-adapter | 1 | 12 | CLEAN | - | - |
| xai-computer-hub-sdk | 16 | 242 | CLEAN | - | metrics |
| xai-crash-handler | 6 | 22 | RISK | perm-mode, posix-path, signal | - |
| xai-dirs | 1 | 5 | RISK | symlink, posix-path | - |
| xai-fast-worktree | 41 | 575 | COMPILE-BREAK | posix-path, symlink, unix-import | metadata |
| xai-file-utils | 7 | 208 | RISK | posix-path | - |
| xai-fsnotify | 6 | 115 | RISK | symlink | - |
| xai-fuzzy-file-search | 1 | 5 | CLEAN | - | - |
| xai-gix-status | 1 | 9 | CLEAN | - | - |
| xai-grok-active-sessions | 2 | 6 | RISK | posix-path | - |
| xai-grok-agent | 20 | 556 | RISK | posix-path, symlink, env-home | - |
| xai-grok-announcements | 1 | 11 | CLEAN | - | - |
| xai-grok-auth | 2 | 7 | CLEAN | - | - |
| xai-grok-bundle | 1 | 39 | RISK | perm-mode | - |
| xai-grok-compaction | 17 | 133 | CLEAN | - | - |
| xai-grok-config | 18 | 223 | RISK | posix-path, symlink | - |
| xai-grok-config-types | 5 | 56 | CLEAN | - | - |
| xai-grok-dashboard-store | 5 | 31 | CLEAN | - | - |
| xai-grok-diag-server | 1 | 21 | CLEAN | - | - |
| xai-grok-env | 2 | 8 | CLEAN | - | - |
| xai-grok-extra-ca | 9 | 23 | CLEAN | - | - |
| xai-grok-feedback | 5 | 32 | RISK | symlink | - |
| xai-grok-foreign-sessions | 7 | 60 | RISK | symlink | - |
| xai-grok-gboom | 4 | 35 | CLEAN | - | - |
| xai-grok-hooks | 12 | 287 | RISK | posix-path, sh-exec | - |
| xai-grok-http | 1 | 14 | CLEAN | - | - |
| xai-grok-image | 1 | 40 | CLEAN | - | - |
| xai-grok-login | 29 | 463 | RISK | posix-path, sh-exec, perm-mode | - |
| xai-grok-markdown | 12 | 484 | RISK | term-detect, posix-path | - |
| xai-grok-markdown-core | 1 | 42 | CLEAN | - | - |
| xai-grok-mcp | 11 | 226 | RISK | posix-path, perm-mode | - |
| xai-grok-memory | 19 | 368 | RISK | posix-path, symlink, perm-mode | - |
| xai-grok-mermaid | 7 | 60 | RISK | signal | - |
| xai-grok-otel | 5 | 32 | RISK | term-detect, posix-path | - |
| xai-grok-pager | 395 | 10093 | RISK | posix-path, term-detect, signal | local-workspace, release-dist |
| xai-grok-pager-bin | 1 | 39 | CLEAN | - | - |
| xai-grok-pager-diff | 1 | 28 | RISK | posix-path | - |
| xai-grok-pager-minimal | 9 | 82 | RISK | posix-path | - |
| xai-grok-pager-pty-harness | 242 | 455 | RISK-LIGHT | posix-path, signal, term-detect | - |
| xai-grok-pager-render | 58 | 1176 | RISK | posix-path, term-detect, env-home | - |
| xai-grok-paths | 1 | 19 | CLEAN | - | - |
| xai-grok-plugin-marketplace | 10 | 150 | RISK | posix-path, symlink | - |
| xai-grok-sampler | 26 | 303 | CLEAN | - | - |
| xai-grok-sampling-types | 12 | 301 | RISK-LIGHT | posix-path | - |
| xai-grok-sandbox | 14 | 151 | COMPILE-BREAK | symlink, posix-path, unix-import | - |
| xai-grok-secrets | 1 | 16 | RISK | env-home | - |
| xai-grok-session-events | 3 | 16 | CLEAN | - | - |
| xai-grok-session-search | 5 | 56 | RISK | perm-mode | - |
| xai-grok-shared | 4 | 140 | RISK | posix-path, symlink | - |
| xai-grok-shell | 442 | 6811 | COMPILE-BREAK | posix-path, symlink, env-home | dhat-heap, local-workspace, test-support |
| xai-grok-shell-base | 10 | 81 | RISK | signal, perm-mode, sh-exec | - |
| xai-grok-shell-session-support | 1 | 11 | CLEAN | - | - |
| xai-grok-shell-terminal | 6 | 79 | RISK | signal, posix-path, pty-fork | - |
| xai-grok-status-line | 2 | 18 | RISK | posix-path | - |
| xai-grok-subagent-resolution | 6 | 93 | CLEAN | - | - |
| xai-grok-telemetry | 39 | 284 | RISK | posix-path, symlink, perm-mode | - |
| xai-grok-test-support | 8 | 82 | RISK | term-detect, posix-path, env-home | - |
| xai-grok-tools | 185 | 3295 | COMPILE-BREAK | posix-path, signal, symlink | dhat-heap |
| xai-grok-tools-api | 4 | 23 | CLEAN | - | - |
| xai-grok-update | 13 | 281 | COMPILE-BREAK | symlink, posix-path, perm-mode | - |
| xai-grok-version | 1 | 2 | CLEAN | - | - |
| xai-grok-voice | 12 | 53 | COMPILE-BREAK | perm-mode, unix-import, sh-exec | audio |
| xai-grok-workspace | 77 | 2060 | RISK | posix-path, symlink, env-home | compression |
| xai-grok-workspace-client | 1 | 15 | CLEAN | - | - |
| xai-grok-workspace-daemon | 2 | 66 | RISK | posix-path, symlink, signal | - |
| xai-grok-workspace-types | 29 | 134 | RISK | posix-path, symlink | - |
| xai-hooks-plugins-types | 1 | 20 | RISK | posix-path | - |
| xai-hunk-tracker | 5 | 184 | RISK | posix-path, symlink | - |
| xai-interjection-core | 3 | 16 | CLEAN | - | - |
| xai-message-delivery-core | 3 | 17 | CLEAN | - | - |
| xai-mixpanel | 1 | 1 | CLEAN | - | - |
| xai-prompt-queue | 2 | 14 | CLEAN | - | - |
| xai-proto-build | 1 | 2 | CLEAN | - | - |
| xai-ratatui-inline | 7 | 51 | CLEAN | - | scrolling-regions |
| xai-ratatui-textarea | 6 | 374 | CLEAN | - | - |
| xai-sqlite-journal | 1 | 19 | CLEAN | - | - |
| xai-system-power | 2 | 6 | CLEAN | - | - |
| xai-test-utils | 1 | 2 | RISK | posix-path | - |
| xai-token-estimation | 1 | 15 | CLEAN | - | - |
| xai-tool-protocol | 12 | 260 | RISK | posix-path, signal | - |
| xai-tool-runtime | 13 | 119 | RISK-LIGHT | posix-path | - |
| xai-tool-types | 5 | 99 | RISK | signal | prompt-render |
| xai-tracing | 3 | 13 | CLEAN | - | - |
| xai-tty-utils | 8 | 73 | COMPILE-BREAK | signal, posix-path, unix-import | - |
| xai-workflow | 4 | 61 | RISK | symlink | - |

## ptyctl — RISK

### `crates\codegen\ptyctl\src\session.rs`（8 tests, unix 自门控命中 3）
- **posix-path** ×5
  - L453: `let session = start_session(vec!["/bin/sh".into()]).await;`
  - L502: `let session = start_session(vec!["/bin/sh".into()]).await;`
  - L533: `let session = start_session(vec!["/bin/sh".into()]).await;`
  - L555: `let session = start_session(vec!["/bin/sleep".into(), "30".into()]).await;`
  - L583: `let session = start_session(vec!["/bin/sh".into()]).await;`


## xai-acp-lib — CLEAN


## xai-agent-lifecycle — CLEAN


## xai-chat-state — CLEAN


## xai-circuit-breaker — CLEAN


## xai-codebase-graph — CLEAN


## xai-compaction-transcript — CLEAN


## xai-computer-hub-core — CLEAN


## xai-computer-hub-mcp-adapter — CLEAN


## xai-computer-hub-sdk — CLEAN

feature 门控测试（默认构建不跑，抽样不含）：metrics


## xai-crash-handler — RISK

### `crates\codegen\xai-crash-handler\src\handler.rs`（4 tests, unix 自门控命中 10）
- **perm-mode** ×2
  - L962: `mode & 0o777,`
  - L963: `0o600,`

### `crates\codegen\xai-crash-handler\src\lib.rs`（3 tests, unix 自门控命中 12）
- **posix-path** ×2
  - L20: `//! let crash_dir = PathBuf::from("/home/user/.myapp/crash");`
  - L189: `let dir = PathBuf::from("/tmp/xai-crash-handler-test-nonexistent");`

### `crates\codegen\xai-crash-handler\tests\integration.rs`（7 tests, unix 自门控命中 9）
- **signal** ×2
  - L79: `// Register a tokio SIGTERM handler (same as the pager does).`
  - L181: `"SIGUSR1 should be delivered through tokio\nstderr: {stderr}"`


## xai-dirs — RISK

### `crates\codegen\xai-dirs\src\lib.rs`（5 tests, unix 自门控命中 0）
- **symlink** ×2
  - L48: `/// Used as-is (not canonicalized) so literal prefix checks and symlink guards still see original components.`
  - L115: `// A real, existing dir whose canonical form differs (macOS symlinks`
- **posix-path** ×1
  - L106: `resolve_grok_home_from(Some(OsStr::new("/custom/home")), Some(Path::new("/home/u")));`


## xai-fast-worktree — COMPILE-BREAK

feature 门控测试（默认构建不跑，抽样不含）：metadata

### `crates\codegen\xai-fast-worktree\src\api.rs`（37 tests, unix 自门控命中 83, feature 门控命中 2）
- **symlink** ×10（共 10 处，仅列 5）
  - L58: `/// exposed via a namespace-crossing symlink the way a btrfs snapshot can.`
  - L147: `pub symlinks_copied: u64,`
  - L159: `symlinks_copied: stats.symlinks_copied,`
  - L758: `// symlink_metadata so a symlink-exposed worktree (btrfs snapshot layout),`
  - L760: `// and returns false for a broken symlink, leaking it.`

### `crates\codegen\xai-fast-worktree\src\api\gc\integration_tests.rs`（29 tests, unix 自门控命中 5）
- **symlink** ×2
  - L439: `/// A dangling worktree symlink (its target gone) reads as absent to `exists()`,`
  - L441: `/// record and leak the broken symlink on disk.`
- **posix-path** ×1
  - L792: `keep_worktrees_containing: vec![std::path::PathBuf::from("/tmp/p")],`

### `crates\codegen\xai-fast-worktree\src\api\gc\tests.rs`（10 tests, unix 自门控命中 2）
- **posix-path** ×2
  - L298: `let mut rec = rec_at("/tmp/nfs-wt", 1);`
  - L302: `vec![PathBuf::from("/tmp/nfs-wt/sub")],`

### `crates\codegen\xai-fast-worktree\src\bin\worktree_lifecycle_bench\tests.rs`（39 tests, unix 自门控命中 0）
- **sh-exec** ×7（共 7 处，仅列 5）
  - L210: `let mut command = std::process::Command::new("sh");`
  - L318: `"setsid sh -c 'sleep 0.1; touch {}' >/dev/null 2>&1 & wait",`
  - L321: `let mut command = std::process::Command::new("sh");`
  - L353: `let mut command = std::process::Command::new("sh");`
  - L376: `let mut command = std::process::Command::new("sh");`
- **signal** ×5
  - L413: `runtime::notify_signal_for_test(libc::SIGTERM).unwrap();`
  - L427: `libc::SIGTERM,`
  - L466: `unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) },`
  - L470: `assert_eq!(result.status.code(), Some(128 + libc::SIGTERM));`
  - L981: `libc::SIGTERM,`
- **perm-mode** ×2
  - L1016: `use std::os::unix::fs::PermissionsExt;`
  - L1040: `std::fs::set_permissions(repo.join("tracked"), std::fs::Permissions::from_mode(0o755)).unwrap();`
- **symlink** ×2
  - L1045: `std::os::unix::fs::symlink("first-target", repo.join("tracked")).unwrap();`
  - L1048: `std::os::unix::fs::symlink("second-target", repo.join("tracked")).unwrap();`
- **unix-import** ×1
  - L1016: `use std::os::unix::fs::PermissionsExt;`

### `crates\codegen\xai-fast-worktree\src\btrfs\detect.rs`（9 tests, unix 自门控命中 0）
- **posix-path** ×10（共 10 处，仅列 5）
  - L99: `|| (source.starts_with('/') && !source.starts_with("/dev/"));`
  - L110: `// For bind mounts, the source might be in format like "/dev/loop0[/repo]"`
  - L134: `let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")`
  - L282: `let mountinfo = std::fs::read_to_string("/proc/self/mountinfo")`
  - L503: `let result = resolve_via_subvol_mount("/dev/loop0", "/repo", mountinfo).unwrap();`
- **unix-import** ×1
  - L11: `use nix::sys::statfs::{BTRFS_SUPER_MAGIC, statfs};`
- **symlink** ×1
  - L364: `// created on the real mount and exposed at dest via a symlink.`

### `crates\codegen\xai-fast-worktree\src\btrfs\snapshot.rs`（25 tests, unix 自门控命中 0）
- **symlink** ×40（共 40 处，仅列 5）
  - L54: `/// The real on-disk btrfs snapshot subvolume. In the symlink case this`
  - L60: `pub symlink_path: Option<PathBuf>,`
  - L64: `/// then expose it at `dest` via a symlink. A bind mount would be namespace-local`
  - L65: `/// and die on process exit; a symlink persists and is visible to other shells.`
  - L74: `// it is namespace-independent and persistent. No symlink needed.`
- **posix-path** ×12（共 12 处，仅列 5）
  - L455: `let dest = Path::new("/tmp/nonexistent_dest");`
  - L479: `let dest_path = PathBuf::from("/tmp/test_snapshot_dest_nonbtrfs");`
  - L512: `mount_target: PathBuf::from("/home/user/.grok/worktrees/repo/session/wt-abc"),`
  - L529: `let mount_target = Path::new("/home/user/.grok/worktrees/wt-abc");`
  - L563: `let dest = Path::new("/home/user/.grok/worktrees/repo/session/wt-abc");`

### `crates\codegen\xai-fast-worktree\src\copy\cow.rs`（4 tests, unix 自门控命中 4）
- **perm-mode** ×5
  - L17: `std::fs::set_permissions(dest, perms)?;`
  - L74: `use std::os::unix::fs::PermissionsExt;`
  - L84: `perms.set_mode(0o755);`
  - L85: `std::fs::set_permissions(&src, perms).unwrap();`
  - L90: `assert_eq!(dest_perms.mode() & 0o777, 0o755);`
- **symlink** ×4
  - L21: `/// Point `dst` at `target`, replacing any existing entry. `symlink()` will not`
  - L25: `symlink_to(target, dst)`
  - L34: `fn symlink_to(target: &Path, dst: &Path) -> std::io::Result<()> {`
  - L35: `std::os::windows::fs::symlink_file(target, dst)`
- **unix-import** ×1
  - L74: `use std::os::unix::fs::PermissionsExt;`

### `crates\codegen\xai-fast-worktree\src\copy\engine.rs`（6 tests, unix 自门控命中 8）
- **symlink** ×1
  - L227: `symlinks_copied: symlinks_copied.load(Ordering::Relaxed),`

### `crates\codegen\xai-fast-worktree\src\copy\gitdir.rs`（12 tests, unix 自门控命中 6）
- **symlink** ×15（共 15 处，仅列 5）
  - L23: `pub symlinks_copied: u64,`
  - L97: `let symlinks_copied = AtomicU64::new(0);`
  - L121: `copy_single_entry(&item.source, &item.dest, &files_copied, &symlinks_copied)?;`
  - L133: `let symlinks_copied = &symlinks_copied;`
  - L140: `symlinks_copied,`

### `crates\codegen\xai-fast-worktree\src\db\tests.rs`（36 tests, unix 自门控命中 1）
- **posix-path** ×44（共 44 处，仅列 5）
  - L25: `let rec = make_record("abc", "/tmp/wt-abc", WorktreeKind::Session);`
  - L31: `assert_eq!(fetched.path, PathBuf::from("/tmp/wt-abc"));`
  - L42: `let rec = make_record("xyz", "/tmp/wt-xyz", WorktreeKind::Fork);`
  - L45: `let fetched = db.get("/tmp/wt-xyz").unwrap().expect("should find by path");`
  - L60: `db.register(&make_record("a", "/tmp/a", WorktreeKind::Session))`

### `crates\codegen\xai-fast-worktree\src\discovery.rs`（16 tests, unix 自门控命中 3）
- **posix-path** ×2
  - L646: `let gitdir = "/home/user/myrepo/.git/worktrees/wt";`
  - L650: `assert_eq!(source, Some(PathBuf::from("/home/user/myrepo")));`
- **symlink** ×1
  - L264: `// Refuse symlink escape outside managed roots.`

### `crates\codegen\xai-fast-worktree\src\git\checkout.rs`（19 tests, unix 自门控命中 1）
- **symlink** ×2
  - L212: `/// symlinks, or stat checks.`
  - L219: `"core.symlinks=true",`

### `crates\codegen\xai-fast-worktree\src\git\safety_tests\gate.rs`（8 tests, unix 自门控命中 0）
- **perm-mode** ×2
  - L159: `use std::os::unix::fs::PermissionsExt;`
  - L171: `std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).unwrap();`
- **unix-import** ×1
  - L159: `use std::os::unix::fs::PermissionsExt;`

### `crates\codegen\xai-fast-worktree\src\git\safety_tests\working_tree.rs`（13 tests, unix 自门控命中 2）
- **symlink** ×1
  - L35: `"untracked-directory-named-like-a-build-symlink",`
- **posix-path** ×1
  - L90: `run_git(&embedded, &["config", "core.excludesFile", "/dev/null"]);`

### `crates\codegen\xai-fast-worktree\src\git\worktree.rs`（7 tests, unix 自门控命中 4）
- **symlink** ×1
  - L180: `/// a symlinked spelling still compares equal after the path is deleted.`

### `crates\codegen\xai-fast-worktree\src\mount_info.rs`（19 tests, unix 自门控命中 0）
- **posix-path** ×8（共 8 处，仅列 5）
  - L50: `std::fs::read_to_string("/proc/self/mountinfo").context("read /proc/self/mountinfo")?;`
  - L168: `std::fs::read_link("/proc/self/ns/mnt"),`
  - L169: `std::fs::read_link("/proc/1/ns/mnt"),`
  - L317: `PathBuf::from("/var/lib/repo-fuse/instance/fuse-lower")`
  - L338: `PathBuf::from("/var/lib/repo-fuse/instance/fuse-lower")`

### `crates\codegen\xai-fast-worktree\src\nfs\client.rs`（24 tests, unix 自门控命中 0）
- **unix-import** ×3
  - L12: `use std::os::unix::ffi::OsStrExt;`
  - L13: `use std::os::unix::net::UnixStream;`
  - L929: `use std::os::unix::net::UnixListener;`
- **posix-path** ×2
  - L162: `.unwrap_or_else(|| PathBuf::from("/tmp/grove-missing-runtime"));`
  - L726: `PathBuf::from("/tmp/grove-missing/control.sock")`

### `crates\codegen\xai-fast-worktree\src\nfs\liveness.rs`（8 tests, unix 自门控命中 0, feature 门控命中 1）
- **symlink** ×1
  - L106: `None => std::fs::symlink_metadata(dest).is_err(),`

### `crates\codegen\xai-fast-worktree\src\nfs\mod.rs`（21 tests, unix 自门控命中 2）
- **unix-import** ×1
  - L433: `use std::os::unix::net::UnixListener;`

### `crates\codegen\xai-fast-worktree\src\nfs\mount_table.rs`（8 tests, unix 自门控命中 5）
- **unix-import** ×1
  - L5: `use std::os::unix::ffi::OsStrExt;`

### `crates\codegen\xai-fast-worktree\src\overlay\snapshot.rs`（5 tests, unix 自门控命中 0）
- **posix-path** ×11（共 11 处，仅列 5）
  - L578: `snapshot_root: PathBuf::from("/var/lib/repo-fuse/instance/worktrees/abc/root"),`
  - L579: `work_dir: PathBuf::from("/var/lib/repo-fuse/instance/worktrees/abc/work"),`
  - L580: `lower_dir: PathBuf::from("/var/lib/repo-fuse/instance/fuse-lower"),`
  - L581: `mount_target: PathBuf::from("/home/user/.grok/worktrees/abc"),`
  - L605: `"snapshot_upper": "/var/lib/repo-fuse/instance/worktrees/abc123/upper",`

### `crates\codegen\xai-fast-worktree\src\sync.rs`（34 tests, unix 自门控命中 23）
- **symlink** ×7（共 7 处，仅列 5）
  - L555: `/// Copy one dirty entry. `symlink_metadata` (not `exists`) so a dangling`
  - L556: `/// symlink is recreated, not skipped.`
  - L561: `let src_meta = match std::fs::symlink_metadata(&src_file) {`
  - L581: `.with_context(|| format!("failed to read symlink {}", src_file.display()))?;`
  - L583: `.with_context(|| format!("failed to recreate symlink {}", dst_file.display()));`

### `crates\codegen\xai-fast-worktree\src\worktree\execute.rs`（16 tests, unix 自门控命中 13）
- **symlink** ×2
  - L422: `/// namespace-local and cannot be exposed via a symlink. `Unknown` stays enabled`
  - L739: `/// Delete the subvolume, metadata, and exposing symlink. The symlink would`

### `crates\codegen\xai-fast-worktree\src\worktree\plan.rs`（4 tests, unix 自门控命中 4）
- **symlink** ×1
  - L10: `/// symlink follow); macOS `/tmp` `/var` rewrite so the two names stay one id.`

### `crates\codegen\xai-fast-worktree\tests\overlay_integration.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L53: `let content = match std::fs::read_to_string("/proc/self/mountinfo") {`
  - L189: `let Ok(content) = std::fs::read_to_string("/proc/self/mountinfo") else {`
  - L914: `let content = std::fs::read_to_string("/proc/self/mountinfo").ok()?;`


## xai-file-utils — RISK

### `crates\codegen\xai-file-utils\src\gcs.rs`（6 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L685: `std::path::Path::new("/tmp/nonexistent_upload_queue_test_file"),`
  - L706: `std::path::Path::new("/tmp/nonexistent_upload_queue_test_file"),`
  - L730: `std::path::Path::new("/tmp/file"),`

### `crates\codegen\xai-file-utils\src\workspace_classifier.rs`（25 tests, unix 自门控命中 0）
- **posix-path** ×6（共 6 处，仅列 5）
  - L62: `|| cwd.starts_with("/tmp/")`
  - L63: `|| cwd == Path::new("/var/tmp")`
  - L64: `|| cwd.starts_with("/var/tmp/")`
  - L65: `|| cwd.starts_with("/var/folders/")`
  - L182: `assert!(!is_project_dir(Path::new("/tmp/scratch")));`


## xai-fsnotify — RISK

### `crates\codegen\xai-fsnotify\src\source.rs`（26 tests, unix 自门控命中 2）
- **symlink** ×3
  - L66: `// A raw symlinked `cwd` could miss `.sl` while the watcher still attaches `wlock`, leaking `.sl/*`.`
  - L227: `// Reject non-regular files: a planted FIFO/symlink could block the`
  - L229: `if !std::fs::symlink_metadata(&dirstate)`


## xai-fuzzy-file-search — CLEAN


## xai-gix-status — CLEAN


## xai-grok-active-sessions — RISK

### `crates\codegen\xai-grok-active-sessions\src\lib.rs`（5 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L215: `cwd: "/tmp/test".into(),`

### `crates\codegen\xai-grok-active-sessions\tests\smoke.rs`（1 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L12: `cwd: "/tmp/test".into(),`


## xai-grok-agent — RISK

### `crates\codegen\xai-grok-agent\src\discovery.rs`（42 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L674: `let root = PathBuf::from(format!("/tmp/{plugin_name}"));`
  - L736: `let home = Path::new("/home/u");`
  - L751: `let home = Path::new("/home/u");`

### `crates\codegen\xai-grok-agent\src\plugins\discovery.rs`（29 tests, unix 自门控命中 0）
- **symlink** ×2
  - L131: `/// Canonical (symlink-resolved) root path.`
  - L480: `.filter(|e| e.path().is_dir()) // follows symlinks`
- **posix-path** ×2
  - L866: `let home = Path::new("/home/u");`
  - L1355: `Path::new("/home/user/.grok/plugins/my-plugin"),`

### `crates\codegen\xai-grok-agent\src\plugins\git_install.rs`（33 tests, unix 自门控命中 0）
- **symlink** ×8（共 8 处，仅列 5）
  - L1: `//! Installs plugin sources: git clones and local-directory copies land in the managed `installed-plugins` snapshot (not live symlinks).`
  - L227: `/// Install a plugin source (clone or symlink) and discover plugins.`
  - L286: `// Deliberate full copy (not a symlink): isolates the install from later source edits/deletion and keeps uninstall a simple dir remove`
  - L486: `/// Remove a repo path (handles both symlinks and directories).`
  - L662: `/// Session spawn / reload uses [`super::local_refresh`] to re-copy trusted sources (install is a full directory copy, not a live symlink).`
- **posix-path** ×5
  - L976: `let source = parse_install_source("/home/user/my-plugin", Path::new("/tmp"));`
  - L979: `assert_eq!(path, PathBuf::from("/home/user/my-plugin"));`
  - L1000: `let source = parse_install_source("/home/user/workspace#my-plugin", Path::new("/tmp"));`
  - L1003: `assert_eq!(path, PathBuf::from("/home/user/workspace"));`
  - L1335: `"/tmp/repo.git",`

### `crates\codegen\xai-grok-agent\src\plugins\hooks_adapter.rs`（13 tests, unix 自门控命中 0）
- **posix-path** ×5
  - L360: `"/opt/plugins/gb1183",`
  - L361: `"/var/plugins/gb1183",`
  - L371: `assert!(commands.contains(&"/opt/plugins/gb1183/hooks/pre.sh".to_string()));`
  - L372: `assert!(commands.contains(&"/opt/plugins/gb1183/hooks/alias.sh".to_string()));`
  - L373: `assert!(commands.contains(&"/var/plugins/gb1183/cache/post.sh".to_string()));`

### `crates\codegen\xai-grok-agent\src\plugins\install_registry.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L487: `source_path: PathBuf::from("/home/user/plugins/linter"),`
  - L638: `let json = r#"{"type":"Local","source_path":"/home/user/plugin"}"#;`
  - L645: `assert_eq!(source_path, PathBuf::from("/home/user/plugin"));`
  - L655: `source_path: PathBuf::from("/home/user/workspace"),`
- **symlink** ×2
  - L1: `//! Tracks which repos have been cloned/symlinked into the managed install directory, along with the plugins discovered within each repo.`
  - L51: `/// Absolute path to the repo directory (or symlink) in the install dir.`

### `crates\codegen\xai-grok-agent\src\plugins\local_refresh.rs`（11 tests, unix 自门控命中 1）
- **symlink** ×3
  - L1: `//! A local install is a full directory copy under `installed-plugins/`, not a live symlink.`
  - L147: `/// The set of `(relative_path, file_len)` for every non-symlink file under a tree.`
  - L154: `let meta = std::fs::symlink_metadata(&path)?;`
- **env-home** ×1
  - L375: `let guard = EnvVarGuard::set("HOME", &home);`

### `crates\codegen\xai-grok-agent\src\plugins\manifest.rs`（24 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L474: `name_from_dirname(Path::new("/home/user/my-plugin")),`
  - L568: `let result = substitute_env_vars(input, "/home/user/plugin", "/home/user/.data/plugin");`
  - L571: `"/home/user/plugin/bin:/home/user/plugin/lib:/home/user/.data/plugin/cache"`
- **symlink** ×1
  - L70: `/// Canonicalizes both sides (resolving symlinks and `..`) before the prefix check.`

### `crates\codegen\xai-grok-agent\src\plugins\registry.rs`（33 tests, unix 自门控命中 0）
- **symlink** ×2
  - L19: `/// Canonical (symlink-resolved) root path.`
  - L351: `/// The snapshot is a copy, not a symlink, so spawn refresh picks up agents/skills added after install.`
- **posix-path** ×1
  - L577: `let root = PathBuf::from(format!("/tmp/test-plugins/{name}"));`

### `crates\codegen\xai-grok-agent\src\plugins\trust.rs`（7 tests, unix 自门控命中 0）
- **symlink** ×1
  - L54: `/// Returns `false` if canonicalization fails (broken symlink, permission error).`

### `crates\codegen\xai-grok-agent\src\prompt\context.rs`（45 tests, unix 自门控命中 0）
- **posix-path** ×13（共 13 处，仅列 5）
  - L399: `"shell_path": "/bin/bash",`
  - L417: `"shell_path": "/bin/bash",`
  - L452: `ctx.shell_path = Some("/bin/bash".into());`
  - L461: `assert_eq!(p["shell_path"], "/bin/bash");`
  - L482: `ctx.shell_path = Some("/bin/bash".into());`

### `crates\codegen\xai-grok-agent\src\prompt\skills.rs`（89 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L1495: `let root = PathBuf::from("/tmp/plugin-dev");`
  - L1532: `"/tmp/plugin-dev/skills/indexer/SKILL.md",`
  - L2321: `let root = PathBuf::from(format!("/tmp/{name}"));`

### `crates\codegen\xai-grok-agent\src\prompt\template.rs`（31 tests, unix 自门控命中 0）
- **posix-path** ×6（共 6 处，仅列 5）
  - L144: `"shell_path": "/bin/zsh",`
  - L145: `"working_directory": "/tmp/test",`
  - L463: `placeholders["memory_global_path"] = serde_json::json!("/home/test/.grok/memory-v2/global");`
  - L465: `serde_json::json!("/home/test/.grok/memory-v2/workspaces/project");`
  - L469: `assert!(prompt.contains("/home/test/.grok/memory-v2/global"));`

### `crates\codegen\xai-grok-agent\src\prompt\user_message.rs`（10 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L458: `path: "/home/dev/.grok/AGENTS.md".into(),`
  - L483: `path: "/home/dev/.grok/AGENTS.md".into(),`
  - L502: `path: "/home/dev/.grok/rules/personal.md".into(),`

### `crates\codegen\xai-grok-agent\src\repo.rs`（4 tests, unix 自门控命中 0）
- **symlink** ×4
  - L29: `// Home is compared canonically to match the symlink handling below.`
  - L34: `// Canonicalize only for the stop test so a symlinked cwd still halts at the worktree root.`
  - L35: `// Pushed dirs keep their original spelling. Do not reduce this to `starts_with`: a mid-chain absolute symlink would walk past the root.`
  - L181: `// Compare by canonical form so a `/tmp` to `/private/tmp` symlink doesn't fail the test`
- **env-home** ×2
  - L213: `let _home_guard = EnvVarGuard::set("HOME", &home);`
  - L229: `let _home_guard = EnvVarGuard::set("HOME", home.path());`


## xai-grok-announcements — CLEAN


## xai-grok-auth — CLEAN


## xai-grok-bundle — RISK

### `crates\codegen\xai-grok-bundle\src\lib.rs`（39 tests, unix 自门控命中 0）
- **perm-mode** ×8（共 8 处，仅列 5）
  - L517: `header.set_mode(0o644);`
  - L1298: `header.set_mode(0o644);`
  - L1309: `h.set_mode(0o644);`
  - L1332: `h.set_mode(0o644);`
  - L1340: `dir_h.set_mode(0o755);`


## xai-grok-compaction — CLEAN


## xai-grok-config — RISK

### `crates\codegen\xai-grok-config\src\config_override.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L245: `set = { LD_PRELOAD = \"/tmp/evil.so\" }\n\`
  - L372: `mtls_cert_dir = \"/tmp/replacement\"\n\`
  - L377: `mtls_cert_dir = \"/tmp/injected\"\n",`

### `crates\codegen\xai-grok-config\src\env_overlay_tests.rs`（11 tests, unix 自门控命中 0）
- **posix-path** ×5
  - L86: `"paths": {"extra_skill_dirs": ["/tmp/evil"]},`
  - L94: `"plugins": {"paths": ["/tmp/evil"]},`
  - L96: `"shell_environment_policy": {"set": {"LD_PRELOAD": "/tmp/evil.so"}},`
  - L141: `"set": {"LD_PRELOAD": "/tmp/evil.so", "PATH": "/tmp/evil"}`
  - L166: `"plugins": {"paths": ["/tmp/evil"]}`

### `crates\codegen\xai-grok-config\src\loader.rs`（13 tests, unix 自门控命中 5）
- **symlink** ×1
  - L317: `/// Warn when a policy-tier hooks file is a symlink or not root-owned; the no-disable exemption assumes admin ownership of the system dir.`

### `crates\codegen\xai-grok-config\src\managed_text\tests.rs`（23 tests, unix 自门控命中 16）
- **posix-path** ×2
  - L580: `program: "/bin/sh".into(),`
  - L649: `program: "/bin/sh".into(),`

### `crates\codegen\xai-grok-config\src\paths.rs`（15 tests, unix 自门控命中 23）
- **posix-path** ×1
  - L25: `Some(PathBuf::from("/etc/grok"))`

### `crates\codegen\xai-grok-config\src\signed_policy\tests.rs`（37 tests, unix 自门控命中 12）
- **symlink** ×2
  - L373: `/// A symlink at an artifact slot is tamper, never the lenient Unreadable: the check does not follow links.`
  - L414: `/// A symlink at the SIDECAR slot reads NoAuthenticSidecar, which refuses under a fail-closed marker, never the lenient SidecarUnreadable.`


## xai-grok-config-types — CLEAN


## xai-grok-dashboard-store — CLEAN


## xai-grok-diag-server — CLEAN


## xai-grok-env — CLEAN


## xai-grok-extra-ca — CLEAN


## xai-grok-feedback — RISK

### `crates\codegen\xai-grok-feedback\src\draft_store_tests.rs`（16 tests, unix 自门控命中 1）
- **symlink** ×1
  - L241: `fn symlink_and_non_file_paths_return_typed_errors() {`


## xai-grok-foreign-sessions — RISK

### `crates\codegen\xai-grok-foreign-sessions\src\claude\windows_tests.rs`（4 tests, unix 自门控命中 0）
- **symlink** ×1
  - L108: `if std::os::windows::fs::symlink_dir(&outside, &fixture.project).is_err() {`

### `crates\codegen\xai-grok-foreign-sessions\src\codex\mod.rs`（1 tests, unix 自门控命中 0）
- **symlink** ×1
  - L78: `match std::fs::symlink_metadata(&path) {`

### `crates\codegen\xai-grok-foreign-sessions\src\lib.rs`（14 tests, unix 自门控命中 1）
- **symlink** ×1
  - L69: `match std::fs::symlink_metadata(path) {`


## xai-grok-gboom — CLEAN


## xai-grok-hooks — RISK

### `crates\codegen\xai-grok-hooks\src\config.rs`（38 tests, unix 自门控命中 0）
- **posix-path** ×34（共 34 处，仅列 5）
  - L905: `let (specs, errors) = parse_hook_file(json, Path::new("/tmp/hooks/test.json"));`
  - L931: `let (specs, errors) = parse_hook_file(json, Path::new("/tmp/test.json"));`
  - L947: `let (specs, errors) = parse_hook_file(json, Path::new("/tmp/test.json"));`
  - L961: `let (specs, errors) = parse_hook_file(json, Path::new("/tmp/test.json"));`
  - L988: `let (specs, errors) = parse_hook_file(json, Path::new("/tmp/test.json"));`
- **sh-exec** ×1
  - L214: `/// Command path, env-expanded; unresolved/modifier forms kept for the runner's `sh -c` branch.`

### `crates\codegen\xai-grok-hooks\src\env_expand.rs`（42 tests, unix 自门控命中 0）
- **sh-exec** ×5
  - L13: `//! The user wrote the modifier form because they wanted the shell's interpretation, so the runtime `sh -c` branch resolves it.`
  - L25: `//! Unix `sh -c` expands `$VAR` from the child env; Windows PowerShell rewrites known `$VAR` to `$env:VAR`.`
  - L139: `/// Unterminated braced forms (`${VAR:-no-close`) are skipped: the `$` is consumed and scanning continues at the next byte. This matches `shellexpand`, which tr`
  - L426: `/// The runtime `sh -c` branch applies POSIX `:-` semantics: bash returns the default for an empty value, shellexpand the empty string.`
  - L509: `/// The literal `${B}` survives inside the masked body for the runtime `sh -c` branch, which handles nesting natively.`
- **posix-path** ×5
  - L241: `extra.insert("ROOT".to_string(), "/opt/plugin".to_string());`
  - L243: `assert_eq!(out, "/opt/plugin/bin/x.sh");`
  - L301: `let already = "/opt/plugins/foo/hooks/x.sh";`
  - L408: `extra.insert("GROK_HOOKS_PLAIN".to_string(), "/usr/local".to_string());`
  - L411: `assert_eq!(out, "/usr/local/${GROK_HOOKS_DEFER:-/fallback}");`

### `crates\codegen\xai-grok-hooks\src\event.rs`（10 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L833: `transcript_path: Some("/tmp/transcript.jsonl".into()),`

### `crates\codegen\xai-grok-hooks\src\runner\command.rs`（58 tests, unix 自门控命中 16）
- **posix-path** ×2
  - L1951: `Some("/usr/bin/hook"),`
  - L1954: `Some(std::path::PathBuf::from("/usr/bin/hook"))`

### `crates\codegen\xai-grok-hooks\tests\integration.rs`（11 tests, unix 自门控命中 3）
- **sh-exec** ×1
  - L1: `//! Hooks use inline shell command strings routed via `sh -c` rather than standalone scripts.`


## xai-grok-http — CLEAN


## xai-grok-image — CLEAN


## xai-grok-login — RISK

### `crates\codegen\xai-grok-login\src\auth_provider_tests.rs`（28 tests, unix 自门控命中 3）
- **sh-exec** ×1
  - L765: `let mut cmd = tokio::process::Command::new("sh");`
- **posix-path** ×1
  - L791: `"/usr/local/bin/helper"`

### `crates\codegen\xai-grok-login\src\external_auth.rs`（8 tests, unix 自门控命中 0）
- **sh-exec** ×1
  - L196: `// `sh -c` on a missing path is a fast non-zero exit, not a hang, so it`

### `crates\codegen\xai-grok-login\src\flow.rs`（41 tests, unix 自门控命中 2）
- **sh-exec** ×1
  - L1818: `let cmd = r#"sh -c 'i=0; while [ $i -lt 2000 ]; do printf "%s" "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" >&2; i=$((i+1)); done; printf token'"#;`

### `crates\codegen\xai-grok-login\src\pre_tui.rs`（6 tests, unix 自门控命中 0）
- **sh-exec** ×1
  - L174: `let cmd = r#"sh -c 'i=0; while [ $i -lt 2000 ]; do printf "%s" "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx" >&2; i=$((i+1)); done; printf token'"#;`

### `crates\codegen\xai-grok-login\src\storage.rs`（13 tests, unix 自门控命中 16）
- **posix-path** ×5
  - L391: `Path::new("/home/u/.grok")`
  - L400: `resolve_auth_json_path(None, Path::new("/home/u/.grok")),`
  - L401: `PathBuf::from("/home/u/.grok/auth.json"),`
  - L408: `resolve_auth_json_path(Some(OsString::new()), Path::new("/home/u/.grok")),`
  - L409: `PathBuf::from("/home/u/.grok/auth.json"),`
- **perm-mode** ×1
  - L210: `/// Serialize `auth_store` to `path` (truncate and rewrite), owner-only (0o600) and `fsync`'d.`


## xai-grok-markdown — RISK

### `crates\codegen\xai-grok-markdown\src\colors.rs`（7 tests, unix 自门控命中 0）
- **term-detect** ×1
  - L44: `/// `COLORTERM` environment variable (for truecolor detection); `TERM` environment variable; Terminal-specific environment variables (like `ITERM_SESSION_ID`); `

### `crates\codegen\xai-grok-markdown\src\latex_delimiters.rs`（36 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L1123: `pre_finish.contains('`') && pre_finish.contains("/tmp/project/results"),`


## xai-grok-markdown-core — CLEAN


## xai-grok-mcp — RISK

### `crates\codegen\xai-grok-mcp\src\credentials.rs`（10 tests, unix 自门控命中 13）
- **perm-mode** ×1
  - L17: `/// Ensure credential paths are owner-only (Unix `0o600`).`

### `crates\codegen\xai-grok-mcp\src\servers_tests.rs`（182 tests, unix 自门控命中 5）
- **posix-path** ×74（共 74 处，仅列 5）
  - L315: `let configs = vec![make_stdio_server("test", "/bin/test")];`
  - L347: `let configs = vec![make_stdio_server("test", "/bin/test")];`
  - L361: `let configs = vec![make_stdio_server("test", "/bin/test")];`
  - L365: `let new_configs = vec![make_stdio_server("test2", "/bin/test2")];`
  - L377: `let configs = vec![make_stdio_server("test", "/bin/test")];`


## xai-grok-memory — RISK

### `crates\codegen\xai-grok-memory\src\archive.rs`（6 tests, unix 自门控命中 7）
- **symlink** ×4
  - L12: `/// planted `/dev/zero` symlink) must not balloon the process.`
  - L15: `/// Open `path` without following a final-component symlink, without blocking on a FIFO, and only if it is a regular file. The memory dir is writable to sandbox`
  - L31: `/// Snapshot `path`'s bytes and append them as `name`: the tar header size must consolidation, `/flush`) resizes the live file mid-build. Skips (never fails the`
  - L164: `/// A planted symlink must not smuggle its target into the archive: the`
- **perm-mode** ×1
  - L75: `header.set_mode(0o644);`

### `crates\codegen\xai-grok-memory\src\backend.rs`（29 tests, unix 自门控命中 0）
- **symlink** ×1
  - L1025: `// On macOS, TempDir paths may live under /private/tmp (via a symlink from /tmp)`

### `crates\codegen\xai-grok-memory\src\storage.rs`（82 tests, unix 自门控命中 0）
- **posix-path** ×18（共 18 处，仅列 5）
  - L702: `|| raw_s.starts_with("/tmp/")`
  - L703: `|| raw_s.starts_with("/var/tmp/")`
  - L704: `|| (raw_s.contains("/var/folders/") && raw_s.contains("/T/"))`
  - L1537: `assert!(is_ephemeral_cwd(Path::new("/tmp/foo")));`
  - L1538: `assert!(is_ephemeral_cwd(Path::new("/tmp/subagent-worktree-123")));`
- **symlink** ×1
  - L362: `// the configured v2 root; otherwise a replaced scope symlink could admit files outside memory.`

### `crates\codegen\xai-grok-memory\src\storage_v2_tests.rs`（8 tests, unix 自门控命中 7）
- **posix-path** ×1
  - L11: `Path::new("/home/user/project"),`

### `crates\codegen\xai-grok-memory\src\v2_capture_tests.rs`（25 tests, unix 自门控命中 2）
- **posix-path** ×1
  - L939: `Path::new("/home/test/project"),`


## xai-grok-mermaid — RISK

### `crates\codegen\xai-grok-mermaid\src\subprocess.rs`（7 tests, unix 自门控命中 2）
- **signal** ×2
  - L120: `/// Best-effort teardown of a spawned child: SIGKILL its process group (to reach any grandchildren), then kill and reap the child.`
  - L127: `/// SIGKILL the child's process group so grandchildren are reaped, not just the direct child.`


## xai-grok-otel — RISK

### `crates\codegen\xai-grok-otel\src\config.rs`（2 tests, unix 自门控命中 0）
- **term-detect** ×1
  - L43: `.or_else(|| std::env::var("TERM").ok())`

### `crates\codegen\xai-grok-otel\src\redact.rs`（11 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L316: `KeyValue::new("path", "/tmp/x.rs"),`


## xai-grok-pager — RISK

feature 门控测试（默认构建不跑，抽样不含）：local-workspace, release-dist

### `crates\codegen\xai-grok-pager\src\app\acp_handler\tests\background_tasks.rs`（27 tests, unix 自门控命中 0）
- **posix-path** ×5
  - L18: `output_file: "/tmp/mon-1.log".into(),`
  - L164: `output_file: "/tmp/output.log".into(),`
  - L216: `output_file: "/tmp/output.log".into(),`
  - L634: `output_file: "/tmp/out.log".into(),`
  - L702: `output_file: "/tmp/output.log".into(),`
- **signal** ×1
  - L373: `Some("SIGKILL"),`

### `crates\codegen\xai-grok-pager\src\app\acp_handler\tests\interactions.rs`（24 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L891: `notify_status(&mut app, "/tmp/first"),`
  - L902: `notify_status(&mut app, "/tmp/second");`

### `crates\codegen\xai-grok-pager\src\app\acp_handler\tests\mod.rs`（2 tests, unix 自门控命中 0）
- **posix-path** ×5
  - L238: `output_file: "/tmp/out".into(),`
  - L1741: `r#"{{"method":"session/update","params":{{"sessionId":"{child_sid}","update":{{"sessionUpdate":"tool_call","toolCallId":"tc1","title":"Read foo","kind":"read","`
  - L1941: `output_file: "/tmp/output.log".into(),`
  - L1965: `output_file: "/tmp/output.log".into(),`
  - L2087: `output_file: "/tmp/out.log".into(),`

### `crates\codegen\xai-grok-pager\src\app\acp_handler\tests\permissions.rs`（19 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L31: `Some(serde_json::json!({"file_path": "/tmp/x"})),`
  - L133: `serde_json::json!({"file_path": "/tmp/x.rs", "old_string": "a", "new_string": "b"}),`
  - L140: `serde_json::json!({"target_file": "/tmp/x.rs"}),`
  - L285: `output_file: "/tmp/out".into(),`

### `crates\codegen\xai-grok-pager\src\app\acp_handler\tests\session_events.rs`（54 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L1446: `command: Some("/bin/true".to_string()),`

### `crates\codegen\xai-grok-pager\src\app\acp_handler\tests\session_routing.rs`（9 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L237: `output_file: "/tmp/out".into(),`

### `crates\codegen\xai-grok-pager\src\app\agent.rs`（43 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L1785: `output_file: "/tmp/out.log".into(),`

### `crates\codegen\xai-grok-pager\src\app\agent_view\dock_input_tests.rs`（56 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L55: `output_file: "/tmp/out".into(),`
  - L367: `output_file: "/tmp/monitor-out".into(),`

### `crates\codegen\xai-grok-pager\src\app\agent_view\interactions.rs`（40 tests, unix 自门控命中 0）
- **term-detect** ×1
  - L2029: `agent.last_terminal_size = (80, 30);`

### `crates\codegen\xai-grok-pager\src\app\agent_view\links.rs`（96 tests, unix 自门控命中 3）
- **term-detect** ×17（共 17 处，仅列 5）
  - L688: `agent.last_terminal_size = (80, 30);`
  - L729: `agent.last_terminal_size = (80, 30);`
  - L808: `agent.last_terminal_size = (80, 30);`
  - L863: `agent.last_terminal_size = (80, 30);`
  - L898: `agent.last_terminal_size = (80, 30);`

### `crates\codegen\xai-grok-pager\src\app\agent_view\media.rs`（5 tests, unix 自门控命中 1）
- **posix-path** ×1
  - L664: `path: std::path::PathBuf::from("/tmp/clip.mp4"),`

### `crates\codegen\xai-grok-pager\src\app\agent_view\mod.rs`（44 tests, unix 自门控命中 0）
- **term-detect** ×4
  - L1165: `/// This is the size of the rect this view last painted into, which can be smaller than the terminal (dashboard overlay header band/popup, dev tracing split). O`
  - L1166: `pub(crate) last_terminal_size: (u16, u16),`
  - L1170: `/// Set on every `Event::Resize` (see `AppView::handle_input`), cleared by the next draw's re-measure. While set, `last_terminal_size` is known-invalidated and `
  - L1171: `pub(crate) terminal_size_stale: bool,`

### `crates\codegen\xai-grok-pager\src\app\agent_view\modals.rs`（54 tests, unix 自门控命中 0）
- **posix-path** ×18（共 18 处，仅列 5）
  - L2575: `root: "/tmp/p".into(),`
  - L2958: `hooks: vec![hook_info("src/hook-a", "/tmp/hooks", false)],`
  - L3386: `hook_info("src/hook-a", "/tmp/hooks", true),`
  - L3387: `hook_info("src/hook-b", "/tmp/hooks", false),`
  - L3405: `let mut pinned = hook_info("policy/hook-a", "/etc/grok", false);`

### `crates\codegen\xai-grok-pager\src\app\agent_view\notices.rs`（5 tests, unix 自门控命中 0）
- **term-detect** ×9（共 9 处，仅列 5）
  - L33: `if !self.ephemeral_tip_renderable(self.last_terminal_size.1) {`
  - L42: `self.ephemeral_tip_renderable(self.last_terminal_size.1)`
  - L136: `!self.terminal_size_stale && crate::tips::tip_row_renderable(occluded, screen_height)`
  - L142: `pub(crate) fn note_terminal_size(&mut self, size: (u16, u16)) {`
  - L143: `if self.last_terminal_size != (0, 0) && self.last_terminal_size != size {`

### `crates\codegen\xai-grok-pager\src\app\agent_view\paste.rs`（81 tests, unix 自门控命中 5）
- **term-detect** ×14（共 14 处，仅列 5）
  - L1509: `agent.last_terminal_size = (80, 16);`
  - L1511: `agent.last_terminal_size = (80, 30);`
  - L1583: `agent.note_terminal_size((80, 30));`
  - L1589: `agent.last_terminal_size,`
  - L1593: `agent.note_terminal_size((80, 30));`
- **posix-path** ×6（共 6 处，仅列 5）
  - L819: `let outcome = paste_cmd_v(&mut agent, Some("/tmp/not-an-image.txt"));`
  - L822: `assert_eq!(agent.prompt.text(), "/tmp/not-an-image.txt");`
  - L1939: `let path = std::path::PathBuf::from("/tmp/tool-media.png");`
  - L2199: `path: std::path::PathBuf::from("/tmp/clip.mp4"),`
  - L2232: `.insert(std::path::PathBuf::from("/tmp/a.png"), 2);`

### `crates\codegen\xai-grok-pager\src\app\agent_view\queue.rs`（46 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L1900: `output_file: "/tmp/out".into(),`

### `crates\codegen\xai-grok-pager\src\app\agent_view\render.rs`（22 tests, unix 自门控命中 0）
- **term-detect** ×1
  - L861: `self.note_terminal_size((area.width, area.height));`

### `crates\codegen\xai-grok-pager\src\app\agent_view\session.rs`（37 tests, unix 自门控命中 0, feature 门控命中 2）

### `crates\codegen\xai-grok-pager\src\app\agent_view\task_icon_mouse_tests.rs`（4 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L80: `output_file: "/tmp/out".into(),`

### `crates\codegen\xai-grok-pager\src\app\agent_view\task_status_tests.rs`（1 tests, unix 自门控命中 0）
- **term-detect** ×1
  - L74: `agent.last_terminal_size = (80, 30);`

### `crates\codegen\xai-grok-pager\src\app\agent_view\viewer_tests.rs`（28 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L927: `output_file: "/tmp/out".into(),`

### `crates\codegen\xai-grok-pager\src\app\app_view_tests.rs`（270 tests, unix 自门控命中 0）
- **term-detect** ×11（共 11 处，仅列 5）
  - L445: `app.agents.get_mut(&id).unwrap().last_terminal_size = (80, 30);`
  - L459: `app.agents.get_mut(&id).unwrap().last_terminal_size = (80, 30);`
  - L497: `app.agents.get_mut(&id).unwrap().last_terminal_size = (80, 30);`
  - L517: `app.agents.get_mut(&id).unwrap().last_terminal_size = (80, 10);`
  - L974: `agent.last_terminal_size = (80, 30);`
- **posix-path** ×10（共 10 处，仅列 5）
  - L2168: `cwd: "/tmp/repo".into(),`
  - L2258: `workspace: std::path::PathBuf::from("/tmp/x"),`
  - L2268: `workspace: std::path::PathBuf::from("/tmp/x"),`
  - L2279: `workspace: std::path::PathBuf::from("/tmp/x"),`
  - L4568: `.insert(std::path::PathBuf::from("/tmp/media.png"), 5);`

### `crates\codegen\xai-grok-pager\src\app\cli.rs`（28 tests, unix 自门控命中 0）
- **posix-path** ×8（共 8 处，仅列 5）
  - L1293: `let args = PagerArgs::try_parse_from(["grok", "--leader-socket", "/tmp/leader-x.sock"])`
  - L1297: `Some(std::path::Path::new("/tmp/leader-x.sock"))`
  - L1307: `"/tmp/leader-y.sock",`
  - L1312: `Some(std::path::Path::new("/tmp/leader-y.sock"))`
  - L1352: `let root = PagerArgs::try_parse_from(["grok", "--debug-file", "/tmp/fire.txt"])`

### `crates\codegen\xai-grok-pager\src\app\dispatch\queue.rs`（73 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L1251: `output_file: "/tmp/out".into(),`

### `crates\codegen\xai-grok-pager\src\app\dispatch\tests\dashboard.rs`（247 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L172: `app.cwd = "/tmp/process-cwd".into();`
  - L175: `dashboard.cwd = "/tmp/dashboard-cwd".into();`
  - L185: `repo_name_from_cwd("/tmp/dashboard-cwd")`
  - L4401: `r#"{{"method":"session/update","params":{{"sessionId":"{child_sid}","update":{{"sessionUpdate":"tool_call","toolCallId":"tc1","title":"Read foo","kind":"read","`

### `crates\codegen\xai-grok-pager\src\app\dispatch\tests\modes.rs`（63 tests, unix 自门控命中 0）
- **term-detect** ×9（共 9 处，仅列 5）
  - L11: `app.agents.get_mut(&id).unwrap().last_terminal_size = (80, 30);`
  - L27: `app.agents.get_mut(&id).unwrap().last_terminal_size = (80, 30);`
  - L43: `app.agents.get_mut(&id).unwrap().last_terminal_size = (80, 30);`
  - L61: `app.agents.get_mut(&id).unwrap().last_terminal_size = (80, 30);`
  - L80: `app.agents.get_mut(&id).unwrap().last_terminal_size = (80, 30);`

### `crates\codegen\xai-grok-pager\src\app\dispatch\tests\prompt.rs`（151 tests, unix 自门控命中 0）
- **term-detect** ×24（共 24 处，仅列 5）
  - L372: `app.agents.get_mut(&id).unwrap().last_terminal_size = (80, 30);`
  - L388: `app.agents.get_mut(&id).unwrap().last_terminal_size = (80, 30);`
  - L407: `app.agents.get_mut(&id).unwrap().last_terminal_size = (100, 24);`
  - L419: `app.agents.get_mut(&id).unwrap().last_terminal_size = (100, 24);`
  - L447: `agent.last_terminal_size = (100, 24);`
- **posix-path** ×1
  - L817: `"path": "/tmp/skills/pr-workflow/SKILL.md",`

### `crates\codegen\xai-grok-pager\src\app\dispatch\tests\rewind.rs`（38 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L1101: `let mut app = app_mid_inline_edit("/etc/hosts is wrong, fix it");`
  - L1115: `|e| matches!(e, Effect::SendPrompt { text, .. } if text == "/etc/hosts is wrong, fix it")`

### `crates\codegen\xai-grok-pager\src\app\dispatch\tests\router.rs`（105 tests, unix 自门控命中 0, feature 门控命中 1）
- **posix-path** ×4
  - L306: `let path = std::path::PathBuf::from("/tmp/agent-config.md");`
  - L1410: `"path": "/home/user/.grok/skills/pick-best/SKILL.md",`
  - L1760: `"/tmp/chat-mode-build-refuse-{}",`
  - L1782: `let cwd = PathBuf::from(format!("/tmp/chat-mode-conv-ok-{}", std::process::id()));`

### `crates\codegen\xai-grok-pager\src\app\dispatch\tests\session\fork.rs`（60 tests, unix 自门控命中 0）
- **posix-path** ×18（共 18 处，仅列 5）
  - L31: `let worktree_path = PathBuf::from("/tmp/grok-worktrees/pager-fork");`
  - L32: `let session_cwd = PathBuf::from("/tmp/grok-worktrees/pager-fork/sub");`
  - L110: `let worktree_path = PathBuf::from("/tmp/grok-worktrees/pager-fork-sticky");`
  - L151: `let worktree_path = PathBuf::from("/tmp/grok-worktrees/pager-fork");`
  - L152: `let session_cwd = PathBuf::from("/tmp/grok-worktrees/pager-fork/sub");`

### `crates\codegen\xai-grok-pager\src\app\dispatch\tests\session\lifecycle.rs`（120 tests, unix 自门控命中 0）
- **posix-path** ×7（共 7 处，仅列 5）
  - L292: `let worktree_path = PathBuf::from("/tmp/grok-worktrees/pager-123");`
  - L355: `let worktree_path = PathBuf::from("/tmp/grok-worktrees/pager-sticky");`
  - L392: `let worktree_root = PathBuf::from("/home/user/.grok/worktrees/repo/pager-123");`
  - L522: `let worktree_path = PathBuf::from("/tmp/grok-worktrees/pager-abc");`
  - L528: `session_cwd: PathBuf::from("/tmp/grok-worktrees/pager-abc"),`

### `crates\codegen\xai-grok-pager\src\app\dispatch\tests\session\load.rs`（89 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L2363: `sessions: vec![make_picker_entry("stale-generation", "/tmp/repo")],`
  - L2391: `sessions: vec![make_picker_entry("stale-policy", "/tmp/repo")],`
  - L2420: `sessions: vec![make_picker_entry("current", "/tmp/repo")],`

### `crates\codegen\xai-grok-pager\src\app\dispatch\tests\status.rs`（57 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L1437: `cwd: "/tmp/test".to_string(),`

### `crates\codegen\xai-grok-pager\src\app\dispatch\tests\status_line.rs`（20 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L199: `let mut snapshot = test_context("/tmp/project");`
  - L471: `agent.status_context = Some(test_context("/tmp/second"));`

### `crates\codegen\xai-grok-pager\src\app\dispatch\tests\task_result.rs`（90 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L1635: `worktree_path: std::path::PathBuf::from("/tmp/wt"),`
  - L1636: `session_cwd: std::path::PathBuf::from("/tmp/wt"),`
  - L1643: `worktree_path: std::path::PathBuf::from("/tmp/wt"),`
  - L1644: `session_cwd: std::path::PathBuf::from("/tmp/wt"),`

### `crates\codegen\xai-grok-pager\src\app\edit_highlight_worker.rs`（9 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L518: `PathBuf::from("/tmp/edit-hl-test"),`
  - L569: `PathBuf::from("/tmp/edit-hl-test"),`

### `crates\codegen\xai-grok-pager\src\app\effects\tests.rs`（128 tests, unix 自门控命中 1, feature 门控命中 6）
- **posix-path** ×2
  - L792: `cwd: "/tmp/test".into(),`
  - L2423: `cwd: "/tmp/test".into(),`

### `crates\codegen\xai-grok-pager\src\app\event_loop.rs`（104 tests, unix 自门控命中 0）
- **term-detect** ×1
  - L495: `let _ = crossterm::terminal::enable_raw_mode();`
- **signal** ×1
  - L2595: `// Kept high in the biased order so a SIGTERM quit isn't starved by an ACP firehose`

### `crates\codegen\xai-grok-pager\src\app\mermaid_worker.rs`（36 tests, unix 自门控命中 21）
- **symlink** ×5
  - L252: `/// Refuses a symlinked final component (a model-predictable per-session path shouldn't be followed through a symlink).`
  - L255: `let Ok(meta) = std::fs::symlink_metadata(path) else {`
  - L1405: `/// The disk-cache hit predicate: a real PNG is a hit; a missing, corrupt, or symlinked entry is a miss.`
  - L1406: `/// A miss re-renders, and a symlink planted at the model-predictable path is never followed.`
  - L1427: `// A symlink (even to a valid PNG) is refused`
- **posix-path** ×5
  - L1106: `tx.send(job("same", "/tmp/old.png")).unwrap();`
  - L1107: `tx.send(job("other", "/tmp/other.png")).unwrap();`
  - L1108: `tx.send(job("same", "/tmp/new.png")).unwrap();`
  - L1115: `PathBuf::from("/tmp/new.png")`
  - L1119: `PathBuf::from("/tmp/other.png")`
- **signal** ×3
  - L338: `// A signal-terminated child (`RLIMIT_AS` allocation abort, panic under `panic=abort`, SIGKILL) is distinct from the watchdog's self-destruct`
  - L373: `/// The watchdog only matters when the parent died abruptly (SIGKILL, `panic = "abort"`, quit) and so cannot kill the child itself.`
  - L446: `/// Ask the kernel to SIGKILL this child if its parent (the pager) dies, so an abruptly-killed parent doesn't strand the child.`
- **term-detect** ×2
  - L851: `representative_content_cols(self.last_terminal_size.0)`
  - L1934: `agent.last_terminal_size = (100, 40);`

### `crates\codegen\xai-grok-pager\src\app\mod.rs`（88 tests, unix 自门控命中 1, feature 门控命中 1）
- **term-detect** ×2
  - L1272: `/// `crossterm::enable_raw_mode()` sets flags on stdin only.`
  - L1462: `terminal::enable_raw_mode()?;`
- **signal** ×1
  - L2640: `/// SIGPIPE is SIG_IGN, so the write returns BrokenPipe instead of killing the process.`

### `crates\codegen\xai-grok-pager\src\app\mouse.rs`（13 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L1704: `output_file: "/tmp/out".into(),`

### `crates\codegen\xai-grok-pager\src\app\queue_edit.rs`（50 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L1054: `"path": "/tmp/skills/pr-workflow/SKILL.md",`

### `crates\codegen\xai-grok-pager\src\app\screen_mode_relaunch.rs`（25 tests, unix 自门控命中 1）
- **posix-path** ×6（共 6 处，仅列 5）
  - L563: `"/tmp/proj",`
  - L565: `"/tmp/leader.sock",`
  - L567: `"/tmp/debug.log",`
  - L580: `"/tmp/proj",`
  - L582: `"/tmp/leader.sock",`

### `crates\codegen\xai-grok-pager\src\app\session_startup.rs`（72 tests, unix 自门控命中 0, feature 门控命中 2）
- **symlink** ×1
  - L508: `/// Returns the canonical directory so callers stamp and persist what was actually checked (symlinks and `..` must not diverge from validation).`
- **posix-path** ×1
  - L1576: `let err = ensure_session_id_available("my-run-1", "/tmp/does-not-matter").unwrap_err();`

### `crates\codegen\xai-grok-pager\src\app\signal_handler.rs`（1 tests, unix 自门控命中 0）
- **signal** ×7（共 7 处，仅列 5）
  - L3: `//! Without these, signal-triggered termination (SIGINT, SIGTERM, SIGHUP from WSL/SSH disconnect) leaves the terminal in raw mode.`
  - L7: `//! SIGINT / SIGTERM / SIGHUP are handled in a tokio task.`
  - L12: `//! SIGPIPE is intentionally left alone.`
  - L16: `//! That re-introduces a prior SIGPIPE regression.`
  - L43: `/// Lets the signal handler route SIGINT/SIGTERM/SIGHUP into the same graceful quit as `/exit` instead of a hard exit.`

### `crates\codegen\xai-grok-pager\src\app\status_line\command_tests.rs`（10 tests, unix 自门控命中 0）
- **term-detect** ×1
  - L26: `async fn script_is_handed_the_payload_on_stdin_and_the_terminal_size() {`

### `crates\codegen\xai-grok-pager\src\app\subagent_format_tests.rs`（6 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L315: `r#"{"prompt":"do stuff","child_cwd":"/tmp/work","worktree_path":"/tmp/wt"}"#,`
  - L318: `child_cwd: Some("/tmp/work"),`
  - L319: `worktree: Some("/tmp/wt"),`
  - L347: `let cwd = std::path::Path::new("/home/user/project");`

### `crates\codegen\xai-grok-pager\src\app\subagent_tests.rs`（13 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L257: `r#"{{"method":"session/update","params":{{"sessionId":"{child_sid}","update":{{"sessionUpdate":"tool_call","toolCallId":"tc1","title":"Read foo","kind":"read","`
  - L298: `r#"{{"method":"session/update","params":{{"sessionId":"{child_sid}","update":{{"sessionUpdate":"tool_call","toolCallId":"tc1","title":"Read foo","kind":"read","`
  - L601: `r#"{{"method":"session/update","params":{{"sessionId":"{child_sid}","update":{{"sessionUpdate":"tool_call","toolCallId":"tc1","title":"Read foo","kind":"read","`

### `crates\codegen\xai-grok-pager\src\app\turn_completion\tests.rs`（40 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L1064: `output_file: "/tmp/out".into(),`

### `crates\codegen\xai-grok-pager\src\app\workspace_sync.rs`（5 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L225: `agent.session.cwd = "/tmp/workspace-sync".into();`
  - L239: `assert_eq!(member.metadata.cwd.as_deref(), Some("/tmp/workspace-sync"));`
  - L250: `agent.session.cwd = "/tmp/workspace-sync".into();`
  - L270: `dispatched.session.cwd = "/tmp/workspace-sync".into();`

### `crates\codegen\xai-grok-pager\src\best_effort_stderr.rs`（2 tests, unix 自门控命中 0）
- **signal** ×1
  - L35: `/// leaves behind. SIGPIPE is ignored in Rust binaries, so the write returns an error.`

### `crates\codegen\xai-grok-pager\src\diagnostics\fix_tests.rs`（32 tests, unix 自门控命中 26）
- **posix-path** ×11（共 11 处，仅列 5）
  - L298: `"/tmp/../escape",`
  - L299: `"/tmp/bad\nname",`
  - L315: `reload_instruction(Path::new("/tmp/a b/q'v.conf")),`
  - L316: `"Reload tmux with `tmux source-file '/tmp/a b/q'\\''v.conf'`, or restart the tmux server."`
  - L319: `reload_instruction(Path::new("/tmp/a`b.conf")),`
- **env-home** ×1
  - L304: `SafeAbsoluteDirectory::parse(PathBuf::from(value), "HOME"),`
- **perm-mode** ×1
  - L1197: `std::fs::set_permissions(&fish_grok, std::fs::Permissions::from_mode(0o755)).unwrap();`

### `crates\codegen\xai-grok-pager\src\diagnostics\mod.rs`（112 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L1126: `tmux_env: Some("/tmp/tmux-501/default,12345,0".to_owned()),`
  - L1139: `tmux_env: Some("/tmp/tmux-501/default,12345,0".to_owned()),`
  - L2735: `tmux_env: Some("/tmp/tmux-501/default,12345,0".to_owned()),`

### `crates\codegen\xai-grok-pager\src\disk_usage_cmd\tests.rs`（25 tests, unix 自门控命中 16）
- **posix-path** ×9（共 9 处，仅列 5）
  - L528: `grok_home: "/home/user/.grok".into(),`
  - L546: `grok_home: "/home/user/.grok".into(),`
  - L559: `registry_path: "/home/user/.grok/worktrees.db".into(),`
  - L563: `path: "/home/user/.grok/worktrees/xai/wt-1".into(),`
  - L578: `path: "/home/user/.grok/worktree_pool/inst/wt-2".into(),`
- **symlink** ×3
  - L476: `let db_bytes = physical_file_size(&std::fs::symlink_metadata(&db_path).unwrap());`
  - L756: `one: "1 top-level symlink to a directory is not followed, so its contents are missing from the total.",`
  - L757: `two: "2 top-level symlinks to directories are not followed, so their contents are missing from the total.",`

### `crates\codegen\xai-grok-pager\src\doctor_cmd\tests.rs`（17 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L47: `Some(std::path::PathBuf::from("/bin/bash")),`

### `crates\codegen\xai-grok-pager\src\fs_size_tests.rs`（6 tests, unix 自门控命中 6）
- **symlink** ×1
  - L49: `.map(|p| physical_file_size(&std::fs::symlink_metadata(p).unwrap()))`

### `crates\codegen\xai-grok-pager\src\headless\ext_protocol_tests.rs`（27 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L675: `"path": "/tmp/memory/sessions/log.md"`
  - L681: `assert_eq!(path.as_deref(), Some("/tmp/memory/sessions/log.md"));`

### `crates\codegen\xai-grok-pager\src\hyperlink_route.rs`（16 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L74: `tmux_env: Some("/tmp/tmux-501/default,12345,0".to_owned()),`
  - L87: `tmux_env: Some("/tmp/tmux-501/default,12345,0".to_owned()),`

### `crates\codegen\xai-grok-pager\src\inline_media_ffmpeg.rs`（7 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L163: `path: std::path::PathBuf::from("/tmp/x"),`

### `crates\codegen\xai-grok-pager\src\notifications\hooks.rs`（9 tests, unix 自门控命中 0）
- **sh-exec** ×2
  - L15: `let mut cmd = Command::new("sh");`
  - L132: `// LOCAL: 以下用例经 `sh -c` 执行 POSIX 命令（env/touch/printf 重定向）并断言其副作用，`

### `crates\codegen\xai-grok-pager\src\notifications\title.rs`（35 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L687: `cwd: Some("/home/user/my-project"),`
  - L833: `cwd: Some("/home/user/workspace"),`

### `crates\codegen\xai-grok-pager\src\plugin_cmd.rs`（9 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L1148: `path: "/tmp/my-marketplace".into(),`
  - L1176: `let found = find_removal_source(&sources, "/tmp/my-marketplace", Path::new("/")).unwrap();`
- **symlink** ×1
  - L587: `println!("{repo_key}: local symlink, already live");`

### `crates\codegen\xai-grok-pager\src\pty_wrap.rs`（5 tests, unix 自门控命中 4）
- **pty-fork** ×1
  - L45: `let pair = pty_system.openpty(PtySize {`
- **term-detect** ×1
  - L77: `crossterm::terminal::enable_raw_mode()?;`

### `crates\codegen\xai-grok-pager\src\scrollback\block.rs`（33 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L1129: `path: std::path::PathBuf::from("/tmp/img.png"),`
  - L1152: `assert_eq!(p.info.path, std::path::PathBuf::from("/tmp/img.png"));`
  - L1460: `let block = RenderBlock::list_dir_with_output("/tmp/proj", "a.txt\nb.txt");`
  - L1462: `assert!(text.contains("/tmp/proj"), "got: {text:?}");`

### `crates\codegen\xai-grok-pager\src\scrollback\blocks\bg_task.rs`（12 tests, unix 自门控命中 0）
- **signal** ×1
  - L159: `.is_some_and(|s| matches!(s, "killed" | "SIGTERM" | "SIGKILL" | "oom"));`

### `crates\codegen\xai-grok-pager\src\scrollback\blocks\tool\read.rs`（15 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L483: `let block = ReadToolCallBlock::new("/home/user/.grok/skills/deploy/SKILL.md");`
  - L655: `let block = ReadToolCallBlock::new("/home/user/.grok/skills/deploy/SKILL.md");`

### `crates\codegen\xai-grok-pager\src\scrollback\link_map.rs`（20 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L222: `"/tmp/non-display-target/file name.rs",`

### `crates\codegen\xai-grok-pager\src\scrollback\render_tests.rs`（73 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L2071: `ScrollbackEntry::new(RenderBlock::read("/tmp/verbgeo/a1.rs", None)),`
  - L2252: `ScrollbackEntry::new(RenderBlock::read("/tmp/verbind/b1.rs", None)),`
  - L2805: `("outside", "/opt/service/main.rs"),`

### `crates\codegen\xai-grok-pager\src\slash\acp_command.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L197: `"path": "/home/user/.grok/skills/commit/SKILL.md",`
  - L202: `path: "/home/user/.grok/skills/commit/SKILL.md".to_string(),`

### `crates\codegen\xai-grok-pager\src\slash\commands\export.rs`（3 tests, unix 自门控命中 0）
- **symlink** ×1
  - L102: `// Follow symlinks so symlinked directories get trailing `/`.`

### `crates\codegen\xai-grok-pager\src\slash\mod.rs`（85 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L3001: `"path": "/home/user/.grok/skills/skill-cmd/SKILL.md",`

### `crates\codegen\xai-grok-pager\src\views\agents_modal.rs`（40 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L2782: `source_path: Some("/home/user/.grok/bundled/personas/b.toml".into()),`

### `crates\codegen\xai-grok-pager\src\views\dashboard\chrome_tests.rs`（21 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L620: `Span::styled("/home/me/repo".to_string(), plain),`
  - L634: `vec![("main", true), (" ", false), ("/home/me/repo", true)]`

### `crates\codegen\xai-grok-pager\src\views\dashboard\render_tests.rs`（113 tests, unix 自门控命中 0）
- **posix-path** ×8（共 8 处，仅列 5）
  - L98: `cwd: Some("/tmp/saved".to_owned()),`
  - L221: `cwd: Some("/tmp/saved".to_owned()),`
  - L3132: `path: std::path::PathBuf::from("/home/me/frontend"),`
  - L3248: `path: std::path::PathBuf::from("/home/me/wt"),`
  - L3272: `path: std::path::PathBuf::from("/home/me/myproj"),`

### `crates\codegen\xai-grok-pager\src\views\dashboard\row.rs`（55 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L1961: `agent.session.cwd = PathBuf::from("/home/me/.grok/worktrees/x/location-picker");`
  - L1974: `agent.session.cwd = PathBuf::from("/home/me/wt/my-wt-dir");`
  - L1987: `agent.session.cwd = PathBuf::from("/home/me/xai/crates/foo");`
  - L1996: `agent.session.cwd = PathBuf::from("/home/me/projects/bar");`

### `crates\codegen\xai-grok-pager\src\views\dashboard\state_tests.rs`（252 tests, unix 自门控命中 2）
- **posix-path** ×18（共 18 处，仅列 5）
  - L165: `let p = Path::new("/var/tmp/x");`
  - L166: `assert_eq!(compact_cwd(p, Some("/Users/alice")), "/var/tmp/x");`
  - L5471: `location_candidate("/home/me/alpha", "alpha"),`
  - L5472: `location_candidate("/home/me/beta", "beta"),`
  - L5480: `location_candidate("/home/me/alpha", "alpha"),`
- **symlink** ×2
  - L5586: `/// A worktree directory that is itself a symlink still gets tagged: the index key is the canonical`
  - L5587: `/// (real) path, so `read_subdirs` must canonicalize the entry (resolving the symlink), not just`

### `crates\codegen\xai-grok-pager\src\views\extensions_modal.rs`（159 tests, unix 自门控命中 0）
- **posix-path** ×20（共 20 处，仅列 5）
  - L265: `root: format!("/tmp/{name}"),`
  - L5410: `let mut pinned = make_hook("policy/a", "/etc/grok", false);`
  - L5412: `let sibling = make_hook("user/b", "/etc/grok", false);`
  - L5413: `let elsewhere = make_hook("user/c", "/home/u/.grok", false);`
  - L5416: `assert!(hook_source_pinned(&hooks, "/etc/grok"));`

### `crates\codegen\xai-grok-pager\src\views\extensions_modal\workflows_picker_rows.rs`（2 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L91: `path: Some("/home/u/.grok/workflows/alpha-wf.rhai".into()),`
  - L107: `"/home/u/.grok/workflows/alpha-wf.rhai".to_string()`

### `crates\codegen\xai-grok-pager\src\views\goal_detail.rs`（70 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L1114: `goal.last_classifier_details_path = Some("/tmp/goal-details.md".into());`
  - L1131: `text.contains("/tmp/goal-details.md") && !text.contains("(unavailable)"),`
  - L2071: `classifier_details_display(Some("/tmp/exists.md"), true),`
  - L2072: `"/tmp/exists.md"`

### `crates\codegen\xai-grok-pager\src\views\memory_modal.rs`（27 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L1171: `assert_eq!(file_label("/home/user/.grok/memory/MEMORY.md"), "MEMORY.md");`

### `crates\codegen\xai-grok-pager\src\views\prompt_widget\tests.rs`（262 tests, unix 自门控命中 0）
- **posix-path** ×6（共 6 处，仅列 5）
  - L400: `img.source_path = Some(PathBuf::from("/tmp/grok-test-image.png"));`
  - L410: `!full.contains("/tmp/grok-test-image.png"),`
  - L415: `Some(std::path::Path::new("/tmp/grok-test-image.png")),`
  - L423: `!selected.contains("/tmp/grok-test-image.png"),`
  - L1162: `img.source_path = Some(PathBuf::from("/tmp/preview-path.png"));`

### `crates\codegen\xai-grok-pager\src\views\session_picker.rs`（31 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L1018: `assert_eq!(repo_name_from_cwd("/home/user/fw/1"), "fw-1");`
  - L1023: `assert_eq!(repo_name_from_cwd("/home/user/xai"), "user-xai");`
  - L1034: `repo_name_from_cwd("/home/user/projects/rust/myapp"),`

### `crates\codegen\xai-grok-pager\src\views\status_line\segments_tests.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L3: `const DIR: &str = "/home/user/project";`

### `crates\codegen\xai-grok-pager\src\views\welcome\mod.rs`（68 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L2851: `cwd: format!("/home/user/{repo_name}"),`

### `crates\codegen\xai-grok-pager\src\worktree_cmd\display.rs`（3 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L223: `std::path::Path::new(&format!("/tmp/wt-{id}")),`
  - L246: `crate::test_util::assert_path_column_aligned(&text, "/tmp/wt-");`

### `crates\codegen\xai-grok-pager\src\worktree_cmd\mod.rs`（18 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L378: `let json = r#"{"result": {"path": "/home/user/.grok/worktrees.db"}, "error": null}"#;`
  - L386: `assert_eq!(inner.path, "/home/user/.grok/worktrees.db");`

### `crates\codegen\xai-grok-pager\src\wrap_cmd_tests.rs`（11 tests, unix 自门控命中 0）
- **posix-path** ×21（共 21 处，仅列 5）
  - L18: `"/bin/zsh",`
  - L22: `assert_eq!(plan.program, "/bin/zsh");`
  - L28: `"/bin/zsh",`
  - L37: `"/bin/sh",`
  - L41: `assert_eq!(plan.program, "/bin/sh");`
- **sh-exec** ×2
  - L4: `//! The exception is the final test, which round-trips the rejoined line through a real `/bin/sh -c` to prove the quoting contract end to end.`
  - L178: `/// Feed the rejoined command line through a real `/bin/sh -c` and assert the child receives exactly the original words.`

### `crates\codegen\xai-grok-pager\tests\grok_home_paths.rs`（2 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L41: `PathBuf::from("/tmp/other").as_path()`

### `crates\codegen\xai-grok-pager\tests\signal_errno_preservation.rs`（1 tests, unix 自门控命中 0）
- **signal** ×5
  - L37: `signal_hook::low_level::register(libc::SIGUSR2, || {`
  - L42: `.expect("register SIGUSR2 handler");`
  - L45: `// SAFETY: pthread_self returns the current valid thread and SIGUSR2 is registered above.`
  - L46: `let result = unsafe { libc::pthread_kill(libc::pthread_self(), libc::SIGUSR2) };`
  - L49: `assert!(signal_hook::low_level::unregister(handler));`


## xai-grok-pager-bin — CLEAN


## xai-grok-pager-diff — RISK

### `crates\codegen\xai-grok-pager-diff\src\lib.rs`（28 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L973: `absolute_path: "/tmp/test.rs".into(),`


## xai-grok-pager-minimal — RISK

### `crates\codegen\xai-grok-pager-minimal\src\auth.rs`（6 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L432: `workspace: PathBuf::from("/tmp/untrusted-repo"),`
  - L436: `assert_eq!(workspace, PathBuf::from("/tmp/untrusted-repo"));`
  - L484: `workspace: PathBuf::from("/home/agent/project"),`
  - L493: `text.contains("/home/agent/project"),`

### `crates\codegen\xai-grok-pager-minimal\src\panel.rs`（6 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L549: `minimal_api::test_agent_view(Some("s1"), std::path::PathBuf::from("/tmp/repo"))`
  - L591: `cwd: "/tmp/repo".into(),`


## xai-grok-pager-pty-harness — RISK-LIGHT

### `crates\codegen\xai-grok-pager-pty-harness\src\leader.rs`（2 tests, unix 自门控命中 0）
- **symlink** ×1
  - L146: `// No-follow file type: a symlinked directory has `is_dir() == false`, so a symlink cycle can never recurse forever here`

### `crates\codegen\xai-grok-pager-pty-harness\src\pty.rs`（11 tests, unix 自门控命中 12）
- **signal** ×4
  - L14: `/// Grace after group SIGTERM before SIGKILL so a responsive child can run TERM cleanup. Wedged children fall through to SIGKILL.`
  - L307: `/// Exercises the real SIGINT/SIGTERM/SIGHUP paths (distinct from injected Ctrl+C key bytes, which are key events under raw mode).`
  - L459: `// Graceful first: SIGTERM the whole group so a responsive child gets`
  - L464: `// Hard stop: SIGKILL the group, kill the direct child, reap bounded.`
- **term-detect** ×2
  - L14: `/// Grace after group SIGTERM before SIGKILL so a responsive child can run TERM cleanup. Wedged children fall through to SIGKILL.`
  - L460: `// one grace period to run its own TERM cleanup before the hard kill.`
- **pty-fork** ×1
  - L129: `let pair = pty_system.openpty(size)?;`

### `crates\codegen\xai-grok-pager-pty-harness\src\pty_spawn.rs`（7 tests, unix 自门控命中 17）
- **term-detect** ×3
  - L142: `// Set TERM so the pager renders with full color support.`
  - L143: `cmd.set_var(OsStr::new("TERM"), OsStr::new("xterm-256color"));`
  - L328: `cmd.get_env("TERM").and_then(|v| v.to_str()),`
- **signal** ×1
  - L173: `/// Own fork/exec because portable-pty has no pre_exec. Linux PDEATHSIG(SIGKILL) reaps the child if the spawner dies without Drop.`
- **posix-path** ×1
  - L290: `cmd.env("GROK_SCROLL_LOG", "/tmp/scroll.jsonl");`
- **env-home** ×1
  - L352: `cmd.get_env("HOME").and_then(|v| v.to_str()),`

### `crates\codegen\xai-grok-pager-pty-harness\src\scripted.rs`（19 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L2015: `for bad in ["/etc/evil", "../escape", "sub/../../escape"] {`

### `crates\codegen\xai-grok-pager-pty-harness\src\scroll_matrix\cells.rs`（5 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L88: `const TMUX: (&str, &str) = ("TMUX", "/tmp/tmux-0/default,1,0");`

### `crates\codegen\xai-grok-pager-pty-harness\src\scroll_matrix\report.rs`（4 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L164: `log_path: format!("/tmp/{cell_id}.jsonl"),`

### `crates\codegen\xai-grok-pager-pty-harness\tests\doctor_early_dispatch.rs`（13 tests, unix 自门控命中 29）
- **posix-path** ×15（共 15 处，仅列 5）
  - L28: `"/bin/sh",`
  - L278: `"/bin/bash",`
  - L293: `"/bin/bash",`
  - L297: `("TMUX", "/tmp/tmux/default,1,0"),`
  - L319: `let mut command = base_pager_command(&binary, &home, &grok_home, "/bin/bash");`
- **env-home** ×2
  - L318: `for (key, value) in [("HOME", "."), ("BYOBU_CONFIG_DIR", "relative")] {`
  - L638: `.env("HOME", home)`
- **term-detect** ×1
  - L642: `.env("TERM", "xterm-256color")`

### `crates\codegen\xai-grok-pager-pty-harness\tests\exit_error_dead_stderr.rs`（2 tests, unix 自门控命中 0）
- **env-home** ×1
  - L20: `.env("HOME", home)`

### `crates\codegen\xai-grok-pager-pty-harness\tests\mcp_toggle_cli.rs`（10 tests, unix 自门控命中 0）
- **env-home** ×1
  - L59: `.env("HOME", &env.home)`
- **posix-path** ×1
  - L61: `.env("SHELL", "/bin/sh")`
- **term-detect** ×1
  - L63: `.env("TERM", "xterm-256color")`

### `crates\codegen\xai-grok-pager-pty-harness\tests\orphan_reap.rs`（2 tests, unix 自门控命中 0）
- **signal** ×5
  - L5: `//! (CI timeouts deliver SIGTERM then SIGKILL — neither unwinds, so no Drop`
  - L7: `//! `PR_SET_PDEATHSIG(SIGKILL)` on the child at spawn, so the *kernel* reaps it`
  - L91: `// Ungraceful death: SIGTERM/SIGKILL terminate the holder without`
  - L113: `assert_no_orphan_after_holder_killed_by(libc::SIGKILL);`
  - L118: `assert_no_orphan_after_holder_killed_by(libc::SIGTERM);`

### `crates\codegen\xai-grok-pager-pty-harness\tests\prompt_history_durable_quit.rs`（2 tests, unix 自门控命中 7）
- **signal** ×4
  - L3: `//! The other delivers a real OS SIGINT routed through the same graceful quit.`
  - L11: `//! The deterministic regression catch here is the SIGINT path exiting 0 (pre-fix it was `process::exit(130)`).`
  - L37: `/// A real OS SIGINT (not an injected Ctrl+C key byte) must route through the`
  - L126: `/// Real-SIGINT variant of [`run`]: deliver an OS signal to the pager child and`

### `crates\codegen\xai-grok-pager-pty-harness\tests\pty_auto_mode.rs`（3 tests, unix 自门控命中 0）
- **env-home** ×1
  - L42: `std::env::var_os("HOME")`
- **term-detect** ×1
  - L76: `("TERM".into(), "xterm-256color".into()),`

### `crates\codegen\xai-grok-pager-pty-harness\tests\pty_e2e\background_task_reaped_on_quit.rs`（1 tests, unix 自门控命中 4）
- **signal** ×1
  - L7: `//! Scripts a background command that records its PID then sleeps, quits via a real SIGINT, and asserts the PID is gone.`

### `crates\codegen\xai-grok-pager-pty-harness\tests\pty_e2e\bash_mode_file_completion_shell_like.rs`（1 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L14: `("SHELL".into(), "/bin/bash".into()),`

### `crates\codegen\xai-grok-pager-pty-harness\tests\pty_e2e\bash_mode_tab_completion_dropdown.rs`（1 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L22: `("SHELL".into(), "/bin/bash".into()),`

### `crates\codegen\xai-grok-pager-pty-harness\tests\pty_e2e\doubled_lines_out_of_band_repro.rs`（1 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L29: `vec![("NVIM".into(), "/tmp/grok-pty-harness-fake-nvim.sock".into())];`

### `crates\codegen\xai-grok-pager-pty-harness\tests\pty_e2e\edit_hl_inplace_refresh_pty.rs`（1 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L17: `const ARTIFACT_DIR: &str = "/tmp/edit_hl_video";`
  - L77: `"env": {"TERM": "xterm-256color", "SHELL": "/bin/zsh"},`
- **term-detect** ×1
  - L77: `"env": {"TERM": "xterm-256color", "SHELL": "/bin/zsh"},`

### `crates\codegen\xai-grok-pager-pty-harness\tests\pty_e2e\embedded_mode_boots_without_hanging_on_blocked_backend.rs`（1 tests, unix 自门控命中 0）
- **env-home** ×1
  - L29: `("HOME", home.path().to_str().unwrap()),`

### `crates\codegen\xai-grok-pager-pty-harness\tests\pty_e2e\file_path_with_space_emits_full_osc8_hyperlink.rs`（1 tests, unix 自门控命中 0）
- **term-detect** ×1
  - L24: `// The default harness PTY only sets `TERM=xterm-256color`, so the brand is `Unknown` and the pager deliberately skips OSC 8`

### `crates\codegen\xai-grok-pager-pty-harness\tests\pty_e2e\wrap_sigterm_restores_terminal_and_exit_code.rs`（1 tests, unix 自门控命中 3）
- **signal** ×1
  - L5: `/// Signal-death e2e: SIGTERM delivered to `grok wrap` itself (an external kill, or the HUP a`

### `crates\codegen\xai-grok-pager-pty-harness\tests\pty_e2e\writer_blocked_tty_keeps_loop_alive.rs`（1 tests, unix 自门控命中 0）
- **pty-fork** ×2
  - L77: `.openpty(portable_pty::PtySize {`
  - L83: `.expect("openpty");`
- **term-detect** ×1
  - L93: `cmd.env("TERM", "xterm-256color");`

### `crates\codegen\xai-grok-pager-pty-harness\tests\pty_xtversion.rs`（9 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L166: `env.push(("TMUX", "/tmp/tmux-1000/default,12345,0"));`

### `crates\codegen\xai-grok-pager-pty-harness\tests\update_never_blocked_by_config.rs`（1 tests, unix 自门控命中 0）
- **env-home** ×1
  - L45: `.env("HOME", home.path())`


## xai-grok-pager-render — RISK

### `crates\codegen\xai-grok-pager-render\src\clipboard\mod.rs`（46 tests, unix 自门控命中 15）
- **posix-path** ×5
  - L1238: `tmux_env: Some("/tmp/tmux-501/default,12345,0".to_owned()),`
  - L1251: `tmux_env: Some("/tmp/tmux-501/default,12345,0".to_owned()),`
  - L2198: `let path = std::path::PathBuf::from("/tmp/grok-1/last-copy.txt");`
  - L2219: `let path = std::path::PathBuf::from("/tmp/grok-1/last-copy.txt");`
  - L2271: `let path = std::path::PathBuf::from("/tmp/grok-1/last-copy.txt");`
- **term-detect** ×1
  - L33: `/// `grok wrap` intercepts OSC 52 onto the local clipboard and advertises it. Over SSH only `TERM` propagates, so brands look incapable.`

### `crates\codegen\xai-grok-pager-render\src\link_opener.rs`（29 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L308: `let path = std::path::Path::new("/tmp/grok session/image 1.jpg");`

### `crates\codegen\xai-grok-pager-render\src\prompt_images.rs`（156 tests, unix 自门控命中 10）
- **posix-path** ×4
  - L2286: `let images = try_read_images_from_paste("/tmp/does_not_exist_zzz.png");`
  - L2288: `assert!(try_read_image_from_path("/tmp/does_not_exist_zzz.png").is_none());`
  - L3249: `let entries = dropped_paths("/tmp/definitely_does_not_exist_xyz_grok_pager.txt");`
  - L3455: `let original_path = PathBuf::from("/tmp/ephemeral-screenshot.png");`
- **symlink** ×2
  - L1015: `/// Insert as plain text. Canonicalised when possible; raw path if canonicalisation fails (broken symlink, permission, missing).`
  - L1056: `// Fall back to the raw decoded path when canonicalisation fails (broken symlinks, permission issues, network mounts, missing files)`

### `crates\codegen\xai-grok-pager-render\src\render\image_overlay\tests.rs`（11 tests, unix 自门控命中 0）
- **posix-path** ×9（共 9 处，仅列 5）
  - L64: `Some("/tmp/logo.png"),`
  - L67: `Some(Path::new("/tmp/logo.png")),`
  - L72: `Some("/tmp/logo.png"),`
  - L75: `Some(Path::new("/tmp/logo.png")),`
  - L90: `image.session_image_path = Some(PathBuf::from("/tmp/session/image-uuid.png"));`

### `crates\codegen\xai-grok-pager-render\src\render\osc8.rs`（62 tests, unix 自门控命中 5）
- **posix-path** ×4
  - L789: `let target = tool_path_file_target("/tmp/does-not-exist-xyz/foo.rs", None).expect("target");`
  - L792: `LinkTarget::File(Arc::from(Path::new("/tmp/does-not-exist-xyz/foo.rs")))`
  - L1004: `let home = Path::new("/home/me");`
  - L1746: `let path = "/tmp/release/Demo App.app";`

### `crates\codegen\xai-grok-pager-render\src\render\tool_paths.rs`（17 tests, unix 自门控命中 3）
- **symlink** ×1
  - L48: `/// Resolve the path the OS should receive, preserving `.`/`..` and symlink semantics.`
- **posix-path** ×1
  - L322: `resolve_tool_path_target_with_home(raw, None, Some(Path::new("/home/me"))),`

### `crates\codegen\xai-grok-pager-render\src\terminal\da2.rs`（4 tests, unix 自门控命中 0）
- **term-detect** ×1
  - L10: `//! The read owns stdin, so it must run after `enable_raw_mode()` and before crossterm's `EventStream` exists.`

### `crates\codegen\xai-grok-pager-render\src\terminal\embedded_editor.rs`（7 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L46: `let env = env_from(&[("NVIM", "/tmp/nvim.12345.0")]);`
  - L52: `let env = env_from(&[("NVIM_LISTEN_ADDRESS", "/tmp/nvim.sock")]);`
  - L72: `("TMUX", "/tmp/tmux-501/default,1,0"),`
  - L86: `("NVIM", "/tmp/nvim.12345.0"),`
- **term-detect** ×1
  - L71: `("TERM", "xterm-256color"),`

### `crates\codegen\xai-grok-pager-render\src\terminal\term_version.rs`（18 tests, unix 自门控命中 0）
- **term-detect** ×3
  - L167: `let absent = build_terminal_context_from_env(&env_from(&[("TERM", "xterm-256color")]))`
  - L276: `let (version, source) = resolved(&[("TERM", "alacritty"), ("VTE_VERSION", "7402")]);`
  - L334: `let empty = snapshot(&[("TERM", "xterm-256color")]);`
- **posix-path** ×3
  - L198: `("VSCODE_GIT_ASKPASS_MAIN", "/home/u/.vscode-server/askpass"),`
  - L286: `("TMUX", "/tmp/tmux-501/default,12345,0"),`
  - L298: `("TMUX", "/tmp/tmux-501/default,12345,0"),`

### `crates\codegen\xai-grok-pager-render\src\terminal\test.rs`（168 tests, unix 自门控命中 0）
- **posix-path** ×41（共 41 处，仅列 5）
  - L203: `let env = env_from(&[("TERM_PROGRAM", "terminator"), ("TMUX", "/tmp/tmux")]);`
  - L211: `let env = env_from(&[("TERM_PROGRAM", "terminator"), ("SSH_TTY", "/dev/pts/0")]);`
  - L303: `let env = env_from(&[("BYOBU_BACKEND", "tmux"), ("TMUX", "/tmp/tmux")]);`
  - L316: `("BYOBU_CONFIG_DIR", "/home/user/.byobu"),`
  - L317: `("TMUX", "/tmp/tmux"),`
- **term-detect** ×11（共 11 处，仅列 5）
  - L128: `let env = env_from(&[("TERM", "xterm-kitty")]);`
  - L134: `let env = env_from(&[("TERM", "alacritty")]);`
  - L143: `let env = env_from(&[("TERM", "rio")]);`
  - L149: `let env = env_from(&[("TERM", "foot")]);`
  - L155: `let env = env_from(&[("TERM", "foot-extra")]);`
- **pty-fork** ×1
  - L211: `let env = env_from(&[("TERM_PROGRAM", "terminator"), ("SSH_TTY", "/dev/pts/0")]);`

### `crates\codegen\xai-grok-pager-render\src\terminal\tmux_probe.rs`（4 tests, unix 自门控命中 0）
- **signal** ×2
  - L132: `/// SIGTERM the group, then escalate to SIGKILL only if it outlives the grace.`
  - L139: `// `return`, not `break`: the reaped leader's pid may already belong to an unrelated group, so an empty group gets no SIGKILL`

### `crates\codegen\xai-grok-pager-render\src\terminal\xtversion.rs`（5 tests, unix 自门控命中 0）
- **term-detect** ×1
  - L8: `//! - Query write must happen after `enable_raw_mode()` and before the `EventStream` filter is constructed.`

### `crates\codegen\xai-grok-pager-render\src\theme\color_support.rs`（12 tests, unix 自门控命中 1）
- **term-detect** ×4
  - L193: `.get("TERM")`
  - L362: `&env(&[("TERM", "xterm-256color")]),`
  - L393: `&env(&[("TERM", "xterm-256color")]),`
  - L406: `&env(&[("TERM", "xterm")]),`

### `crates\codegen\xai-grok-pager-render\src\theme\mod.rs`（36 tests, unix 自门控命中 0）
- **term-detect** ×1
  - L437: `// But it's guaranteed visible on every 16-color terminal, including museum-grade `TERM=ansi` boxes`

### `crates\codegen\xai-grok-pager-render\src\util.rs`（17 tests, unix 自门控命中 0）
- **env-home** ×4
  - L455: `let prev = std::env::var("HOME").ok();`
  - L457: `std::env::set_var("HOME", "");`
  - L462: `Some(home) => unsafe { std::env::set_var("HOME", home) },`
  - L463: `None => unsafe { std::env::remove_var("HOME") },`


## xai-grok-paths — CLEAN


## xai-grok-plugin-marketplace — RISK

### `crates\codegen\xai-grok-plugin-marketplace\src\config.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×5
  - L279: `path = "/home/user/plugins"`
  - L287: `matches!(&sources[0].kind, SourceKind::Local { path } if path == &PathBuf::from("/home/user/plugins"))`
  - L316: `path = "/tmp/plugins"`
  - L406: `"installLocation": "/tmp/test",`
  - L472: `"installLocation": "/tmp/test"`

### `crates\codegen\xai-grok-plugin-marketplace\src\git.rs`（21 tests, unix 自门控命中 4）
- **posix-path** ×2
  - L785: `let stderr = "Cloning into '/tmp/x'...\nfatal: could not read Username for 'https://mcp.linear.app': terminal prompts disabled\n";`
  - L795: `"Cloning into '/tmp/x'...\nfatal: repository 'https://example.com/x.git/' not found\n";`

### `crates\codegen\xai-grok-plugin-marketplace\src\install_resolve.rs`（34 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L344: `addressable_qualifier(&local_source("Local Dev", "/tmp/p")),`
  - L396: `local_source("Local Dev", "/tmp/plugins"),`
  - L439: `local_source("Local Dev", "/tmp/plugins"),`
  - L487: `let sources = [local_source("Local Dev", "/tmp/plugins")];`

### `crates\codegen\xai-grok-plugin-marketplace\src\installer.rs`（17 tests, unix 自门控命中 0）
- **symlink** ×2
  - L45: `// Each plugin gets its own repo key and symlink.`
  - L78: `// Also remove the symlink if it's one.`


## xai-grok-sampler — CLEAN


## xai-grok-sampling-types — RISK-LIGHT

### `crates\codegen\xai-grok-sampling-types\src\conversation.rs`（102 tests, unix 自门控命中 0）
- **posix-path** ×11（共 11 处，仅列 5）
  - L2848: `let worktree = "/home/user/.grok/worktrees/project/ab-uuid-a";`
  - L2849: `let root = "/home/user/project";`
  - L2922: `let worktree = "/home/user/.grok/worktrees/myproject/fork-a";`
  - L2923: `let root = "/home/user/myproject";`
  - L2989: `let root = "/home/user/myproject";`


## xai-grok-sandbox — COMPILE-BREAK

### `crates\codegen\xai-grok-sandbox\src\allow_path.rs`（1 tests, unix 自门控命中 0）
- **posix-path** ×9（共 9 处，仅列 5）
  - L69: `("/home/u/.cargo/cache/**", Some("/home/u/.cargo/cache")),`
  - L70: `("/home/u/.cargo/cache/**/", Some("/home/u/.cargo/cache")),`
  - L71: `("/home/u/.cargo/cache/**/*", Some("/home/u/.cargo/cache")),`
  - L72: `("/tmp/scratch/*", Some("/tmp/scratch")),`
  - L79: `("/tmp/scratch", Some("/tmp/scratch")),`

### `crates\codegen\xai-grok-sandbox\src\child_net.rs`（8 tests, unix 自门控命中 10）
- **unix-import** ×2
  - L131: `use libc::{BPF_JEQ, BPF_JMP, BPF_K};`
  - L141: `use libc::{`

### `crates\codegen\xai-grok-sandbox\src\deny\glob.rs`（27 tests, unix 自门控命中 10）
- **symlink** ×20（共 20 处，仅列 5）
  - L229: `/// A symlinked prefix also anchors the deny at its resolved target, matching the Linux masking.`
  - L338: `/// Insert a match plus its canonical target when the path involves a symlink.`
  - L943: `fn symlinked_nested_root_keeps_its_own_walk() {`
  - L949: `std::os::unix::fs::symlink(&outside, workspace.join("secrets")).unwrap();`
  - L950: `// The workspace walk skips the symlink; the narrow root walks itself.`
- **unix-import** ×2
  - L1057: `use std::os::unix::ffi::OsStrExt;`
  - L1069: `use std::os::unix::ffi::OsStrExt;`

### `crates\codegen\xai-grok-sandbox\src\hook_write_deny_tests.rs`（6 tests, unix 自门控命中 1）
- **symlink** ×6（共 6 处，仅列 5）
  - L33: `"grok-id-symlink-{}-{}",`
  - L46: `std::os::unix::fs::symlink(&moved, &hooks).unwrap();`
  - L54: `"expected symlink/identity error, got {err:?}"`
  - L73: `std::fs::hard_link(&reg, &alias).unwrap();`
  - L139: `std::fs::hard_link(&active, &alias).unwrap();`

### `crates\codegen\xai-grok-sandbox\src\lib.rs`（19 tests, unix 自门控命中 3）
- **posix-path** ×1
  - L21: `//! let workspace = Path::new("/home/user/project");`

### `crates\codegen\xai-grok-sandbox\src\profiles.rs`（22 tests, unix 自门控命中 21）
- **symlink** ×1
  - L456: `"/private", // Real path behind /etc, /tmp, /var symlinks`

### `crates\codegen\xai-grok-sandbox\src\read_deny_verify_tests.rs`（13 tests, unix 自门控命中 0）
- **symlink** ×17（共 17 处，仅列 5）
  - L120: `let parent = temp_parent("exact-mount-symlink");`
  - L124: `std::os::unix::fs::symlink(&target, &alias).unwrap();`
  - L126: `let err = verify_exact_read_only_mount(&alias).expect_err("symlink must not be a mountpoint");`
  - L127: `assert!(err.contains("symlink"), "unexpected error: {err}");`
  - L209: `/// A symlink sentinel must be rejected at the `O_NOFOLLOW` open, before the target's filesystem is ever consulted.`
- **perm-mode** ×4
  - L78: `use std::os::unix::fs::PermissionsExt;`
  - L82: `std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap();`
  - L272: `use std::os::unix::fs::PermissionsExt;`
  - L284: `std::fs::set_permissions(&masked, std::fs::Permissions::from_mode(0o000)).unwrap();`
- **unix-import** ×2
  - L78: `use std::os::unix::fs::PermissionsExt;`
  - L272: `use std::os::unix::fs::PermissionsExt;`
- **posix-path** ×2
  - L102: `mountpoint: PathBuf::from("/tmp/deny target"),`
  - L259: `deny = [\"/var/run/docker.sock\"]\n",`

### `crates\codegen\xai-grok-sandbox\src\runtime_sockets_tests.rs`（17 tests, unix 自门控命中 18）
- **posix-path** ×4
  - L142: `let alias = PathBuf::from("/var/run/docker.sock");`
  - L398: `deny: vec!["/var/run/docker.sock".to_string()],`
  - L410: `.any(|p| p == Path::new("/var/run/docker.sock")),`
  - L416: `vec![PathBuf::from("/var/run/docker.sock")],`

### `crates\codegen\xai-grok-sandbox\tests\deny_paths_e2e.rs`（14 tests, unix 自门控命中 12）
- **symlink** ×3
  - L1173: `fs::hard_link(&reg, &alias).unwrap();`
  - L1257: `/// Hard-linked or symlinked discovery JSON under hooks/ must refuse startup.`
  - L1269: `fs::hard_link(&active, &alias).unwrap();`
- **sh-exec** ×1
  - L160: `/// Assert a denied file's bytes are unreadable via an in-process read, a `cat` child, and a nested `sh -c "cat"` child.`
- **env-home** ×1
  - L231: `std::env::set_var("HOME", &home);`

### `crates\codegen\xai-grok-sandbox\tests\integration_test.rs`（5 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L53: `logger.log(SandboxEvent::fs_violation("workspace", "/tmp/test", "read"));`
  - L56: `"/etc/shadow",`

### `crates\codegen\xai-grok-sandbox\tests\read_write_trailing_glob_e2e.rs`（2 tests, unix 自门控命中 0）
- **env-home** ×1
  - L86: `.env("HOME", home.as_os_str())`


## xai-grok-secrets — RISK

### `crates\codegen\xai-grok-secrets\src\sanitizer.rs`（16 tests, unix 自门控命中 0）
- **env-home** ×1
  - L147: `std::env::var("HOME")`


## xai-grok-session-events — CLEAN


## xai-grok-session-search — RISK

### `crates\codegen\xai-grok-session-search\src\manager.rs`（10 tests, unix 自门控命中 3）
- **perm-mode** ×1
  - L483: `/// Permission bits (`mode & 0o777`) of `path`, for owner-only assertions.`


## xai-grok-shared — RISK

### `crates\codegen\xai-grok-shared\src\clipboard.rs`（79 tests, unix 自门控命中 5）
- **posix-path** ×4
  - L2820: `let raw = sample_output("/tmp/a.png\n", "PNGf");`
  - L2822: `assert_eq!(urls.as_deref(), Some("/tmp/a.png"));`
  - L2844: `assert_eq!(urls.as_deref(), Some("/tmp/a.png"));`
  - L2861: `(Some("/tmp/foo.txt".to_owned()), None),`

### `crates\codegen\xai-grok-shared\src\placeholder_images.rs`（51 tests, unix 自门控命中 7）
- **posix-path** ×4
  - L633: `assert_eq!(matches[0].path, "/tmp/a.png");`
  - L635: `assert_eq!(matches[1].path, "/home/user/b.jpg");`
  - L671: `assert_eq!(matches[0].path, "/tmp/café.png");`
  - L732: `assert_eq!(matches[0].path, "/tmp/[odd");`
- **symlink** ×2
  - L14: `//! * Canonicalises every candidate path (resolves `..` and symlinks).`
  - L314: `/// Symlinks: this loader follows symlinks (via `canonicalize`), then checks the **resolved** path against the prefix allowlist.`


## xai-grok-shell — COMPILE-BREAK

feature 门控测试（默认构建不跑，抽样不含）：dhat-heap, local-workspace, test-support

### `crates\codegen\xai-grok-shell\src\agent\config_tests.rs`（356 tests, unix 自门控命中 0）
- **posix-path** ×21（共 21 处，仅列 5）
  - L828: `command = "/usr/local/bin/litellm-token"`
  - L845: `command: "/usr/local/bin/litellm-token".into(),`
  - L859: `assert_eq!(provider.config.command, "/usr/local/bin/litellm-token");`
  - L5510: `command = "/opt/bin/grok-identity"`
  - L5533: `path = "/tmp/plugins"`

### `crates\codegen\xai-grok-shell\src\agent\external_otel_pin.rs`（13 tests, unix 自门控命中 0）
- **posix-path** ×7（共 7 处，仅列 5）
  - L562: `otel_certificate = "/etc/ssl/corp-ca.pem"`
  - L578: `otel_logs_certificate = "/etc/ssl/logs-ca.pem"`
  - L589: `"OTEL_EXPORTER_OTLP_CERTIFICATE" => Some("/tmp/decoy-ca.pem".into()),`
  - L590: `"OTEL_EXPORTER_OTLP_METRICS_CERTIFICATE" => Some("/tmp/decoy-metrics-ca.pem".into()),`
  - L595: `Some("/etc/ssl/logs-ca.pem")`

### `crates\codegen\xai-grok-shell\src\agent\folder_trust.rs`（39 tests, unix 自门控命中 0）
- **env-home** ×1
  - L550: `let _home = EnvGuard::set("HOME", home.path());`
- **posix-path** ×1
  - L1105: `path: PathBuf::from("/home/.grok/lsp.json"),`

### `crates\codegen\xai-grok-shell\src\agent\mvp_agent\acp_agent.rs`（1 tests, unix 自门控命中 0）
- **env-home** ×1
  - L440: `"HOME": std::env::var("HOME").unwrap_or_else(|_| "(unset)".into()),`

### `crates\codegen\xai-grok-shell\src\agent\mvp_agent\replay_tests.rs`（9 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L32: `params.contains("/tmp/bg-old.log"),`
  - L55: `output_file: std::path::PathBuf::from("/tmp/bg-old.log"),`

### `crates\codegen\xai-grok-shell\src\agent\mvp_agent\tests\session_rename_tests.rs`（14 tests, unix 自门控命中 0）
- **posix-path** ×14（共 14 处，仅列 5）
  - L58: `let cwd = "/tmp/rename-resident";`
  - L103: `let cwd = "/tmp/rename-dormant";`
  - L130: `let cwd = "/tmp/rename-sanitize";`
  - L174: `let cwd = "/tmp/rename-too-long";`
  - L232: `let cwd = "/tmp/rename-strip-len";`

### `crates\codegen\xai-grok-shell\src\agent\mvp_agent\tests\session_resume_close_tests.rs`（26 tests, unix 自门控命中 0）
- **posix-path** ×5
  - L19: `acp::McpServerStdio::new("filesystem", std::path::PathBuf::from("/bin/mcp"))`
  - L26: `std::path::PathBuf::from("/tmp/proj"),`
  - L42: `std::path::PathBuf::from("/tmp/proj"),`
  - L56: `.additional_directories(vec![std::path::PathBuf::from("/tmp/extra")]),`
  - L209: `std::path::PathBuf::from("/tmp/proj"),`

### `crates\codegen\xai-grok-shell\src\agent\session_registry_client.rs`（11 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L521: `"cwd": "/home/user/repo",`
  - L544: `"cwd": "/home/user/repo",`

### `crates\codegen\xai-grok-shell\src\agent\subagent\tests\rest.rs`（131 tests, unix 自门控命中 0）
- **posix-path** ×13（共 13 处，仅列 5）
  - L397: `worktree_path: Some("/tmp/grok-wt/sa-snap".into()),`
  - L464: `worktree_path: Some("/tmp/grok-wt/subagent-x".into()),`
  - L494: `Some("/tmp/grok-wt/subagent-x")`
  - L771: `Some("/tmp/worktree"),`
  - L895: `Some("/tmp/worktree-1"),`

### `crates\codegen\xai-grok-shell\src\config\reloader.rs`（25 tests, unix 自门控命中 0）
- **posix-path** ×5
  - L794: `paths = ["/home/user/.grok/skills"]`
  - L800: `assert_eq!(skills.paths, vec!["/home/user/.grok/skills".to_string()]);`
  - L907: `command = "/bin/test"`
  - L919: `let cwd = PathBuf::from("/tmp/proj-x");`
  - L984: `command = "/bin/test"`

### `crates\codegen\xai-grok-shell\src\config\tests.rs`（202 tests, unix 自门控命中 0）
- **posix-path** ×11（共 11 处，仅列 5）
  - L2866: `source_path: Some("/tmp/home/.grok/bundled/personas/reviewer.toml".to_string()),`
  - L3058: `"[auth_provider.corp]\ncommand = \"/usr/local/bin/corp-token\"\n",`
  - L3069: `Some("/usr/local/bin/corp-token"),`
  - L3078: `[model_providers.gateway.auth]\ncommand = \"/usr/local/bin/gw-token\"\n",`
  - L3095: `Some("/usr/local/bin/gw-token"),`
- **sh-exec** ×2
  - L3206: `/// RCE guard: a project `.grok/config.toml` must never source `[feedback.user]` (its `command` runs `sh -c`).`
  - L3239: `"a project [feedback.user] must never reach Config (would be sh -c RCE)"`

### `crates\codegen\xai-grok-shell\src\config\watcher.rs`（23 tests, unix 自门控命中 7）
- **posix-path** ×3
  - L738: `let root = Path::new("/tmp/project/.claude");`
  - L751: `let project = Path::new("/tmp/repo");`
  - L918: `let grok_home = PathBuf::from("/home/u/.grok");`
- **symlink** ×2
  - L108: `// We snapshot `$HOME` here so the closure can tell `<home>/.claude.json` apart from a project-level `<cwd>/.claude.json` purely by path Canonicalize `$HOME` ON`
  - L233: `/// Answers "is `parent` the directory `dir`?" while tolerating symlink and canonicalization differences.`

### `crates\codegen\xai-grok-shell\src\extensions\background_task_tests.rs`（15 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L24: `output_file: PathBuf::from("/tmp/bg-1.log"),`
  - L138: `assert_eq!(value["tasks"][0]["output_file"], "/tmp/bg-1.log");`

### `crates\codegen\xai-grok-shell\src\extensions\marketplace.rs`（22 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L1534: `"# my custom marketplaces\n[[marketplace.sources]]\nname = \"Local\"\npath = \"/tmp/mine\"\n",`
  - L1594: `let default_skills = repo_at(std::path::Path::new("/tmp/ds"), Some("default-skills"));`
  - L1595: `let other = repo_at(std::path::Path::new("/tmp/office"), Some("plugins/office"));`
  - L1596: `let no_marketplace = repo_at(std::path::Path::new("/tmp/local"), None);`

### `crates\codegen\xai-grok-shell\src\extensions\mcp.rs`（32 tests, unix 自门控命中 0）
- **posix-path** ×14（共 14 处，仅列 5）
  - L2311: `!message.contains("/etc/grok/"),`
  - L2332: `Some(std::path::PathBuf::from("/etc/grok/managed_config.toml")),`
  - L2400: `source: std::path::PathBuf::from("/etc/grok/managed_config.toml"),`
  - L2412: `&& !message.contains("/etc/grok/"),`
  - L2559: `source: std::path::PathBuf::from("/etc/grok/managed_config.toml"),`

### `crates\codegen\xai-grok-shell\src\extensions\notification.rs`（47 tests, unix 自门控命中 0）
- **posix-path** ×7（共 7 处，仅列 5）
  - L1838: `path: Some("/home/user/.grok/memory/ws/sessions/log.md".into()),`
  - L1863: `path: Some("/home/user/.grok/memory/ws/MEMORY.md".into()),`
  - L1873: `path: "/home/user/.grok/memory/ws/sessions/2026-01-15-fix-auth-abc12345.md".into(),`
  - L1898: `path: "/home/user/.grok/memory/MEMORY.md".into(),`
  - L2060: `last_classifier_details_path: Some("/tmp/details.md".into()),`

### `crates\codegen\xai-grok-shell\src\extensions\plugins.rs`（4 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L190: `let root = std::path::PathBuf::from("/tmp/test-plugin");`

### `crates\codegen\xai-grok-shell\src\extensions\search.rs`（9 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L274: `let json = r#"{"cwd": "/home/user", "root": "src"}"#;`
  - L277: `assert_eq!(req.cwd, Some("/home/user".to_string()));`

### `crates\codegen\xai-grok-shell\src\extensions\skills.rs`（10 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L538: `let json = r#"{"path": "/home/user/skills", "cwd": "/project"}"#;`
  - L540: `assert_eq!(req.path, "/home/user/skills");`
  - L554: `let json = r#"{"path": "/home/user/skills", "cwd": "/project"}"#;`
  - L556: `assert_eq!(req.path, "/home/user/skills");`
- **symlink** ×2
  - L163: `// canonicalize resolves symlinks and `..`; fall back to the joined path if it fails (e.g. the path doesn't exist yet)`
  - L616: `/// Remote sandboxes (missing HOME, symlink-resolved homes, a pre-existing ~/my-skills) can make `starts_with($HOME)` fail spuriously.`
- **env-home** ×1
  - L625: `let _home = EnvGuard::set("HOME", &home);`

### `crates\codegen\xai-grok-shell\src\extensions\suggest\ai_provider.rs`（14 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L202: `assert_eq!(cwd, "/home/user");`
  - L208: `let result = suggest(&tx, "docker", "/home/user", Some("custom-model".into())).await;`

### `crates\codegen\xai-grok-shell\src\extensions\suggest\file_provider.rs`（53 tests, unix 自门控命中 3）
- **posix-path** ×19（共 19 处，仅列 5）
  - L411: `assert!(extract_file_context("/usr/bin/ca").is_some());`
  - L426: `assert!(is_path_like("/usr/bin"));`
  - L452: `assert_eq!(s.list_dir, PathBuf::from("/usr/bin/"));`
  - L454: `assert_eq!(s.raw_dir, "/usr/bin/");`
  - L460: `let s = split_token(&tok, "cat src/main", "/home/user", None, no_vars);`
- **symlink** ×8（共 8 处，仅列 5）
  - L24: `/// Max `stat` calls spent per scan classifying symlinks.`
  - L25: `/// A directory of up to [`SCAN_CAP`] symlinks would otherwise serialize that many `stat`s (hundreds of ms locally, worse on network filesystems).`
  - L26: `/// Past the budget a symlink classifies as a file; worst case a symlinked directory loses its trailing `/` and dirs-first ranking.`
  - L262: `let mut symlink_stats = 0usize;`
  - L304: `// `file_type()` is free on most Unix (it comes from the dirent); only symlinks need the full `stat``
- **env-home** ×3
  - L556: `let lookup = |name: &str| (name == "HOME").then(|| "/home/me".to_owned());`
  - L596: `"HOME" => Some("/home/me".to_owned()),`
  - L615: `let lookup = |name: &str| (name == "HOME").then(|| "/home/me".to_owned());`
- **uid-gid** ×1
  - L32: `"awk", "bat", "cat", "cd", "chmod", "chown", "code", "cp", "diff", "file", "find", "grep",`

### `crates\codegen\xai-grok-shell\src\extensions\worktree.rs`（15 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L627: `path: "/home/user/.grok/worktrees.db".into(),`
  - L630: `assert!(json.contains("\"path\":\"/home/user/.grok/worktrees.db\""));`

### `crates\codegen\xai-grok-shell\src\heap_profile\mod.rs`（1 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L149: `dump_to_path(Path::new("/tmp/no-hooks.heap")).unwrap_err(),`
  - L184: `let dump_path = PathBuf::from("/tmp/fake.heap");`
  - L193: `let fail_path = Path::new("/tmp/fail.heap");`

### `crates\codegen\xai-grok-shell\src\inspect\mod.rs`（33 tests, unix 自门控命中 0）
- **posix-path** ×41（共 41 处，仅列 5）
  - L1950: `source: "/etc/grok/managed-settings.json".to_owned(),`
  - L1971: `"source": "/etc/grok/managed-settings.json"`
  - L2013: `let file_type = instruction_file_type(path, Path::new("/home/user/.grok"), false, &[]);`
  - L2023: `instruction_file_type(path, Path::new("/home/user/.grok"), false, &[]),`
  - L2032: `instruction_file_type(path, Path::new("/home/user/.grok"), true, &[]),`

### `crates\codegen\xai-grok-shell\src\leader\lock.rs`（22 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L295: `let root = Path::new("/home/u/.grok");`
  - L296: `let override_sock = PathBuf::from("/home/u/.grok/leader-branch.sock");`
  - L306: `PathBuf::from("/home/u/.grok/leader-branch.lock")`
  - L312: `let root = Path::new("/home/u/.grok");`

### `crates\codegen\xai-grok-shell\src\leader\mod.rs`（45 tests, unix 自门控命中 6）
- **signal** ×6（共 6 处，仅列 5）
  - L1092: `/// and re-checks the directional guard, so this is idempotent and never downgrades), else SIGTERM its pid.`
  - L1139: `/// Signals it to vacate, waits for the pid to exit, then re-sends SIGTERM if it overran the grace window, so the caller can reclaim the socket and respawn.`
  - L1151: `warn!(error = %e, pid, "Failed to re-signal (SIGTERM) stale leader");`
  - L1328: `/// SIGTERM, wait, then escalate to SIGKILL if it overran the grace window.`
  - L1337: `warn!(error = %e, pid, "Failed to SIGTERM suspected zombie leader");`
- **symlink** ×3
  - L1556: `/// For a **managed install** — the running binary lives under `grok_home` (e.g. `~/.grok/...`) — prefer the managed `~/.grok/bin/grok` symlink. After an auto-u`
  - L1557: `/// The symlink always points to the freshly-installed version.`
  - L1591: `/// Whether `path` is located within `dir`, canonicalizing both where possible so symlinked / relative paths compare correctly.`

### `crates\codegen\xai-grok-shell\src\leader\protocol.rs`（18 tests, unix 自门控命中 0）
- **posix-path** ×7（共 7 处，仅列 5）
  - L472: `output: Some("/tmp/profile.folded".into()),`
  - L488: `} if request_id == "req-1" && output == "/tmp/profile.folded"`
  - L613: `"socket_path":"/tmp/leader.sock",`
  - L614: `"lock_path":"/tmp/leader.lock",`
  - L663: `cwd: "/home/u/proj".into(),`
- **signal** ×2
  - L299: `/// ## Runtime status | Variant | Emitted today? | Notes | |---------|---------------|-------| | `AutoUpdate` | **Yes** — when `run_auto_update_checker` trigger`
  - L309: `/// Unspecified or externally-triggered shutdown (SIGTERM, programmatic cancel, etc.).`

### `crates\codegen\xai-grok-shell\src\leader\server_tests.rs`（155 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L200: `let state = default_test_control_state(Path::new("/tmp/grok-ws-auth-test.sock"));`
  - L3439: `let req = r#"{"jsonrpc":"2.0","id":42,"method":"fs/read_text_file","params":{"sessionId":"sess-multi","path":"/tmp/x"}}"#;`
  - L4335: `let req = r#"{"jsonrpc":"2.0","id":7,"method":"fs/read_text_file","params":{"sessionId":"sess-xfer","path":"/tmp/x"}}"#;`

### `crates\codegen\xai-grok-shell\src\leader\transport.rs`（5 tests, unix 自门控命中 0）
- **posix-path** ×5
  - L235: `let a = path_to_pipe_name(Path::new("/tmp/grok.sock"));`
  - L236: `let b = path_to_pipe_name(Path::new("/tmp/grok.sock"));`
  - L242: `let a = path_to_pipe_name(Path::new("/tmp/a.sock"));`
  - L243: `let b = path_to_pipe_name(Path::new("/tmp/b.sock"));`
  - L249: `let name = path_to_pipe_name(Path::new("/tmp/test.sock"));`

### `crates\codegen\xai-grok-shell\src\mcp_doctor.rs`（8 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L855: `"/etc/grok/managed_config.toml".into(),`
  - L867: `Some("/etc/grok/managed_config.toml")`
  - L893: `let user_path = Path::new("/home/u/.grok/config.toml");`
  - L989: `source: std::path::PathBuf::from("/etc/grok/managed_config.toml"),`

### `crates\codegen\xai-grok-shell\src\plugin\acquire.rs`（8 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L658: `path: PathBuf::from("/tmp/mp"),`
  - L879: `source_path: PathBuf::from("/tmp/p"),`

### `crates\codegen\xai-grok-shell\src\plugin\mod.rs`（43 tests, unix 自门控命中 0）
- **posix-path** ×7（共 7 处，仅列 5）
  - L1679: `source_path: PathBuf::from("/tmp/plugin"),`
  - L1684: `path: PathBuf::from("/tmp/installed"),`
  - L1774: `registered_source_label(&local_source("Local Dev", "/tmp/p")),`
  - L1800: `candidate_label(&local_source("Local Dev", "/tmp/p"), "sentry"),`
  - L1980: `local_source("Local Dev", "/tmp/p"),`
- **symlink** ×2
  - L107: `/// Parse, clone/symlink, register, and enable a plugin. Does not emit telemetry.`
  - L526: `// Lexical cleanup only (`.` segments, trailing slashes; `..` is kept, no symlink resolution)`

### `crates\codegen\xai-grok-shell\src\plugin\sources.rs`（4 tests, unix 自门控命中 0）
- **posix-path** ×11（共 11 处，仅列 5）
  - L199: `path: "/opt/marketplace".into(),`
  - L208: `path: "/tmp/user-pin".into(),`
  - L224: `local_source("Local", "/tmp/p"),`
  - L228: `local_source("Pinned Local", "/opt/marketplace"),`
  - L233: `local_source("Pinned Local", "/tmp/squat"),`

### `crates\codegen\xai-grok-shell\src\remote\pull_smoke_test.rs`（1 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L42: `let test_cwd = "/tmp/smoke-test".to_string();`

### `crates\codegen\xai-grok-shell\src\session\acp_conversion.rs`（30 tests, unix 自门控命中 0）
- **posix-path** ×10（共 10 处，仅列 5）
  - L1008: `Some("/home/user/project"),`
  - L1083: `"/tmp/session/videos/3.mp4",`
  - L1095: `assert_eq!(prompt_json["path"], "/tmp/session/videos/3.mp4");`
  - L1104: `assert_eq!(raw["path"], "/tmp/session/videos/3.mp4");`
  - L1148: `output_file: "/tmp/output.txt".to_string(),`

### `crates\codegen\xai-grok-shell\src\session\acp_session.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L1756: `arguments: r#"{"target_file":"/tmp/stamp.txt"}"#.to_string(),`
  - L1804: `assert_eq!(t["input"]["path"], "/tmp/stamp.txt");`
  - L1869: `assert_eq!(t["input"]["path"], "/tmp/stamp.txt");`

### `crates\codegen\xai-grok-shell\src\session\acp_session_impl\hook_dispatch.rs`（5 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L752: `output_file: std::path::PathBuf::from("/tmp/out"),`

### `crates\codegen\xai-grok-shell\src\session\acp_session_impl\prompt_build.rs`（11 tests, unix 自门控命中 0）
- **posix-path** ×17（共 17 处，仅列 5）
  - L77: `file("/home/user/.cursor/rules/b.md"),`
  - L83: `(Path::new("/home/user/.claude").to_path_buf(), true),`
  - L84: `(Path::new("/home/user/.cursor").to_path_buf(), true),`
  - L96: `"/home/user/.cursor/rules/b.md",`
  - L191: `file_path: "/home/user/.grok/AGENTS.md".into(),`

### `crates\codegen\xai-grok-shell\src\session\acp_session_impl\stop_gate.rs`（5 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L399: `output_file: std::path::PathBuf::from("/tmp/out"),`

### `crates\codegen\xai-grok-shell\src\session\acp_session_impl\tool_calls.rs`（29 tests, unix 自门控命中 0）
- **posix-path** ×11（共 11 处，仅列 5）
  - L3217: `r#"{"file_path":"/tmp/plan.md","old_string":"a","new_string":"b"}"#,`
  - L3331: `let mut t = PlanModeTracker::new(std::path::PathBuf::from("/tmp/gate-session"));`
  - L3360: `gate(&t, &search_replace("/tmp/src/main.rs")),`
  - L3364: `gate(&t, &write("/tmp/README.md")),`
  - L3374: `gate(&t, &search_replace("/tmp/gate-session/plan.md")),`

### `crates\codegen\xai-grok-shell\src\session\acp_session_impl\tool_layer_images.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L111: `absolute_path: PathBuf::from("/tmp/x.png"),`
  - L165: `absolute_path: PathBuf::from("/tmp/x.png"),`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\auto_wake_suppression_tests.rs`（43 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L96: `output_file: "/tmp/out.log".into(),`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\cancel_running_task_tests.rs`（25 tests, unix 自门控命中 0）
- **posix-path** ×8（共 8 处，仅列 5）
  - L266: `"/tmp/test-session",`
  - L275: `"/tmp/test-session",`
  - L892: `"/tmp/test-session",`
  - L901: `"/tmp/test-session",`
  - L1232: `std::path::PathBuf::from("/tmp/test-session"),`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\fs_injection_regression_tests.rs`（1 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L9: `let cwd = std::path::PathBuf::from("/tmp/fs-injection-test-nonexistent");`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\goal\goal_planner_e2e_tests.rs`（29 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L860: `Some(std::path::PathBuf::from("/tmp/preexisting/plan.md"));`
  - L911: `Some(std::path::PathBuf::from("/tmp/has-plan/plan.md"));`
  - L1201: `snap.plan_file = Some(std::path::PathBuf::from("/tmp/has-plan/plan.md"));`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\idle_resume_tests.rs`（3 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L279: `"/tmp/test-session",`
  - L288: `"/tmp/test-session",`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\inline_auto_compact_flow_tests.rs`（16 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L207: `"/tmp/test-session",`
  - L216: `"/tmp/test-session",`
  - L654: `"/tmp/test-session",`
  - L663: `"/tmp/test-session",`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\interjection_actor_tests.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L178: `!text.contains("/tmp/secret/x.png"),`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\media_gen_batch_limit_tests.rs`（2 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L32: `r#"{"target_file":"/tmp/media-gen-batch-limit-sibling.txt"}"#,`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\memory_config_tests.rs`（15 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L316: `"/tmp/test-session",`
  - L325: `"/tmp/test-session",`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\plan_mode_edit_gate_tests.rs`（4 tests, unix 自门控命中 0）
- **posix-path** ×6（共 6 处，仅列 5）
  - L50: `search_replace_call_at("call_gate", "/tmp/src/main.rs"),`
  - L59: `text.contains("/tmp/test-session/plan.md"),`
  - L74: `search_replace_call_at("call_plan_file", "/tmp/test-session/plan.md"),`
  - L93: `search_replace_call_at("call_no_plan", "/tmp/src/main.rs"),`
  - L115: `r#"echo '{"hookSpecificOutput":{"updatedInput":{"file_path":"/tmp/src/main.rs","old_string":"a","new_string":"b"}}}'"#,`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\pre_tool_use_decision_tests.rs`（15 tests, unix 自门控命中 0）
- **posix-path** ×5
  - L54: `r#"echo '{"hookSpecificOutput":{"updatedInput":{"target_file":"/tmp/rewritten.txt"}}}'"#;`
  - L77: `prepared.parsed_args["target_file"], "/tmp/rewritten.txt",`
  - L225: `r#"echo '{"hookSpecificOutput":{"updatedInput":{"file_path":"/tmp/rewritten-edit.txt","old_string":"a","new_string":"b"}}}'"#,`
  - L238: `raw.to_string().contains("/tmp/rewritten-edit.txt"),`
  - L557: `search_replace_call_at("call_plan_file_ask", "/tmp/test-session/plan.md"),`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\prompt_context_persistence_tests.rs`（27 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L240: `std::path::PathBuf::from("/tmp/grok-test-home/sessions/cwd/sid/prompts/prompt_0.txt")`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\prompt_mode_transition_tests.rs`（10 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L106: `let mut tracker = PlanModeTracker::new(PathBuf::from("/tmp/test"));`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\replay_buffer_send_update_tests.rs`（4 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L211: `"/tmp/test-session",`
  - L220: `"/tmp/test-session",`

### `crates\codegen\xai-grok-shell\src\session\acp_session_tests\rewind_cross_compaction_tests.rs`（5 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L252: `.add_before_snapshot_for_prompt(0, Path::new("/tmp/a.rs"), cwd, Some("a".into()))`
  - L256: `.add_before_snapshot_for_prompt(0, Path::new("/tmp/b.rs"), cwd, Some("b".into()))`
  - L260: `.add_before_snapshot_for_prompt(1, Path::new("/tmp/c.rs"), cwd, Some("c".into()))`

### `crates\codegen\xai-grok-shell\src\session\compaction_inline_auto_compact_flow_tests.rs`（35 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L208: `"/tmp/test-session",`
  - L217: `"/tmp/test-session",`

### `crates\codegen\xai-grok-shell\src\session\events.rs`（10 tests, unix 自门控命中 0）
- **symlink** ×2
  - L361: `/// plan.md is (or became) a symlink; the guard refused to restore through it (treated as tampering).`
  - L362: `pub(crate) const GOAL_STRATEGIST_RESTORE_SYMLINK_TAMPER: &str = "symlink_tamper";`

### `crates\codegen\xai-grok-shell\src\session\file_system.rs`（3 tests, unix 自门控命中 0）
- **symlink** ×1
  - L111: `// The walk is confined to the canonical root when set (escaping symlinks are not enumerated); `None` (the default) walks unconfined`

### `crates\codegen\xai-grok-shell\src\session\fs_watch.rs`（27 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L1045: `&PathBuf::from("/home/user/.config/project/src/lib.rs"),`
  - L1046: `&PathBuf::from("/home/user/.config/project"),`
  - L1055: `&PathBuf::from("/home/user/.cache/repo/src/main.rs"),`
- **symlink** ×1
  - L38: `// Spelling mismatch (symlink/relative): retry on canonical roots before giving up; never scan the whole absolute path`

### `crates\codegen\xai-grok-shell\src\session\goal_classifier\evidence.rs`（46 tests, unix 自门控命中 0）
- **posix-path** ×7（共 7 处，仅列 5）
  - L1010: `ChangesRef::File("/tmp/goal-classifier-abc-1.patch"),`
  - L1012: `Some(Path::new("/home/u/.grok/sessions/s1/goal/plan.md")),`
  - L1091: `ChangesRef::File("/tmp/p.patch"),`
  - L1105: `ChangesRef::File("/tmp/p.patch"),`
  - L1107: `Some(Path::new("/tmp/plan.md")),`
- **symlink** ×2
  - L663: `// Symlinks are never followed: a symlink into a skipped dir cannot smuggle target bytes into the diff`
  - L691: `// Capture the first hard I/O error; continue walking so one bad symlink doesn't abort the rest`

### `crates\codegen\xai-grok-shell\src\session\goal_classifier_tests.rs`（141 tests, unix 自门控命中 8）
- **posix-path** ×50（共 50 处，仅列 5）
  - L35: `Path::new("/tmp/details.md"),`
  - L88: `Path::new("/tmp/details.md"),`
  - L146: `Path::new("/tmp/d.md"),`
  - L214: `let mac_like = Path::new("/var/folders/zz/T");`
  - L217: `Path::new("/var/folders/zz/T/grok-goal-abc/goal-classifier-abc-1.md"),`
- **symlink** ×2
  - L3560: `/// A symlink pre-planted at the predictable bare-`/tmp` artifact name is never followed.`
  - L3561: `/// Artifacts resolve into the scratch root, and the symlink's victim file stays untouched.`

### `crates\codegen\xai-grok-shell\src\session\goal_orchestrator.rs`（10 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L310: `o.last_classifier_details_path = Some("/tmp/details.md".into());`
  - L330: `Some("/tmp/details.md")`

### `crates\codegen\xai-grok-shell\src\session\goal_strategist.rs`（19 tests, unix 自门控命中 13）
- **symlink** ×12（共 12 处，仅列 5）
  - L10: `//! A restore I/O failure or a symlink planted at the path is refused and surfaced via `GoalStrategistContractRestoreFailed` telemetry.`
  - L399: `/// plan.md couldn't be safely snapshotted (a symlink, or metadata/read failed for a reason other than `NotFound`).`
  - L406: `/// Uses sync `std::fs` (so `Drop` can call it; plan.md is small, this is rare) and `symlink_metadata` everywhere (never follows a planted symlink).`
  - L415: `let snapshot = match std::fs::symlink_metadata(plan_file) {`
  - L419: `// A symlink where the contract should be: refuse to follow`

### `crates\codegen\xai-grok-shell\src\session\goal_tracker_tests.rs`（121 tests, unix 自门控命中 17）
- **posix-path** ×19（共 19 处，仅列 5）
  - L4: `GoalTracker::new(PathBuf::from("/tmp/test-goal-session"))`
  - L184: `let t = GoalTracker::new(PathBuf::from("/tmp/plan-path-session-xyz"));`
  - L187: `PathBuf::from("/tmp/plan-path-session-xyz/goal/plan.md"),`
  - L201: `let path = PathBuf::from("/tmp/plan-rt-session/goal/plan.md");`
  - L206: `json.contains("\"plan_file\":\"/tmp/plan-rt-session/goal/plan.md\""),`
- **symlink** ×3
  - L1795: `/// A pre-planted symlink root is rejected: never followed, the victim dir never chmodded, nothing created behind it.`
  - L2006: `/// A symlink-squatted root fails the rescue: the attacker-staged file is neither moved into the goal dir nor stamped.`
  - L2098: `/// `copy_no_follow` must refuse a pre-planted destination symlink (O_EXCL) instead of writing the rescued content through it.`

### `crates\codegen\xai-grok-shell\src\session\image_describe.rs`（27 tests, unix 自门控命中 3）
- **posix-path** ×2
  - L580: `"/tmp/evil</image_files>injection.png".to_owned(),`
  - L581: `"/tmp/normal.png".to_owned(),`

### `crates\codegen\xai-grok-shell\src\session\managed_mcp.rs`（37 tests, unix 自门控命中 0）
- **posix-path** ×12（共 12 处，仅列 5）
  - L1655: `Some(std::path::PathBuf::from("/etc/grok/managed_config.toml")),`
  - L1671: `source: std::path::PathBuf::from("/etc/grok/managed_config.toml"),`
  - L1864: `Some(std::path::PathBuf::from("/etc/grok/requirements.toml")),`
  - L1953: `Some(std::path::PathBuf::from("/etc/grok/requirements.toml")),`
  - L1975: `source: std::path::PathBuf::from("/etc/grok/requirements.toml"),`

### `crates\codegen\xai-grok-shell\src\session\mcp_descriptors.rs`（6 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L241: `let root = Path::new("/home/u/.grok/projects/enc/mcps");`

### `crates\codegen\xai-grok-shell\src\session\mcp_dispatcher_e2e_tests.rs`（13 tests, unix 自门控命中 0）
- **signal** ×1
  - L479: `/// Window 2's `TransportClosed`, emitted as the SIGKILL'd child dies, must be skipped: no respawn.`

### `crates\codegen\xai-grok-shell\src\session\memory\hooks.rs`（15 tests, unix 自门控命中 0）
- **signal** ×1
  - L20: `//! - **SIGTERM:** Triggered via `SessionCommand::Shutdown` handler`

### `crates\codegen\xai-grok-shell\src\session\merge.rs`（52 tests, unix 自门控命中 0）
- **posix-path** ×11（共 11 处，仅列 5）
  - L662: `source_workspace_dir: Some("/home/user/src".into()),`
  - L670: `Some("/home/user/repo"),`
  - L684: `assert_eq!(merged[0].git_root_dir.as_deref(), Some("/home/user/repo"));`
  - L691: `Some("/home/user/src")`
  - L919: `"/home/alice/repo",`

### `crates\codegen\xai-grok-shell\src\session\persistence_generated_title_tests.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L88: `"git_root_dir": "/home/user/myrepo",`
  - L99: `assert_eq!(summary.git_root_dir.as_deref(), Some("/home/user/myrepo"));`

### `crates\codegen\xai-grok-shell\src\session\plan_mode.rs`（61 tests, unix 自门控命中 0）
- **posix-path** ×29（共 29 处，仅列 5）
  - L404: `PlanModeTracker::new(PathBuf::from("/tmp/test-session"))`
  - L464: `let t = PlanModeTracker::new(PathBuf::from("/home/user/.grok/sessions/proj/abc-123"));`
  - L467: `Path::new("/home/user/.grok/sessions/proj/abc-123/plan.md")`
  - L599: `"/tmp/session/plan.md",`
  - L605: `"/tmp/session/plan.md",`

### `crates\codegen\xai-grok-shell\src\session\signals_tests.rs`（41 tests, unix 自门控命中 0）
- **posix-path** ×14（共 14 处，仅列 5）
  - L986: `handle.record_loc_change(true, 10, 0, "/tmp/a.rs".into());`
  - L987: `handle.record_loc_change(true, 5, 2, "/tmp/b.rs".into());`
  - L990: `handle.record_loc_change(false, 3, 0, "/tmp/a.rs".into());`
  - L991: `handle.record_loc_change(false, 7, 1, "/tmp/c.rs".into());`
  - L1022: `handle.record_loc_change(true, 10, 0, "/tmp/a.rs".into());`

### `crates\codegen\xai-grok-shell\src\session\slash_commands_tests.rs`（104 tests, unix 自门控命中 0）
- **posix-path** ×7（共 7 处，仅列 5）
  - L2156: `let tracker = GoalTracker::new(std::path::PathBuf::from("/tmp/test"));`
  - L2164: `let mut tracker = GoalTracker::new(std::path::PathBuf::from("/tmp/test"));`
  - L2173: `let mut tracker = GoalTracker::new(std::path::PathBuf::from("/tmp/test"));`
  - L2187: `let mut tracker = GoalTracker::new(std::path::PathBuf::from("/tmp/test"));`
  - L2199: `let mut tracker = GoalTracker::new(std::path::PathBuf::from("/tmp/test"));`

### `crates\codegen\xai-grok-shell\src\session\storage\jsonl\tests.rs`（87 tests, unix 自门控命中 17）
- **posix-path** ×4
  - L1251: `let cwd = crate::util::grok_home::encode_cwd_dirname("/home/user/project");`
  - L1262: `let cwd_a = crate::util::grok_home::encode_cwd_dirname("/home/user/project-a");`
  - L1263: `let cwd_b = crate::util::grok_home::encode_cwd_dirname("/home/user/project-b");`
  - L1267: `let a_dirs = adapter.scan_session_dirs(Some("/home/user/project-a")).unwrap();`

### `crates\codegen\xai-grok-shell\src\session\storage\jsonl\worktree_heal_tests.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×9（共 9 处，仅列 5）
  - L98: `kinded.source_workspace_dir = Some("/home/user/repo".to_owned());`
  - L112: `Some("/home/user/repo")`
  - L122: `Some("/home/user/repo")`
  - L141: `kinded.source_workspace_dir = Some("/home/user/repo".to_owned());`
  - L312: `kinded.source_workspace_dir = Some("/home/user/repo".to_owned());`

### `crates\codegen\xai-grok-shell\src\session\storage\mod.rs`（56 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L3550: `r#"{"sessionUpdate":"tool_call","toolCallId":"tc1","title":"Read `/tmp/foo.rs`","kind":"read","locations":[{"path":"/tmp/foo.rs"}]}"#,`
  - L3555: `assert!(result.contains(&"/tmp/foo.rs".to_string()));`

### `crates\codegen\xai-grok-shell\src\session\storage\relocation\mod.rs`（1 tests, unix 自门控命中 0）
- **symlink** ×1
  - L166: `match fs::symlink_metadata(&summary) {`

### `crates\codegen\xai-grok-shell\src\session\storage\replay_tests.rs`（45 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L938: `let cwd = "/tmp/orphan-complete";`

### `crates\codegen\xai-grok-shell\src\session\storage\search_content_tests.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L63: `r#"{"sessionUpdate":"tool_call","toolCallId":"tc1","title":"Read file","kind":"read","locations":[{"path":"/tmp/foo.rs"}]}"#,`
  - L69: `content.contains("/tmp/foo.rs"),`
  - L88: `r#"{"sessionUpdate":"tool_call","toolCallId":"tc1","title":"Run \"cargo test\"","kind":"execute","locations":[{"path":"/tmp/my\tdir/foo.rs"}]}"#,`
  - L106: `content.contains("/tmp/my\tdir/foo.rs"),`

### `crates\codegen\xai-grok-shell\src\session\storage\summary_write.rs`（24 tests, unix 自门控命中 0）
- **posix-path** ×8（共 8 处，仅列 5）
  - L1108: `&identity("fix-bug", Some("/home/user/repo")),`
  - L1117: `expected.source_workspace_dir = Some("/home/user/repo".to_owned());`
  - L1145: `&identity("fix-bug", Some("/home/user/repo")),`
  - L1173: `&identity("fix-bug", Some("/home/user/repo")),`
  - L1200: `&identity("fix-bug", Some("/home/user/repo")),`

### `crates\codegen\xai-grok-shell\src\session\telemetry\mod.rs`（4 tests, unix 自门控命中 1）
- **posix-path** ×4
  - L218: `Path::new("/home/u/.grok/skills/review/SKILL.md"),`
  - L219: `Path::new("/home/u/.grok/skills/review/SKILL.md")`
  - L226: `Path::new("/home/u/.grok/skills/review/SKILL.md"),`
  - L227: `Path::new("/home/u/.grok/skills/design/SKILL.md")`
- **symlink** ×1
  - L44: `/// Paths are canonicalized so a symlinked cwd (macOS `/tmp` vs `/private/tmp`) still matches.`

### `crates\codegen\xai-grok-shell\src\session\workflow\host_service.rs`（4 tests, unix 自门控命中 0）
- **symlink** ×7（共 7 处，仅列 5）
  - L801: `match std::fs::symlink_metadata(path) {`
  - L803: `"{what} must not be a symlink: {}",`
  - L829: `let meta = std::fs::symlink_metadata(&path)`
  - L833: `"scratch directory contains a symlink: {}",`
  - L862: `let target_exists = std::fs::symlink_metadata(&path)`

### `crates\codegen\xai-grok-shell\src\session\workflow\registry.rs`（15 tests, unix 自门控命中 10）
- **symlink** ×6（共 6 处，仅列 5）
  - L234: `let Ok(dir_meta) = std::fs::symlink_metadata(dir) else {`
  - L318: `let path_meta = std::fs::symlink_metadata(&candidate).map_err(|error| ResolveError::Io {`
  - L325: `reason: "expected a non-symlink regular file".into(),`
  - L444: `let meta = std::fs::symlink_metadata(path).map_err(|error| ResolveError::Io {`
  - L451: `reason: "expected a non-symlink regular file".into(),`

### `crates\codegen\xai-grok-shell\src\session\workflow\store.rs`（4 tests, unix 自门控命中 1）
- **symlink** ×1
  - L308: `let metadata = std::fs::symlink_metadata(path)?;`

### `crates\codegen\xai-grok-shell\src\session\worktree.rs`（37 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L917: `source_cwd: "/tmp/definitely-not-a-repo".to_string(),`
  - L1075: `let base = worktree_base_dir(Path::new("/home/user/projects/my-repo"));`
  - L1114: `let outcome = checkout_persisted_head_in_worktree("/tmp/irrelevant", None, "sess").await;`
  - L1120: `checkout_persisted_head_in_worktree("/tmp/irrelevant", Some(""), "sess").await;`

### `crates\codegen\xai-grok-shell\src\tools\notification_bridge_tests.rs`（45 tests, unix 自门控命中 0）
- **posix-path** ×11（共 11 处，仅列 5）
  - L86: `crate::session::plan_mode::PlanModeTracker::new(PathBuf::from("/tmp/test-session")),`
  - L1085: `output_file: PathBuf::from("/tmp/out.log"),`
  - L1116: `output_file: PathBuf::from("/tmp/out.log"),`
  - L1212: `output_file: PathBuf::from("/tmp/out.log"),`
  - L1705: `plan_file_path: "/tmp/test-session/plan.md".into(),`

### `crates\codegen\xai-grok-shell\src\tools\task_completed_frame_tests.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L22: `output_file: PathBuf::from("/tmp/bg-1.log"),`
  - L66: `assert!(snapshot.output.contains("/tmp/bg-1.log"));`
  - L100: `PathBuf::from(format!("/tmp/{}/task.log", "p".repeat(30 * 1024)));`
  - L167: `serde_json::Value::String(format!("/tmp/{}/task.log", "p".repeat(80 * 1024)));`

### `crates\codegen\xai-grok-shell\src\upload\trace.rs`（44 tests, unix 自门控命中 0）
- **perm-mode** ×2
  - L939: `header.set_mode(0o644);`
  - L1567: `header.set_mode(0o644);`

### `crates\codegen\xai-grok-shell\src\util\config\mcp_reenable.rs`（6 tests, unix 自门控命中 0）
- **env-home** ×1
  - L195: `let home_guard = xai_grok_test_support::EnvGuard::set("HOME", home.path());`

### `crates\codegen\xai-grok-shell\src\util\hooks.rs`（3 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L188: `command = "/opt/policy/pin-session-start.sh"`
  - L194: `command = "/opt/policy/pin-prompt-submit.sh"`
  - L201: `command = "/opt/policy/pin-pre-tool-use.sh"`

### `crates\codegen\xai-grok-shell\tests\e2e_grove_worktree.rs`（2 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L48: `&& Path::new("/dev/fuse").exists()`
  - L52: `.open("/dev/fuse")`
  - L68: `panic!("/dev/fuse required (GROVE_REQUIRE_FUSE truthy) but not usable");`
- **perm-mode** ×3
  - L123: `use std::os::unix::fs::PermissionsExt;`
  - L125: `perms.set_mode(0o700);`
  - L126: `std::fs::set_permissions(path, perms).expect("chmod 0700");`
- **unix-import** ×2
  - L123: `use std::os::unix::fs::PermissionsExt;`
  - L277: `use std::os::unix::fs::MetadataExt;`
- **env-home** ×1
  - L170: `.env("HOME", self._tmp.path().join("home"))`

### `crates\codegen\xai-grok-shell\tests\external_auth_conforming_provider.rs`（1 tests, unix 自门控命中 0）
- **perm-mode** ×2
  - L30: `use std::os::unix::fs::PermissionsExt;`
  - L49: `std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))`
- **unix-import** ×1
  - L30: `use std::os::unix::fs::PermissionsExt;`

### `crates\codegen\xai-grok-shell\tests\external_auth_expired_credential.rs`（2 tests, unix 自门控命中 0）
- **perm-mode** ×2
  - L140: `use std::os::unix::fs::PermissionsExt;`
  - L151: `std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))`
- **unix-import** ×1
  - L140: `use std::os::unix::fs::PermissionsExt;`

### `crates\codegen\xai-grok-shell\tests\test_auth_provider_command_e2e.rs`（1 tests, unix 自门控命中 0）
- **sh-exec** ×2
  - L3: `//! The provider used to be spawned through a hardcoded `sh -c`.`
  - L78: `// `echo <token>` is valid in both `sh -c` and `cmd /C`, so this phase needs no external binary and runs identically on every platform`

### `crates\codegen\xai-grok-shell\tests\test_mcp_doctor_isolation.rs`（1 tests, unix 自门控命中 0）
- **env-home** ×1
  - L17: `std::env::set_var("HOME", &dir);`

### `crates\codegen\xai-grok-shell\tests\test_mcp_permission_persistence.rs`（17 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L612: `AccessKind::Read(Some("/tmp/test.txt".to_string())),`
  - L695: `pattern: Some("/etc/**".to_owned()),`
  - L704: `let d = request(&handle, AccessKind::Edit("/etc/passwd".to_string()), "1").await;`
  - L775: `let d = request(&handle, AccessKind::Edit("/etc/passwd".to_string()), "3").await;`

### `crates\codegen\xai-grok-shell\tests\test_trusted_local_plugin_refresh_e2e.rs`（2 tests, unix 自门控命中 0）
- **symlink** ×3
  - L103: `// Canonicalize so auto-trust for installs under the home dir holds when the temp root is a symlink (on macOS `/var` links to `/private/var`)`
  - L118: `// The install copies a full snapshot into installed-plugins, not a live symlink`
  - L202: `// Canonicalize so auto-trust for installs under the home dir holds when the temp root is a symlink (on macOS `/var` links to `/private/var`)`
- **env-home** ×3
  - L107: `let _home_guard = EnvVarGuard::set("HOME", &home);`
  - L218: `let _home_guard = EnvVarGuard::set("HOME", &home);`
  - L246: `.set_env("HOME", &home)`


## xai-grok-shell-base — RISK

### `crates\codegen\xai-grok-shell-base\src\util\mod.rs`（20 tests, unix 自门控命中 10）
- **signal** ×4
  - L216: `/// Graceful `SIGTERM` (Unix); the process may catch and drain.`
  - L218: `/// Forceful `SIGKILL` (Unix); the process cannot catch or block it.`
  - L221: `/// Terminate a process by PID with `SIGTERM`. Idempotent: already-dead is `Ok`.`
  - L225: `/// Terminate a process by PID with a chosen signal. Idempotent: already-dead is `Ok`. Unix: `SIGTERM`/`SIGKILL` via `nix::sys::signal::kill`; ESRCH maps to `Ok`

### `crates\codegen\xai-grok-shell-base\src\util\secure_file.rs`（5 tests, unix 自门控命中 18）
- **perm-mode** ×4
  - L5: `//! - **Unix**: mode 0o600 (owner read/write only)`
  - L40: `/// `mode(0o600)` only applies when the file is newly created, not when truncating an existing path.`
  - L53: `/// Ensure `path` is owner-read/write only (Unix `0o600` / Windows user ACL). Best-effort on missing files (`NotFound` is ignored). Other errors propagate so ca`
  - L88: `/// This is equivalent to Unix mode 0o600.`

### `crates\codegen\xai-grok-shell-base\src\util\subprocess.rs`（8 tests, unix 自门控命中 0）
- **signal** ×3
  - L28: `/// Grace between SIGTERM and SIGKILL when tearing down a timed-out process group, so a signal-aware child can exit cleanly before the force kill.`
  - L167: `/// Tear down a timed-out or errored run: SIGTERM the process group, wait [`TERM_GRACE`], then SIGKILL survivors.`
  - L338: `"escalation must bound teardown of a SIGTERM-ignoring child (took {:?})",`
- **sh-exec** ×2
  - L47: `/// Run a config-provided command string through the platform shell: `sh -c` on unix, `cmd /C` on Windows. It is the escape hatch shared by the auth providers a`
  - L252: `/// `echo hi` is valid in both `sh -c` and `cmd /C`.`
- **term-detect** ×1
  - L330: `sh("trap '' TERM; while :; do sleep 0.2; done"),`


## xai-grok-shell-session-support — CLEAN


## xai-grok-shell-terminal — RISK

### `crates\codegen\xai-grok-shell-terminal\src\adapter_tests.rs`（31 tests, unix 自门控命中 0）
- **signal** ×4
  - L44: `let signal = Some(acp::TerminalExitStatus::new().signal(Some("SIGKILL".into())));`
  - L45: `assert_eq!(parse_exit(&signal), (None, Some("SIGKILL".into())));`
  - L62: `make_tracked_task("killed").to_snapshot("t-3", out("", None, Some("SIGTERM".into())));`
  - L704: `task.mark_completed(out("", None, Some("SIGTERM".into())));`
- **posix-path** ×1
  - L10: `output_file: PathBuf::from("/tmp/out.log"),`

### `crates\codegen\xai-grok-shell-terminal\src\background_task.rs`（26 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L317: `output_file: PathBuf::from(format!("/tmp/{}.log", task_id)),`
  - L521: `output_file: PathBuf::from(format!("/tmp/sessions/tasks/{task_id}.log")),`
  - L523: `cwd: "/home/user".to_string(),`

### `crates\codegen\xai-grok-shell-terminal\src\local_terminal.rs`（3 tests, unix 自门控命中 2）
- **signal** ×1
  - L48: `/// A process wedged in uninterruptible kernel I/O (D-state) also holds the pipe open, and not even SIGKILL moves it.`

### `crates\codegen\xai-grok-shell-terminal\src\pty_session.rs`（5 tests, unix 自门控命中 5）
- **pty-fork** ×1
  - L260: `.openpty(size)`
- **term-detect** ×1
  - L279: `cmd.env("TERM", "xterm-256color");`
- **signal** ×1
  - L645: `/// Dropping the master would not hang the shell up: the reader and writer hold their own dups of it, and SIGHUP needs the last one closed.`

### `crates\codegen\xai-grok-shell-terminal\src\streaming_local_terminal.rs`（7 tests, unix 自门控命中 2）
- **signal** ×5
  - L20: `/// Upper bound on how long terminal teardown waits for a SIGKILL'd child to be reaped.`
  - L22: `/// Teardown runs on the session actor's cancel path, and on the leader every session shares one `LocalSet` thread, so the wait must be bounded. The child is al`
  - L180: `// Deliver SIGKILL to each foreground child *synchronously* so teardown begins immediately, but do NOT wait for it to exit here`
  - L208: `// The children are already SIGKILL'd with `KillOnDrop` set, so they are torn down even if this reaper is cancelled The bounded waits (`KILL_REAP_TIMEOUT`) run `
  - L344: `// Bounded reap: the process has been SIGKILL'd; don't let a wedged child (stuck in an uninterruptible syscall) block the caller indefinitely`
- **posix-path** ×1
  - L1347: `/// After setsid(), the child has no controlling terminal, so `open("/dev/tty")` must fail with ENXIO.`


## xai-grok-status-line — RISK

### `crates\codegen\xai-grok-status-line\src\context_tests.rs`（3 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L5: `const DIR: &str = "/home/user/project";`
  - L31: `transcript_path: Some("/home/user/sessions/019fa651/updates.jsonl".into()),`
  - L88: `path: "/home/user/wt/feature-x".into(),`


## xai-grok-subagent-resolution — CLEAN


## xai-grok-telemetry — RISK

### `crates\codegen\xai-grok-telemetry\src\debug_log.rs`（22 tests, unix 自门控命中 20）
- **posix-path** ×4
  - L553: `Some(OsStr::new("/tmp/custom.log")),`
  - L560: `path: PathBuf::from("/tmp/custom.log"),`
  - L569: `Some(OsStr::new("/tmp/explicit.log")),`
  - L577: `path: PathBuf::from("/tmp/explicit.log"),`
- **symlink** ×3
  - L7: `//!   a `latest.txt` symlink pointing at the most-recently-opened session file.`
  - L449: `// a recently-written (active) log is never deleted. Spares the `latest.txt` symlink (a stale link is harmless and never`
  - L468: `// `DirEntry::metadata` does not follow symlinks, so a dangling orphaned temp still yields its own mtime here`

### `crates\codegen\xai-grok-telemetry\src\events\mod.rs`（31 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L3161: `assert!(!text.contains("/dev/fuse"), "{text}");`
  - L3235: `assert!(!text.contains("/dev/fuse"), "{text}");`

### `crates\codegen\xai-grok-telemetry\src\external\config.rs`（37 tests, unix 自门控命中 0）
- **posix-path** ×33（共 33 处，仅列 5）
  - L1061: `("OTEL_EXPORTER_OTLP_CERTIFICATE", "/etc/ssl/corp-ca.pem"),`
  - L1064: `"/etc/ssl/metrics-ca.pem",`
  - L1072: `Some("/etc/ssl/corp-ca.pem")`
  - L1076: `Some("/etc/ssl/metrics-ca.pem")`
  - L1287: `"/etc/ssl/client.crt",`

### `crates\codegen\xai-grok-telemetry\src\external\providers.rs`（17 tests, unix 自门控命中 0）
- **term-detect** ×2
  - L141: `// terminal.type: emulator brand (TERM_PROGRAM) or terminfo type (TERM).`
  - L144: `.or_else(|| std::env::var("TERM").ok())`

### `crates\codegen\xai-grok-telemetry\src\external\tests.rs`（57 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L863: `.unwrap_or_else(|| "/home/testuser".into());`
  - L1483: `file_path: Some("/tmp/x.rs".into()),`
  - L1496: `assert_eq!(attr(out, "file_path").as_deref(), Some("/tmp/x.rs"));`
  - L1514: `file_path: Some("/tmp/secret.rs".into()),`

### `crates\codegen\xai-grok-telemetry\src\id.rs`（3 tests, unix 自门控命中 9）
- **perm-mode** ×1
  - L97: `xai_grok_config::fs_atomic::write_atomically(path, id, Some(0o600))`

### `crates\codegen\xai-grok-telemetry\src\unified_log.rs`（16 tests, unix 自门控命中 4）
- **perm-mode** ×2
  - L161: `/// Owner-only (0o700), freshly-created directory for the test redirect. The non-recursive `create` fails on any`
  - L175: `#[cfg_attr(not(unix), allow(unused_mut))] // LOCAL: .mode(0o700) is unix-only`

### `crates\codegen\xai-grok-telemetry\tests\external_otlp_gates_on.rs`（2 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L125: `file_path: Some("/tmp/projectdir/config.toml".into()),`


## xai-grok-test-support — RISK

### `crates\codegen\xai-grok-test-support\src\leader.rs`（4 tests, unix 自门控命中 0）
- **term-detect** ×5
  - L919: `let leader = fake_leader("trap 'exit 0' TERM; while :; do sleep 1; done");`
  - L934: `fake_leader("trap 'exit 0' TERM; while :; do sleep 1; done"),`
  - L951: `let initial = fake_leader("trap 'exit 0' TERM; while :; do sleep 1; done");`
  - L953: `let mut replacement = fake_leader("trap 'exit 0' TERM; while :; do sleep 1; done");`
  - L974: `"trap 'exit 0' TERM; sh -c 'trap \"\" TERM; echo $$ > {}; while :; do sleep 1; done' & while :; do sleep 1; done",`
- **posix-path** ×1
  - L889: `let mut cmd = std::process::Command::new("/bin/sh");`
- **sh-exec** ×1
  - L974: `"trap 'exit 0' TERM; sh -c 'trap \"\" TERM; echo $$ > {}; while :; do sleep 1; done' & while :; do sleep 1; done",`

### `crates\codegen\xai-grok-test-support\src\sandbox.rs`（14 tests, unix 自门控命中 6）
- **env-home** ×2
  - L474: `"HOME" | "USERPROFILE" | "GROK_HOME" | "TMPDIR" | "TMP" | "TEMP" | "GIT_CONFIG_GLOBAL"`
  - L696: `assert_eq!(env_value(&sandbox, "HOME"), Some(sandbox.home().into()));`
- **posix-path** ×1
  - L583: `(OsString::from("PATH"), OsString::from("/usr/bin")),`


## xai-grok-tools — COMPILE-BREAK

feature 门控测试（默认构建不跑，抽样不含）：dhat-heap

### `crates\codegen\xai-grok-tools\src\computer\local\embedded_search_tools.rs`（21 tests, unix 自门控命中 4）
- **posix-path** ×13（共 13 处，仅列 5）
  - L257: `bfs: Some(PathBuf::from("/tmp/bfs")),`
  - L258: `ugrep: Some(PathBuf::from("/tmp/ugrep")),`
  - L264: `let fn_body = shell_function("find", "bfs", Some(Path::new("/tmp/bfs")), &[]);`
  - L299: `Some(Path::new("/tmp/ugrep")),`
  - L310: `assert_eq!(bash_safe_quote("/usr/bin/bfs"), "/usr/bin/bfs");`
- **perm-mode** ×7（共 7 处，仅列 5）
  - L409: `// Mode 0o644 — no execute bit; Nix-like / restricted copies.`
  - L642: `use std::os::unix::fs::PermissionsExt;`
  - L644: `perms.set_mode(0o755);`
  - L645: `std::fs::set_permissions(&bin, perms).unwrap();`
  - L714: `use std::os::unix::fs::PermissionsExt;`
- **unix-import** ×2
  - L642: `use std::os::unix::fs::PermissionsExt;`
  - L714: `use std::os::unix::fs::PermissionsExt;`

### `crates\codegen\xai-grok-tools\src\computer\local\shell_state.rs`（26 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L672: `assert_eq!(cwd, PathBuf::from("/home/user/project"));`
  - L690: `let raw = "/home/user\nexport FOO=bar\n__GROK_BASH_STATE_END__\n";`
  - L713: `assert_eq!(cwd, PathBuf::from("/home/user"));`
- **unix-import** ×2
  - L11: `use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};`
  - L19: `use nix::libc;`
- **term-detect** ×2
  - L47: `("TERM".to_string(), "dumb".to_string()),`
  - L660: `assert_eq!(env.get("TERM").map(String::as_str), Some("dumb"));`
- **pty-fork** ×1
  - L508: `/// close-on-exec, eliminating the race window between `pipe()` and `fcntl(F_SETFD)` where a concurrent `fork()` could leak fds to an unrelated`
- **symlink** ×1
  - L933: `// cd to /tmp (macOS resolves to /private/tmp via symlink)`

### `crates\codegen\xai-grok-tools\src\computer\local\static_shell.rs`（4 tests, unix 自门控命中 0）
- **unix-import** ×2
  - L12: `use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};`
  - L18: `use nix::libc;`
- **posix-path** ×1
  - L241: `std::path::Path::new("/bin/bash").exists()`

### `crates\codegen\xai-grok-tools\src\computer\local\terminal.rs`（67 tests, unix 自门控命中 5）
- **signal** ×4
  - L42: `/// SIGTERM → SIGKILL grace period.`
  - L1805: `// An exited task may still hold a live child. Escalate to SIGKILL if`
  - L2971: `// SIGKILL almost always reaps instantly; the cap protects against D-state`
  - L2980: `"Process did not exit after SIGKILL within {:?}, \`

### `crates\codegen\xai-grok-tools\src\computer\types.rs`（11 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L574: `output_file: PathBuf::from("/tmp/t.log"),`

### `crates\codegen\xai-grok-tools\src\implementations\codex\grep_files\tool.rs`（14 tests, unix 自门控命中 0）
- **posix-path** ×6（共 6 处，仅列 5）
  - L282: `let stdout = b"/tmp/file_a.rs\n/tmp/file_b.rs\n";`
  - L286: `vec!["/tmp/file_a.rs".to_string(), "/tmp/file_b.rs".to_string()]`
  - L292: `let stdout = b"/tmp/file_a.rs\n/tmp/file_b.rs\n/tmp/file_c.rs\n";`
  - L296: `vec!["/tmp/file_a.rs".to_string(), "/tmp/file_b.rs".to_string()]`
  - L302: `let stdout = b"/tmp/file_a.rs\n\n\n/tmp/file_b.rs\n";`

### `crates\codegen\xai-grok-tools\src\implementations\codex\list_dir\tool.rs`（16 tests, unix 自门控命中 4）
- **symlink** ×2
  - L85: `// Check is_symlink() FIRST — on Unix, a symlink to a directory has both is_symlink() and`
  - L87: `// follows symlinks). Codex checks symlink first so these are rendered with `@`, not `/`.`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build\bash\mod.rs`（163 tests, unix 自门控命中 1）
- **posix-path** ×8（共 8 处，仅列 5）
  - L2223: `output_file: PathBuf::from("/tmp/test.log"),`
  - L2228: `bg_output_file: PathBuf::from("/tmp/bg.log"),`
  - L2242: `output_file: PathBuf::from("/tmp/test.log"),`
  - L2247: `bg_output_file: PathBuf::from("/tmp/bg.log"),`
  - L2276: `bg_output_file: PathBuf::from(format!("/tmp/{}.log", task_id)),`
- **signal** ×3
  - L1329: `- Timeout enforcement: ${%- if auto_background_on_timeout %}when the timeout fires on an explicit `${{ params.execute.is_background }}: true` command, the wrapp`
  - L1345: `- Timeout enforcement: when the timeout fires, the wrapper${%- if is_windows %} terminates the child's Job Object, killing every descendant process immediately `
  - L1754: `// contains the literal pattern), causing the wrapper to SIGTERM itself before the rest of the script runs. Reject`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build\enter_plan_mode\mod.rs`（20 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L599: `let session_plan = PathBuf::from("/home/user/.grok/sessions/abc123/plan.md");`
  - L693: `"/home/user/.grok/sessions/xyz/plan.md",`
  - L708: `assert_eq!(plan_file_path, "/home/user/.grok/sessions/xyz/plan.md");`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build\image_edit\mod.rs`（20 tests, unix 自门控命中 0）
- **posix-path** ×8（共 8 处，仅列 5）
  - L642: `(1, "/tmp/a.png".to_owned()),`
  - L643: `(2, "/tmp/b.png".to_owned()),`
  - L647: `"/tmp/a.png"`
  - L651: `"/tmp/b.png"`
  - L661: `(1, "/tmp/first.png".to_owned()),`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build\kill_task\terminal_command.rs`（5 tests, unix 自门控命中 0）
- **signal** ×1
  - L30: `- ${%- if is_windows %} Terminates the Job Object of${%- else %} Sends SIGTERM/SIGKILL to${%- endif %} a background command${%- if tools.by_kind.monitor %} or m`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build\list_dir\mod.rs`（35 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L1593: `let listed = Path::new("/tmp/listed");`
  - L1620: `let err = map_list_dir_join_error(join_err, Path::new("/tmp/listed"));`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build\send_feedback_tests.rs`（11 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L295: `let drafts = drafts_file_path(std::path::Path::new("/tmp/session"));`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build\task\mod.rs`（76 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L2372: `r#"{"description": "d", "prompt": "p", "cwd": "/tmp/my-worktree"}"#,`
  - L2375: `assert_eq!(input.cwd.as_deref(), Some("/tmp/my-worktree"));`
  - L2898: `assert_eq!(request.cwd.as_deref(), Some("/tmp/some-dir"));`
  - L2922: `cwd: Some("/tmp/some-dir".into()),`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build\task_output\mod.rs`（44 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L1042: `output_file: PathBuf::from(format!("/tmp/{}.log", task_id)),`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build\video_gen\mod.rs`（26 tests, unix 自门控命中 0）
- **posix-path** ×6（共 6 处，仅列 5）
  - L1398: `serde_json::from_str(r#"{"image":"/tmp/source.jpg"}"#).unwrap();`
  - L1407: `r#"{"prompt":"blend these","images":["/tmp/a.jpg","/tmp/b.jpg"],"aspect_ratio":"16:9","duration":"10"}"#,`
  - L1640: `image: "/tmp/source.jpg".into(),`
  - L1659: `images: vec!["/tmp/a.jpg".into(), "/tmp/b.jpg".into()],`
  - L1680: `image: "/tmp/source.jpg".into(),`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build_concise\bash.rs`（6 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L249: `bash.output_file = "/tmp/bg.log".to_string();`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build_hashline\edit\apply.rs`（58 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L766: `PathBuf::from("/tmp/test.rs")`

### `crates\codegen\xai-grok-tools\src\implementations\grok_build_hashline\edit\mod.rs`（20 tests, unix 自门控命中 0）
- **posix-path** ×6（共 6 处，仅列 5）
  - L928: `let path = std::path::PathBuf::from("/tmp/test.txt");`
  - L985: `let path = std::path::PathBuf::from("/tmp/test.txt");`
  - L1036: `let path = std::path::PathBuf::from("/tmp/test.txt");`
  - L1097: `let path = std::path::PathBuf::from("/tmp/test.txt");`
  - L1136: `let path = std::path::PathBuf::from("/tmp/test.txt");`

### `crates\codegen\xai-grok-tools\src\implementations\lsp\config.rs`（2 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L325: `path: PathBuf::from("/home/.grok/lsp.json"),`

### `crates\codegen\xai-grok-tools\src\implementations\lsp\watched_files.rs`（9 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L447: `Path::new("/tmp/fake-nuget/packages/foo/lib.dll"),`
  - L472: `!watched.watches(Path::new("/tmp/elsewhere/app.ts"), FileChangeType::CHANGED),`

### `crates\codegen\xai-grok-tools\src\implementations\opencode\bash\mod.rs`（24 tests, unix 自门控命中 0）
- **posix-path** ×21（共 21 处，仅列 5）
  - L500: `output_file: PathBuf::from("/tmp/test.log"),`
  - L515: `output_file: PathBuf::from("/tmp/test.log"),`
  - L573: `resources.insert(SessionFolder(PathBuf::from("/tmp/session")));`
  - L701: `let session_cwd = std::path::Path::new("/home/user/project");`
  - L704: `let resolved = BashTool::resolve_cwd(session_cwd, Some("/tmp/other"));`
- **signal** ×2
  - L798: `signal: Some("SIGKILL".to_string()),`
  - L818: `assert_eq!(bash.signal, Some("SIGKILL".to_string()));`

### `crates\codegen\xai-grok-tools\src\implementations\opencode\edit\mod.rs`（24 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L563: `let input = make_input("/tmp/denied.txt", "old", "new");`
  - L567: `ToolInput::SearchReplace(ref sr) if sr.file_path == "/tmp/denied.txt"`
  - L570: `assert_eq!(back.file_path, "/tmp/denied.txt");`

### `crates\codegen\xai-grok-tools\src\implementations\opencode\grep\mod.rs`（19 tests, unix 自门控命中 4）
- **posix-path** ×2
  - L408: `path: Some("/tmp/dir".to_string()),`
  - L414: `assert_eq!(back.path.as_deref(), Some("/tmp/dir"));`

### `crates\codegen\xai-grok-tools\src\implementations\opencode\read\mod.rs`（28 tests, unix 自门控命中 3）
- **symlink** ×2
  - L405: `// Resolve symlink to check if target is a directory.`
  - L633: `// (on macOS /tmp is a symlink to /private/tmp).`
- **posix-path** ×2
  - L822: `let json = r#"{"filePath":"/tmp/test.txt","offset":5,"limit":100}"#;`
  - L824: `assert_eq!(input.file_path, "/tmp/test.txt");`

### `crates\codegen\xai-grok-tools\src\implementations\opencode\write\mod.rs`（11 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L334: `let json = r#"{"file_path":"/tmp/test.txt","content":"hello world"}"#;`
  - L336: `assert_eq!(input.file_path, "/tmp/test.txt");`
  - L440: `file_path: "/tmp/test.txt".to_string(),`
  - L466: `file_path: "/tmp/test.txt".to_string(),`

### `crates\codegen\xai-grok-tools\src\implementations\read_file\pdf.rs`（9 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L526: `let path = std::path::Path::new("/tmp/test.pdf");`

### `crates\codegen\xai-grok-tools\src\implementations\skills\discovery.rs`（40 tests, unix 自门控命中 0）
- **posix-path** ×6（共 6 处，仅列 5）
  - L1371: `"/home/u/.cursor/skills/shell/SKILL.md",`
  - L1375: `"/home/u/.cursor/skills/create-rule/SKILL.md",`
  - L1383: `"/home/u/.claude/skills/pdf/SKILL.md",`
  - L1392: `"/home/u/.grok/skills/shell/SKILL.md",`
  - L1400: `"/home/u/.cursor/skills/my-cursor-skill/SKILL.md",`

### `crates\codegen\xai-grok-tools\src\implementations\skills\skill.rs`（52 tests, unix 自门控命中 0）
- **posix-path** ×5
  - L672: `path: "/home/user/.grok/skills/commit/SKILL.md".to_string(),`
  - L700: `<skill name=\"commit\" description=\"Create a git commit\" path=\"/home/user/.grok/skills/commit/SKILL.md\">`
  - L816: `skill_dir: Some("/home/user/.grok/skills/deploy"),`
  - L1206: `path: "/home/user/.grok/skills/commit/SKILL.md",`
  - L1214: `"<skill name=\"commit\" path=\"/home/user/.grok/skills/commit/SKILL.md\"/>"`

### `crates\codegen\xai-grok-tools\src\implementations\skills\types.rs`（6 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L204: `skill_name_from_path("/home/user/.grok/skills/deploy/SKILL.md"),`

### `crates\codegen\xai-grok-tools\src\implementations\task_output\tool.rs`（15 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L104: `output_file: PathBuf::from(format!("/tmp/{}.log", task_id)),`

### `crates\codegen\xai-grok-tools\src\notification\types.rs`（8 tests, unix 自门控命中 0）
- **signal** ×1
  - L72: `/// Signal that terminated the process (e.g., "SIGKILL", "SIGTERM")`

### `crates\codegen\xai-grok-tools\src\persistence.rs`（12 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L75: `state_path: Some(PathBuf::from("/dev/null")),`

### `crates\codegen\xai-grok-tools\src\reminders\task_completion.rs`（59 tests, unix 自门控命中 0）
- **signal** ×10（共 10 处，仅列 5）
  - L157: `itself, or upstream sources of SIGTERM/SIGHUP.\n",`
  - L902: `signal: Some("SIGKILL".into()),`
  - L974: `signal: Some("SIGTERM".into()),`
  - L987: `msg.contains("[monitor ended: killed by signal SIGTERM]"),`
  - L1005: `signal: Some("SIGKILL".into()),`
- **posix-path** ×6（共 6 处，仅列 5）
  - L1267: `task.output_file = std::path::PathBuf::from("/tmp/bg.log");`
  - L1367: `task.output_file = std::path::PathBuf::from("/tmp/bg-unreadable.log");`
  - L1370: `assert!(msg.contains("/tmp/bg-unreadable.log"), "{msg}");`
  - L1379: `task.output_file = std::path::PathBuf::from("/tmp/bg-large.log");`
  - L1557: `output_file: "/tmp/out.log".into(),`

### `crates\codegen\xai-grok-tools\src\types\agents_md_tracker.rs`（25 tests, unix 自门控命中 0）
- **symlink** ×1
  - L165: `// Canonicalize the starting directory. This resolves symlinks and ensures consistent matching against`

### `crates\codegen\xai-grok-tools\src\types\output.rs`（89 tests, unix 自门控命中 0）
- **posix-path** ×36（共 36 处，仅列 5）
  - L1411: `absolute_path: PathBuf::from("/tmp/x.png"),`
  - L1439: `absolute_path: PathBuf::from("/tmp/y.png"),`
  - L1463: `absolute_path: PathBuf::from("/tmp/f.txt"),`
  - L1561: `ToolOutput::ImageGen(MediaGenOutput::new("/tmp/images/1.jpg".into())),`
  - L1563: `"/tmp/images/1.jpg",`

### `crates\codegen\xai-grok-tools\src\types\resources.rs`（59 tests, unix 自门控命中 0）
- **posix-path** ×19（共 19 处，仅列 5）
  - L890: `res.insert(Cwd(PathBuf::from("/home/user")));`
  - L897: `assert!(!json_str.contains("/home/user"));`
  - L1050: `let display = std::path::Path::new("/home/user/project");`
  - L1052: `super::resolve_model_path(cwd, Some(display), "/home/user/project/src/main.rs");`
  - L1061: `let display = std::path::Path::new("/home/user/project");`
- **symlink** ×1
  - L523: `/// canonicalizes the parent directory to handle symlinks (e.g., macOS `/var` → `/private/var`).`

### `crates\codegen\xai-grok-tools\src\types\skill_discovery_tracker\mod.rs`（61 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L1608: `Some("/home/user/project".to_string()),`
  - L1615: `assert!(text.contains("/home/user/project"));`
- **symlink** ×1
  - L146: `/// files or symlink-resolution failures.`

### `crates\codegen\xai-grok-tools\src\types\skill_discovery_tracker\skill_path_suggestion_tests.rs`（9 tests, unix 自门控命中 0）
- **posix-path** ×12（共 12 处，仅列 5）
  - L25: `"/home/user/.grok/skills/code-review/SKILL.md",`
  - L34: `Path::new("/home/user/.grok/skills/code-review/SKILL.md")`
  - L40: `let mut retired = skill("retired", "/home/user/.grok/skills/retired/SKILL.md");`
  - L44: `skill("review", "/home/user/.grok/skills/review/SKILL.md"),`
  - L46: `skill("solo", "/home/user/.grok/skills/solo/SKILL.md"),`

### `crates\codegen\xai-grok-tools\src\types\template_renderer.rs`（25 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L633: `let _ = std::fs::write("/tmp/task_output_tool_description.txt", &rendered);`

### `crates\codegen\xai-grok-tools\src\util\command_display.rs`（5 tests, unix 自门控命中 0）
- **symlink** ×1
  - L4: `//! `canonicalize`d — so `/var` vs `/private/var` or symlink roots miss peel`

### `crates\codegen\xai-grok-tools\src\util\fs.rs`（7 tests, unix 自门控命中 0）
- **symlink** ×2
  - L14: `/// Async symlink-resolved path or the input path on failure/timeout. Windows-safe canonicalizer:`
  - L43: `/// Async symlink-resolved path, preserving the `io::Error` on failure. Error-preserving sibling of`

### `crates\codegen\xai-grok-tools\src\util\git_detect.rs`（11 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L189: `"/usr/local/bin/gh pr create --head my-branch",`
  - L190: `"/opt/homebrew/bin/gh pr create --fill",`
  - L202: `assert!(detect_git_ops("/usr/bin/echo gh pr create", "").is_none());`

### `crates\codegen\xai-grok-tools\src\util\shell_env_policy_tests.rs`（9 tests, unix 自门控命中 0）
- **env-home** ×6（共 6 处，仅列 5）
  - L85: `let env = create_env_from_vars(vars(&[("PATH", "/bin"), ("HOME", "/root")]), &policy);`
  - L88: `assert!(!env.contains_key("HOME"));`
  - L106: `include_only: patterns(&["PATH", "HOME"]),`
  - L112: `("HOME", "/root"),`
  - L119: `assert_eq!(env.get("HOME").map(String::as_str), Some("/root"));`
- **posix-path** ×2
  - L78: `set.insert("PATH".to_string(), "/usr/bin".to_string());`
  - L86: `assert_eq!(env.get("PATH").map(String::as_str), Some("/usr/bin"));`

### `crates\codegen\xai-grok-tools\src\util\vendor.rs`（4 tests, unix 自门控命中 4）
- **symlink** ×2
  - L75: `// Reject a planted symlink instead of following it during verification.`
  - L76: `std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file())`

### `crates\codegen\xai-grok-tools\tests\path_suggestions_production.rs`（19 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L175: `PathBuf::from("/tmp/.tool/sessions/%2Fworkspace%2Frepo/abc-123/terminal/log.txt");`
  - L192: `let display_cwd = PathBuf::from("/home/user/project");`
  - L205: `s.contains("/home/user/project/"),`


## xai-grok-tools-api — CLEAN


## xai-grok-update — COMPILE-BREAK

### `crates\codegen\xai-grok-update\src\auto_update_tests.rs`（133 tests, unix 自门控命中 50）
- **posix-path** ×3
  - L7: `let dest_181 = std::path::Path::new("/home/u/.grok/downloads/grok-0.1.181-linux-x86_64");`
  - L8: `let dest_182 = std::path::Path::new("/home/u/.grok/downloads/grok-0.1.182-linux-x86_64");`
  - L31: `std::path::Path::new("/home/u/.grok/downloads").into(),`
- **symlink** ×1
  - L712: `// A versioned binary written moments ago may be a concurrent installer's just-renamed download whose symlink swap hasn't happened yet`

### `crates\codegen\xai-grok-update\src\version.rs`（8 tests, unix 自门控命中 1）
- **symlink** ×4
  - L424: `/// Returns `None` when there is no parseable managed symlink (Windows copy-based installs, dev builds) or when the`
  - L425: `/// symlink is DANGLING — a link whose target binary was deleted (e.g. manual `~/.grok/downloads` cleanup) must not report`
  - L557: `/// Disk-version probe: parsing the version out of the managed install's symlink-target file name (`grok-<version>-<platform>`).`
  - L572: `("grok-latest", None),                             // symlink alias, not a version`

### `crates\codegen\xai-grok-update\tests\test_blitz_cancel.rs`（5 tests, unix 自门控命中 1）
- **symlink** ×7（共 7 处，仅列 5）
  - L8: `//! The invariant is checked by RE-RESOLVING the symlink and RE-RUNNING the binary from disk every time, never by re-reading a value the harness set.`
  - L43: `/// Seed a previous-good versioned binary and both managed symlinks (`grok` and `agent`; see `swap_managed_bin_links`).`
  - L59: `std::os::unix::fs::symlink(&rel, &link).unwrap();`
  - L73: `/// THE invariant. Re-resolves the on-disk symlink and RE-EXECUTES the resolved binary; never inspects a harness-held value.`
  - L90: `assert!(link.is_symlink(), "{name} must remain a symlink");`
- **perm-mode** ×1
  - L53: `std::fs::set_permissions(&prev, std::fs::Permissions::from_mode(0o755)).unwrap();`

### `crates\codegen\xai-grok-update\tests\test_concurrent_convergence.rs`（10 tests, unix 自门控命中 1）
- **symlink** ×13（共 13 处，仅列 5）
  - L44: `assert!(link.is_symlink(), "grok must be a symlink");`
  - L46: `.unwrap_or_else(|e| panic!("active grok symlink does not resolve: {e}"));`
  - L83: `std::os::unix::fs::symlink(`
  - L230: `// probe must only be trusted for installers that actually maintain the managed `~/.grok/bin/grok` symlink (internal,`
  - L231: `// gh-release). For npm, a symlink left over from a previous internal install LIES about the npm install's version.`
- **perm-mode** ×2
  - L78: `std::fs::set_permissions(`
  - L80: `std::fs::Permissions::from_mode(0o755),`

### `crates\codegen\xai-grok-update\tests\test_downgrade_matrix.rs`（24 tests, unix 自门控命中 0）
- **symlink** ×9（共 9 处，仅列 5）
  - L10: `//! They verify the GCS internal installer actually downloads and symlinks an older binary when the stable pointer is rolled back.`
  - L111: `let symlink = home.join("bin").join("grok");`
  - L112: `let target = std::fs::read_link(&symlink).unwrap();`
  - L115: `"symlink must point to rolled-back version: {target:?}"`
  - L133: `let symlink = test_home().join("bin").join("grok");`

### `crates\codegen\xai-grok-update\tests\test_install_internal.rs`（19 tests, unix 自门控命中 0）
- **symlink** ×15（共 15 处，仅列 5）
  - L4: `//! fetch version, download the grok binary, chmod, atomic symlink, cleanup_old_downloads, persist installer config.`
  - L97: `let symlink = home.join("bin").join("grok");`
  - L98: `assert!(symlink.is_symlink(), "grok symlink created");`
  - L99: `let target = std::fs::read_link(&symlink).unwrap();`
  - L107: `assert!(agent_link.is_symlink(), "agent symlink created");`
- **perm-mode** ×2
  - L228: `use std::os::unix::fs::PermissionsExt;`
  - L244: `assert!(mode & 0o111 != 0, "binary must be executable, got {mode:o}");`
- **unix-import** ×1
  - L228: `use std::os::unix::fs::PermissionsExt;`
- **posix-path** ×1
  - L262: `std::os::unix::fs::symlink("/tmp/fake-old-pager", &pager_link).unwrap();`

### `crates\codegen\xai-grok-update\tests\test_install_sh.rs`（7 tests, unix 自门控命中 1）
- **posix-path** ×14（共 14 处，仅列 5）
  - L140: `let status = Command::new("/bin/bash")`
  - L338: `let ok = run_installer(&install_sh, home.path(), fakedir.path(), mode, "/bin/bash");`
  - L388: `let status = Command::new("/bin/bash")`
  - L394: `.env("SHELL", "/bin/bash")`
  - L430: `let status = Command::new("/bin/bash")`
- **symlink** ×10（共 10 处，仅列 5）
  - L7: `//! Also covers shell-rc rewrite: stowed/symlinked `~/.bashrc` etc. must survive reinstall without being replaced by a plain file.`
  - L104: `/// Seed a valid previous-good binary and symlink in the isolated home.`
  - L115: `std::os::unix::fs::symlink(format!("../downloads/grok-{platform}"), &link).unwrap();`
  - L122: `assert!(link.is_symlink(), "grok must remain a symlink");`
  - L124: `dunce::canonicalize(&link).unwrap_or_else(|e| panic!("grok symlink dangles: {e}"));`
- **env-home** ×4
  - L144: `.env("HOME", home)`
  - L392: `.env("HOME", home.path())`
  - L434: `.env("HOME", home.path())`
  - L509: `.env("HOME", home.path())`
- **perm-mode** ×3
  - L101: `std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();`
  - L112: `std::fs::set_permissions(&prev, std::fs::Permissions::from_mode(0o755)).unwrap();`
  - L373: `std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();`


## xai-grok-version — CLEAN


## xai-grok-voice — COMPILE-BREAK

feature 门控测试（默认构建不跑，抽样不含）：audio

### `crates\codegen\xai-grok-voice\src\audio\capture_linux.rs`（8 tests, unix 自门控命中 0）
- **perm-mode** ×2
  - L129: `use std::os::unix::fs::PermissionsExt;`
  - L136: `.map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)`
- **unix-import** ×1
  - L129: `use std::os::unix::fs::PermissionsExt;`

### `crates\codegen\xai-grok-voice\src\audio\capture_subprocess.rs`（3 tests, unix 自门控命中 0）
- **sh-exec** ×1
  - L311: `let mut cmd = std::process::Command::new("sh");`


## xai-grok-workspace — RISK

feature 门控测试（默认构建不跑，抽样不含）：compression

### `crates\codegen\xai-grok-workspace\src\bin\workspace_server.rs`（26 tests, unix 自门控命中 1）
- **posix-path** ×2
  - L883: `Args::try_parse_from(["xai-workspace-server", "--ready-file", "/tmp/x.ready"]).unwrap();`
  - L884: `assert_eq!(args.ready_file, Some(PathBuf::from("/tmp/x.ready")));`

### `crates\codegen\xai-grok-workspace\src\config.rs`（47 tests, unix 自门控命中 0）
- **symlink** ×1
  - L794: `/// Confine `x.ai/fs/*` / `workspace.fs_*` resolution to the workspace root (reject `..`, absolute-outside-root, symlink escapes).`
- **posix-path** ×1
  - L997: `"/usr/bin/true",`

### `crates\codegen\xai-grok-workspace\src\envrc.rs`（10 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L271: `let mut bash_cmd = Command::new("/bin/bash");`

### `crates\codegen\xai-grok-workspace\src\file_system\attach_file.rs`（13 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L321: `"/home/user/project/src/main.rs",`
  - L322: `file_reference("/home/user/project/src/main.rs", None, None),`

### `crates\codegen\xai-grok-workspace\src\file_system\client_fs.rs`（20 tests, unix 自门控命中 11）
- **symlink** ×6（共 6 处，仅列 5）
  - L6: `//! The list walk excludes symlinks that resolve outside the base and never descends into them.`
  - L85: `/// Resolve a base-relative request path (`""` and `"."` mean the base), rejecting `..` and symlink escapes above the session's client-fs base.`
  - L144: `// Base confinement also holds mid-walk: a symlink inside the base pointing outside must not enumerate metadata of files outside it`
  - L174: `// A walk under a symlinked base yields entries spelled with the canonical path, so strip either spelling`
  - L436: `/// Regression: the list walk must not traverse (or even list) in-root symlinks that resolve outside the workspace root.`
- **posix-path** ×1
  - L682: `for path in ["/etc/passwd", "../escape.txt"] {`

### `crates\codegen\xai-grok-workspace\src\file_system\ext_fs.rs`（12 tests, unix 自门控命中 13）
- **symlink** ×1
  - L164: `// The walk is confined to the canonical root when set (symlinks that escape are not enumerated); `None` (the default) walks unconfined`
- **posix-path** ×1
  - L416: `path: "/etc/passwd".into(),`

### `crates\codegen\xai-grok-workspace\src\file_system\fsmonitor.rs`（6 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L219: `(config_args(), Some(b"/usr/local/bin/watchman\0".to_vec())),`
  - L220: `(typed_config_args("/usr/local/bin/watchman"), None),`

### `crates\codegen\xai-grok-workspace\src\file_system\walk.rs`（3 tests, unix 自门控命中 0）
- **symlink** ×10（共 10 处，仅列 5）
  - L42: `/// When set, a symlink whose canonical target leaves this canonical root is excluded and not descended into.`
  - L48: `/// One raw walk entry; `metadata` follows symlinks (like `fs::metadata`).`
  - L76: `builder.filter_entry(move |dent| symlink_stays_in_root(dent.path(), &canonical_root));`
  - L91: `let is_symlink = std::fs::symlink_metadata(&path)`
  - L108: `/// `true` when `path` is not a symlink, or is a symlink whose canonical target stays under `canonical_root`.`

### `crates\codegen\xai-grok-workspace\src\folder_trust.rs`（57 tests, unix 自门控命中 3）
- **symlink** ×1
  - L566: `// symlink at a vendor hook path must gate too.`
- **posix-path** ×1
  - L1396: `let key = PathBuf::from("/tmp/x");`
- **env-home** ×1
  - L1608: `let _home = EnvVarGuard::set("HOME", home.path());`

### `crates\codegen\xai-grok-workspace\src\fs_notify.rs`（7 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L305: `&PathBuf::from("/home/user/.config/project/src/lib.rs"),`
  - L306: `&PathBuf::from("/home/user/.config/project"),`

### `crates\codegen\xai-grok-workspace\src\git_content_filters.rs`（9 tests, unix 自门控命中 0）
- **symlink** ×1
  - L49: `// Directories open on Linux, so require a readable regular file after following symlinks.`

### `crates\codegen\xai-grok-workspace\src\handle_tests.rs`（224 tests, unix 自门控命中 21）
- **posix-path** ×6（共 6 处，仅列 5）
  - L3572: `.resolve_service_path("/etc/passwd", &canonical_root)`
  - L3749: `.confine_to_root(std::path::Path::new("/etc/passwd"), alt.path())`
  - L3821: `std::path::PathBuf::from("/tmp/test-file.rs"),`
  - L8102: `Some(serde_json::json!({ "cwd": "/tmp/plain" })),`
  - L8111: `assert_eq!(session.cwd(), std::path::Path::new("/tmp/plain"));`
- **symlink** ×2
  - L3647: `/// A *dangling* leaf symlink (target missing, outside root) must be rejected.`
  - L3791: `/// Off by default: a symlink escaping the root is followed, not rejected.`

### `crates\codegen\xai-grok-workspace\src\hub.rs`（31 tests, unix 自门控命中 0）
- **posix-path** ×8（共 8 处，仅列 5）
  - L1075: `output_file: std::path::PathBuf::from("/tmp/x.log"),`
  - L1091: `output_file: std::path::PathBuf::from("/tmp/x.log"),`
  - L1625: `.handle_call(ctx, serde_json::json!({ "path": "/tmp/secret" }))`
  - L1635: `Some(&serde_json::json!("/tmp/secret")),`
  - L1706: `"/tmp/../etc/passwd",`

### `crates\codegen\xai-grok-workspace\src\hub_auth\mod.rs`（21 tests, unix 自门控命中 4）
- **posix-path** ×5
  - L694: `PathBuf::from("/tmp/x"),`
  - L716: `PathBuf::from("/tmp/x"),`
  - L738: `PathBuf::from("/tmp/x"),`
  - L964: `PathBuf::from("/tmp/x"),`
  - L987: `PathBuf::from("/tmp/x"),`

### `crates\codegen\xai-grok-workspace\src\hub_server_tests.rs`（83 tests, unix 自门控命中 4）
- **posix-path** ×1
  - L1685: `"files": [{"path": "/etc/passwd", "content": "evil"}]`

### `crates\codegen\xai-grok-workspace\src\image_capabilities.rs`（14 tests, unix 自门控命中 4）
- **posix-path** ×1
  - L12: `pub const DEFAULT_CAPABILITIES_DIR: &str = "/usr/share/grok/capabilities.d";`
- **symlink** ×1
  - L123: `// `file_type()` deliberately does not follow symlinks: markers are regular files created by `:` redirection in the image build`

### `crates\codegen\xai-grok-workspace\src\path_virtualization_tests.rs`（23 tests, unix 自门控命中 0）
- **posix-path** ×24（共 24 处，仅列 5）
  - L37: `"/home/dev/repo/",`
  - L38: `"/home//dev/.grok/worktrees/repo/cursor-1",`
  - L41: `assert_eq!("/home/dev/repo", v.visible_root());`
  - L42: `assert_eq!("/home/dev/.grok/worktrees/repo/cursor-1", v.real_root());`
  - L44: `"/home/dev/.grok/worktrees/repo/cursor-1/src/main.rs",`

### `crates\codegen\xai-grok-workspace\src\permission\auto_mode\mod.rs`（49 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L402: `"/dev/tcp/",`
  - L1585: `&AccessKind::Edit("/etc/hosts".into()),`
  - L2238: `&AccessKind::Edit("/etc/hosts".into()),`
  - L2239: `Some("/etc/hosts"),`
- **sh-exec** ×2
  - L2077: `assert_eq!(risk("sh -c 'echo hi'"), EnvRisk::Safe);`
  - L2089: `"IFS=x sh -c cmd",`
- **uid-gid** ×1
  - L408: `"chown -r /",`

### `crates\codegen\xai-grok-workspace\src\permission\auto_mode\routine_git_tests.rs`（2 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L39: `"/usr/bin/git checkout main",`

### `crates\codegen\xai-grok-workspace\src\permission\exec_risk.rs`（11 tests, unix 自门控命中 6）
- **posix-path** ×4
  - L540: `"/usr/bin/sort --compress-program=/tmp/pwn in",`
  - L570: `"/usr/bin/git -c core.fsmonitor=/tmp/pwn status",`
  - L616: `"/usr/bin/git status",`
  - L674: `"/usr/bin/command env /usr/bin/git status",`

### `crates\codegen\xai-grok-workspace\src\permission\gate_preflight.rs`（2 tests, unix 自门控命中 4）
- **symlink** ×2
  - L19: `/// A native path hit an unresolvable symlink under deny/ask file rules; blocks YOLO.`
  - L76: `/// Bash-gate Ask, shell-file Ask, or native symlink fail-closed; blocks YOLO.`

### `crates\codegen\xai-grok-workspace\src\permission\hub_permission.rs`（16 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L415: `"filePath": "/tmp/denied.txt",`
  - L420: `Some(AccessKind::Edit(p)) if p == "/tmp/denied.txt"`

### `crates\codegen\xai-grok-workspace\src\permission\managed_policy\tests.rs`（33 tests, unix 自门控命中 0）
- **posix-path** ×19（共 19 处，仅列 5）
  - L12: `const SYS_REQ: &str = "/etc/grok/requirements.toml";`
  - L13: `const USER_REQ: &str = "/home/u/.grok/requirements.toml";`
  - L14: `const SYS_MANAGED: &str = "/etc/grok/managed_config.toml";`
  - L15: `const USER_MANAGED: &str = "/home/u/.grok/managed_config.toml";`
  - L989: `ss("n", "/usr/local/bin/npx"),`

### `crates\codegen\xai-grok-workspace\src\permission\manager\mod.rs`（209 tests, unix 自门控命中 2）
- **posix-path** ×13（共 13 处，仅列 5）
  - L3270: `"/usr/bin/env --split-string='rm -rf /tmp/victim'",`
  - L6088: `"/etc/hosts",`
  - L6089: `"/home/user/.grok/hooks/evil.json",`
  - L6090: `"/home/user/.grok/sandbox.toml",`
  - L7043: `default_always_allow_scope(&words("/usr/bin/gh pr view 1")),`
- **sh-exec** ×4
  - L5511: `let cmd = "curl http://example.com && sh -c 'echo hi'";`
  - L5996: `"sh -c 'LD_PRELOAD=/x ls'",`
  - L8214: `"sh -c 'echo hi'",`
  - L8254: `"sh -c \"$CMD\"",`
- **uid-gid** ×2
  - L500: `|| matches_command_prefix(&joined, "chown")`
  - L7107: `assert!(is_dangerous_command("chown user:group file"));`
- **signal** ×1
  - L7372: `assert!(!matches_command_prefix("trap handler SIGINT", "tr"));`

### `crates\codegen\xai-grok-workspace\src\permission\policy.rs`（61 tests, unix 自门控命中 28）
- **symlink** ×8（共 8 处，仅列 5）
  - L228: `/// Native Read/Edit/Grep also re-check the followed symlink target for deny/ask only (allow on the target is not granted).`
  - L233: `/// Lexical decision plus whether an unresolvable native symlink forces a prompt.`
  - L272: `/// Lexical rule match only. Callers re-check symlink targets through this so follow cannot recurse.`
  - L865: `/// True if any existing component of `absolute` is a symlink.`
  - L874: `if std::fs::symlink_metadata(&prefix).is_ok_and(|meta| meta.file_type().is_symlink()) {`
- **posix-path** ×7（共 7 处，仅列 5）
  - L1228: `"/usr/local/bin/python3 x.py",`
  - L1274: `file_path: "/tmp/denied.txt".into(),`
  - L1838: `"/usr/bin/env -S 'rm -rf /tmp/victim'",`
  - L2001: `("/usr/bin/python3", "/usr/bin/python3 x.py"),`
  - L2230: `"/etc/passwd",`
- **sh-exec** ×1
  - L1757: `for cmd in ["bash -- -c id", "bash script.sh -c id"] {`

### `crates\codegen\xai-grok-workspace\src\permission\prompter.rs`（43 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L2146: `let access = AccessKind::Read(Some("/etc/hosts".to_owned()));`

### `crates\codegen\xai-grok-workspace\src\permission\resolution_tests.rs`（108 tests, unix 自门控命中 0）
- **posix-path** ×16（共 16 处，仅列 5）
  - L1394: `"/etc/claude/managed-settings.json",`
  - L1403: `"/etc/claude/managed-settings.json",`
  - L1432: `let p = Path::new("/etc/grok/requirements.toml");`
  - L1458: `out.contains("/etc/grok/requirements.toml"),`
  - L1568: `path: "/etc/grok/requirements.toml".into(),`
- **env-home** ×6（共 6 处，仅列 5）
  - L332: `let _home_guard = EnvVarGuard::set("HOME", home.path());`
  - L559: `let _real_home_guard = EnvVarGuard::set("HOME", home.path());`
  - L938: `let _home_guard = EnvVarGuard::set("HOME", home.path());`
  - L960: `let _home_guard = EnvVarGuard::set("HOME", home.path());`
  - L1018: `let _home_guard = EnvVarGuard::set("HOME", home.path());`

### `crates\codegen\xai-grok-workspace\src\permission\shell_access.rs`（51 tests, unix 自门控命中 38）
- **posix-path** ×29（共 29 处，仅列 5）
  - L357: `matches!(path, "/dev/null" | "/dev/stdout" | "/dev/stderr")`
  - L1377: `"/home/user/.zshrc",`
  - L1379: `"/etc/grok-test",`
  - L1381: `"/home/user/.grok/sandbox.toml",`
  - L1434: `"/home/user/.grok/hooks/evil.json",`
- **symlink** ×3
  - L204: `// Also re-check the resolved symlink target so a deny keyed on the real path can't be dodged via an in-workspace symlink (`ln -s /etc x`)`
  - L445: `/// Preserves the resolver's uncollapsed components for physical symlink and `..` checks, plus a separate lexical normalization for traversal aliases.`
  - L554: `/// Both forms are checked because callers hold a lexical and a resolved candidate path, and the home itself may sit behind a symlink.`
- **uid-gid** ×1
  - L1158: `/// (`chmod`/`chown` touch metadata, not content.)`
- **sh-exec** ×1
  - L1937: `r#"bash script.sh -c 'cat .env'"#,`

### `crates\codegen\xai-grok-workspace\src\permission\state.rs`（46 tests, unix 自门控命中 3）
- **symlink** ×1
  - L1177: `// Canonicalize BEFORE creation so git2 records the canonical spelling (macOS tempdirs live behind a symlink from /var to /private/var)`

### `crates\codegen\xai-grok-workspace\src\permission\types.rs`（19 tests, unix 自门控命中 0）
- **posix-path** ×7（共 7 处，仅列 5）
  - L782: `file_path: "/tmp/secret.txt".into(),`
  - L787: `matches!(access, AccessKind::Edit(ref p) if p == "/tmp/secret.txt"),`
  - L797: `file_path: "/tmp/denied.txt".into(),`
  - L804: `ToolInput::SearchReplace(sr) if sr.file_path == "/tmp/denied.txt"`
  - L808: `AccessKind::Edit(p) if p == "/tmp/denied.txt"`

### `crates\codegen\xai-grok-workspace\src\session\checkpoint_store.rs`（10 tests, unix 自门控命中 0）
- **symlink** ×1
  - L348: `let Ok(meta) = std::fs::symlink_metadata(&from) else {`

### `crates\codegen\xai-grok-workspace\src\session\file_state.rs`（29 tests, unix 自门控命中 0）
- **posix-path** ×15（共 15 处，仅列 5）
  - L1004: `let root = Path::new("/home/user/project");`
  - L1013: `let abs = FlexiblePath::Absolute(PathBuf::from("/home/user/project/src/file.txt"));`
  - L1027: `let root = Path::new("/home/user/project");`
  - L1032: `FlexiblePath::Absolute(PathBuf::from("/home/user/project/src/main.rs")),`
  - L1068: `"path": "/home/user/project/src/main.rs",`

### `crates\codegen\xai-grok-workspace\src\session\git_restore_code_tests.rs`（64 tests, unix 自门控命中 0）
- **posix-path** ×13（共 13 处，仅列 5）
  - L129: `let worktrees = Path::new("/home/u/.grok/worktrees");`
  - L131: `Path::new("/home/u/.grok/worktrees/home-u-repo/2026-05-22-9f2e51ce"),`
  - L132: `Some("/home/u/repo"),`
  - L138: `let worktrees = Path::new("/home/u/.grok/worktrees");`
  - L140: `Path::new("/home/u/repo"),`

### `crates\codegen\xai-grok-workspace\src\session\git_tests.rs`（94 tests, unix 自门控命中 0）
- **posix-path** ×8（共 8 处，仅列 5）
  - L113: `git_cli(tmp.path(), &["config", "gpg.program", "/bin/false"])`
  - L723: `let result = effective_worktree_cwd("/home/user/.grok/worktrees/repo/ab-123-a", Path::new(""));`
  - L724: `assert_eq!(result, "/home/user/.grok/worktrees/repo/ab-123-a");`
  - L730: `effective_worktree_cwd("/home/user/.grok/worktrees/repo/ab-123-a", Path::new("src"));`
  - L731: `assert_eq!(result, "/home/user/.grok/worktrees/repo/ab-123-a/src");`
- **env-home** ×1
  - L436: `collapse_home_path(path, std::env::var("HOME").ok().as_deref().map(Path::new))`
- **symlink** ×1
  - L841: `// Canonicalize both sides so macOS /private/... symlinks don't trip up the comparison.`

### `crates\codegen\xai-grok-workspace\src\session\mod.rs`（5 tests, unix 自门控命中 5）
- **symlink** ×3
  - L205: `let Ok(meta) = std::fs::symlink_metadata(&src) else {`
  - L215: `if dest.symlink_metadata().is_ok() {`
  - L230: `let Ok(meta) = std::fs::symlink_metadata(&from) else {`
- **signal** ×1
  - L151: `/// Never adopt an externally owned backend into this field: drop/evict would SIGKILL a backend shared with the shell.`

### `crates\codegen\xai-grok-workspace\src\session\tool_config.rs`（23 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L1006: `let expected = PathBuf::from("/tmp/sessions/sess-1");`
  - L1012: `PathBuf::from("/tmp/sessions/sess-1/terminal/call-42.log")`
  - L1017: `let sessions = PathBuf::from("/tmp/sessions");`
  - L1048: `assert_eq!(under_tmp, PathBuf::from("/tmp/sessions/shared-id"));`

### `crates\codegen\xai-grok-workspace\src\status_config.rs`（43 tests, unix 自门控命中 0）
- **signal** ×1
  - L244: `/// The SIGTERM and server-evict paths now use the two-phase drain bounded by `GROK_WORKSPACE_TERMINATION_GRACE_MS`.`

### `crates\codegen\xai-grok-workspace\src\trust.rs`（46 tests, unix 自门控命中 16）
- **symlink** ×5
  - L322: `/// Strict read: a genuine absent entry (`symlink_metadata` `NotFound` / `NotADirectory`) and a successfully read empty/whitespace file are empty documents.`
  - L323: `/// A symlink (even dangling) is never `Missing`: follow-time `NotFound` from `read_to_string` must not let persist replace the link.`
  - L325: `// Probe without following so a dangling symlink is an existing entry, not Missing.`
  - L326: `let link_meta = match std::fs::symlink_metadata(path) {`
  - L1652: `// Canonicalize so macOS's `/var` (a symlink to `/private/var`) agrees between the stored record path and the canonicalized lookup query`

### `crates\codegen\xai-grok-workspace\src\upload\environment.rs`（10 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L306: `Path::new("/tmp/x"),`
  - L379: `Path::new("/tmp/x"),`

### `crates\codegen\xai-grok-workspace\src\util\mod.rs`（1 tests, unix 自门控命中 0）
- **symlink** ×2
  - L14: `/// Uses symlink_metadata so a dangling symlink counts, and treats unreadable as present so EACCES cannot hide a gated entry.`
  - L16: `match std::fs::symlink_metadata(path) {`

### `crates\codegen\xai-grok-workspace\src\workspace_ops.rs`（42 tests, unix 自门控命中 0）
- **env-home** ×4
  - L1849: `let _home = crate::TestEnvGuard::set("HOME", home.path());`
  - L1865: `let _home = crate::TestEnvGuard::unset("HOME");`
  - L1883: `let _home = crate::TestEnvGuard::set("HOME", home.path());`
  - L1902: `let _home = crate::TestEnvGuard::set("HOME", home.path());`
- **posix-path** ×4
  - L2091: `command: Some(std::path::PathBuf::from("/bin/check.sh")),`
  - L2096: `source_dir: std::path::PathBuf::from("/home/u/.grok/hooks"),`
  - L2209: `command: Some(std::path::PathBuf::from("/bin/check.sh")),`
  - L2214: `source_dir: std::path::PathBuf::from("/home/u/.grok/hooks"),`
- **signal** ×1
  - L1457: `/// `cwd` and `hunk_tracker` are only used on first create; a re-bind only replaces the toolset. The session's own terminal backend is never adopted, or teardow`

### `crates\codegen\xai-grok-workspace\src\worktree\identity_tests.rs`（10 tests, unix 自门控命中 2）
- **posix-path** ×5
  - L74: `let worktrees = Path::new("/home/user/.grok/worktrees");`
  - L76: `worktree_identity_in(worktrees, "/home/user/.grok/worktrees"),`
  - L80: `worktree_identity_in(worktrees, "/home/user/.grok/worktrees/xai"),`
  - L87: `let worktrees = Path::new("/home/user/.grok/worktrees");`
  - L89: `worktree_identity_in(worktrees, "/home/user/projects/xai"),`

### `crates\codegen\xai-grok-workspace\src\worktree\mod.rs`（38 tests, unix 自门控命中 0）
- **posix-path** ×13（共 13 处，仅列 5）
  - L63: `let Ok(text) = std::fs::read_to_string("/proc/self/mountinfo") else {`
  - L73: `path.to_string_lossy().contains("/var/lib/grove/")`
  - L140: `"/var/lib/grove/repos/app/worktree"`
  - L195: `"/var/lib/grove/repos/app/worktree"`
  - L197: `assert!(!is_grove_fuse_mount(Path::new("/tmp/not-a-grove-path")));`

### `crates\codegen\xai-grok-workspace\src\worktree\strategy.rs`（15 tests, unix 自门控命中 1）
- **posix-path** ×5
  - L323: `"/dev/fuse or fusermount missing",`
  - L343: `"/dev/fuse or fusermount missing",`
  - L555: `assert!(!text.contains("/dev/fuse"), "{text}");`
  - L624: `"/dev/fuse or fusermount missing",`
  - L639: `assert!(!text.contains("/dev/fuse"), "{text}");`


## xai-grok-workspace-client — CLEAN


## xai-grok-workspace-daemon — RISK

### `crates\codegen\xai-grok-workspace-daemon\src\daemonize.rs`（29 tests, unix 自门控命中 38）
- **pty-fork** ×1
  - L200: `/// `fork()`; the parent exits 0, the child returns `Ok(())` to continue.`
- **symlink** ×1
  - L213: `/// On Unix it adds `O_NOFOLLOW` and mode `0600` as symlink and permission defense-in-depth; the per-tenant sandbox namespace is the primary control.`

### `crates\codegen\xai-grok-workspace-daemon\src\preview_supervisor.rs`（37 tests, unix 自门控命中 17）
- **posix-path** ×3
  - L26: `pub const PREVIEW_PROXY_BIN_PATH: &str = "/usr/local/bin/xai-grok-preview-proxy";`
  - L31: `pub const PREVIEW_PROXY_LOG_PATH: &str = "/var/tmp/workspace-server/tmp/preview-proxy.log";`
  - L1485: `let mut cmd = tokio::process::Command::new("/bin/sh");`
- **signal** ×2
  - L10: `//! - `PR_SET_PDEATHSIG(SIGKILL)` binds the child's lifetime to the workspace-server so a WS crash cannot orphan the proxy holding the ports.`
  - L354: `// SIGKILL on teardown: preview is best-effort and the container is going away`
- **symlink** ×1
  - L187: `/// It reuses the daemon file options (`O_NOFOLLOW` and mode `0600` on Unix) for the same symlink and permission defense as the workspace-server log.`


## xai-grok-workspace-types — RISK

### `crates\codegen\xai-grok-workspace-types\src\rpc\fs.rs`（5 tests, unix 自门控命中 0）
- **symlink** ×2
  - L351: `/// Follow symlinks while walking.`
  - L381: `/// `Some(true)` when the entry itself is a symlink; omitted otherwise.`

### `crates\codegen\xai-grok-workspace-types\src\rpc\hooks.rs`（4 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L196: `"command": "/bin/check.sh",`
  - L201: `"source_dir": "/home/u/.grok/hooks",`

### `crates\codegen\xai-grok-workspace-types\src\rpc\hunks.rs`（6 tests, unix 自门控命中 0）
- **symlink** ×1
  - L219: `"symlink" => Self::Symlink,`

### `crates\codegen\xai-grok-workspace-types\src\rpc\repos.rs`（8 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L263: `mount_path: "/tmp/evil".into(),`

### `crates\codegen\xai-grok-workspace-types\tests\wire_round_trip.rs`（23 tests, unix 自门控命中 0）
- **posix-path** ×5
  - L35: `input_json: r#"{"path":"/etc/hosts"}"#.into(),`
  - L108: `command: Some("/usr/bin/fs-mcp".into()),`
  - L112: `cwd_override: Some("/tmp/work".into()),`
  - L706: `input_json: r#"{"path":"/tmp/x"}"#.into(),`
  - L713: `r#"{"type":"need_permission","data":{"req_id":"perm-1","request":{"tool_name":"rm","summary":"deletes a file","input_json":"{\"path\":\"/tmp/x\"}","destructive"`


## xai-hooks-plugins-types — RISK

### `crates\codegen\xai-hooks-plugins-types\src\lib.rs`（20 tests, unix 自门控命中 0）
- **posix-path** ×6（共 6 处，仅列 5）
  - L667: `path: "/tmp/hooks".into(),`
  - L671: `assert!(json.contains(r#""path":"/tmp/hooks""#));`
  - L719: `source_dir: "/home/user/.grok/hooks".into(),`
  - L738: `root: "/home/user/.grok/plugins/test-plugin".into(),`
  - L813: `"root": "/tmp/future-plugin",`


## xai-hunk-tracker — RISK

### `crates\codegen\xai-hunk-tracker\src\actor\file_utils.rs`（17 tests, unix 自门控命中 0）
- **symlink** ×3
  - L96: `// Use symlink_metadata (lstat) to detect symlinks without following them.`
  - L99: `let metadata = match tokio::fs::symlink_metadata(path).await {`
  - L301: `std::os::unix::fs::symlink(&target, &link).unwrap();`

### `crates\codegen\xai-hunk-tracker\src\actor\tests.rs`（120 tests, unix 自门控命中 0）
- **posix-path** ×11（共 11 处，仅列 5）
  - L4367: `let path = PathBuf::from("/tmp/a.rs");`
  - L4383: `let path = PathBuf::from("/tmp/a.rs");`
  - L4398: `let path = PathBuf::from("/tmp/a.rs");`
  - L4412: `let path = PathBuf::from("/tmp/a.rs");`
  - L4426: `let path = PathBuf::from("/tmp/a.rs");`

### `crates\codegen\xai-hunk-tracker\src\loc\tests.rs`（19 tests, unix 自门控命中 0）
- **posix-path** ×11（共 11 处，仅列 5）
  - L19: `path: PathBuf::from("/tmp/foo.rs"),`
  - L38: `path: PathBuf::from("/tmp/bar.rs"),`
  - L57: `path: PathBuf::from("/tmp/del.rs"),`
  - L149: `assert_eq!(record.file_path, PathBuf::from("/tmp/foo.rs"));`
  - L278: `path: PathBuf::from("/tmp/foo.rs"),`

### `crates\codegen\xai-hunk-tracker\src\types.rs`（12 tests, unix 自门控命中 0）
- **symlink** ×4
  - L438: `pub fn symlink() -> Self {`
  - L465: `FileContentState::Symlink => Self::symlink(),`
  - L597: `/// and canonicalized prefix variants to handle macOS symlinks (e.g., `/var` → `/private/var`) and paths stored with vs.`
  - L598: `/// without symlink resolution. Returns `None` if the path cannot be made relative to `old_cwd` under any prefix variant.`
- **posix-path** ×1
  - L693: `let old_raw = std::path::Path::new("/var/folders/work");`


## xai-interjection-core — CLEAN


## xai-message-delivery-core — CLEAN


## xai-mixpanel — CLEAN


## xai-prompt-queue — CLEAN


## xai-proto-build — CLEAN


## xai-ratatui-inline — CLEAN

feature 门控测试（默认构建不跑，抽样不含）：scrolling-regions


## xai-ratatui-textarea — CLEAN


## xai-sqlite-journal — CLEAN


## xai-system-power — CLEAN


## xai-test-utils — RISK

### `crates\common\xai-test-utils\src\git.rs`（2 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L110: `if cfg!(windows) { "NUL" } else { "/dev/null" },`
  - L143: `run_git(dir, &["config", "core.excludesFile", "/dev/null"]);`


## xai-token-estimation — CLEAN


## xai-tool-protocol — RISK

### `crates\common\xai-tool-protocol\src\frames.rs`（43 tests, unix 自门控命中 0）
- **posix-path** ×4
  - L1951: `"cwd": "/home/me/proj",`
  - L1958: `assert_eq!(parsed.cwd.as_deref(), Some("/home/me/proj"));`
  - L1971: `"cwd": "/home/me/proj",`
  - L1981: `cwd: Some("/home/me/proj".to_owned()),`
- **signal** ×1
  - L1108: `/// Epoch ms when a graceful drain began (SIGTERM or hub evict); `None``

### `crates\common\xai-tool-protocol\tests\jsonrpc_envelope.rs`（14 tests, unix 自门控命中 0）
- **posix-path** ×1
  - L221: `arguments: json!({"path": "/etc/hosts"}),`

### `crates\common\xai-tool-protocol\tests\serde_roundtrip.rs`（69 tests, unix 自门控命中 0）
- **posix-path** ×2
  - L956: `cwd: Some("/tmp/test".to_owned()),`
  - L961: `assert_eq!(v["cwd"], json!("/tmp/test"));`


## xai-tool-runtime — RISK-LIGHT

### `crates\common\xai-tool-runtime\tests\notification_serde.rs`（17 tests, unix 自门控命中 0）
- **posix-path** ×3
  - L74: `output_file: PathBuf::from("/tmp/out.log"),`
  - L98: `absolute_path: PathBuf::from("/tmp/x"),`
  - L118: `output_file: PathBuf::from("/tmp/out"),`


## xai-tool-types — RISK

feature 门控测试（默认构建不跑，抽样不含）：prompt-render

### `crates\common\xai-tool-types\src\task.rs`（31 tests, unix 自门控命中 0）
- **signal** ×2
  - L1194: `"Sends SIGTERM/SIGKILL to"`
  - L1740: `- Sends SIGTERM/SIGKILL to a bash task or monitor; sends Cancel+Shutdown to a subagent.\n\`


## xai-tracing — CLEAN


## xai-tty-utils — COMPILE-BREAK

### `crates\codegen\xai-tty-utils\src\lib.rs`（32 tests, unix 自门控命中 39）
- **signal** ×2
  - L358: `/// SIGTERM (the default) lets the child run its graceful shutdown; pass `libc::SIGKILL` when the child must die even if`
  - L360: `/// because they were unresponsive to the master-close SIGHUP. All caveats of [`kill_on_parent_death_std`] apply.`
- **posix-path** ×1
  - L1169: `let osrelease = std::fs::read_to_string("/proc/sys/kernel/osrelease").ok();`

### `crates\codegen\xai-tty-utils\src\process_scope.rs`（8 tests, unix 自门控命中 1）
- **signal** ×1
  - L153: `/// Idempotently kill every still-owned process tree (`killpg(SIGKILL)` / `TerminateJobObject`). Safe to call multiple`

### `crates\codegen\xai-tty-utils\src\runtime_eagain_tests.rs`（2 tests, unix 自门控命中 0）
- **unix-import** ×1
  - L17: `use std::os::unix::process::ExitStatusExt;`
- **posix-path** ×1
  - L26: `std::fs::read_to_string("/proc/self/status")`
- **uid-gid** ×1
  - L42: `libc::setgid(NOBODY) == 0 && libc::setuid(NOBODY) == 0`


## xai-workflow — RISK

### `crates\codegen\xai-workflow\src\journal.rs`（16 tests, unix 自门控命中 3）
- **symlink** ×1
  - L256: `let metadata = std::fs::symlink_metadata(path)?;`

