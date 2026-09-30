# CI 缓存事故：Windows release 从 9 分钟变 57 分钟

2026-09-16。仓库 `daidaiJ/grok-build-proxy`。这份记录把 fork 上 GitHub Actions 缓存怎么一步步走歪、数字是怎么测出来的、最后为什么改回 `Swatinem/rust-cache` 写清楚，方便下次 rebase / 再动 CI 时对照。

## 背景

上游 grok-build 的 CI 跑在内部 runner 上，带 DotSlash 工具链和内部 secrets，fork 用不了。phase-1（`f5ad7a5`，2026-09-15）自己加了两份工作流：

- `build.yml`：push main / PR。Linux 测 fork 动过的 crate，Windows 只 check + 跑可移植测试。
- `release.yml`：推 `v*` tag。构建 `xai-grok-pager-bin`（grok CLI），产出 linux-amd64 `.tar.gz` 和 windows-amd64 `.zip`，附 sha256。

工作区大约 100 个 crate。Windows 冷编译 release 依赖树，实测约 50 分钟。Linux 同一次大约 9 分钟。瓶颈一直在 Windows。

`bin/protoc` 是 DotSlash 占位脚本，CI 上要自己装 protoc 31.1。Chocolatey 源给过一次 504 却退出码 0，后面才报 `protoc not found`，所以改成 GitHub release 直下（`bc2c8ea`）。

## 时间线（都在 2026-09-15 ~ 09-16）

| 提交 | 做了什么 |
|---|---|
| `f5ad7a5` | 搭 build / release。两边 job 都叫 `linux` / `windows`。`Swatinem/rust-cache` 默认按 job 名做 key。 |
| `7a6d298` | 发现 release 的 rust-cache 命中了 build 的 **debug** `target/`，对 `--release` 没用。改接 sccache + GitHub Actions 后端，release 关掉 `cache-targets`。指望内容寻址跨工作流共享。 |
| `fd81836` | cargo 的 profile 环境变量只认 `"true"` / `"false"`，写成 `0`/`1` 不生效。 |
| `7988433` | 把 sccache 服务端错误打到日志。 |
| `bc2c8ea` | Windows protoc 从 choco 换成 pinned zip。 |
| `abc2766` | Windows：关 Defender 实时扫描；最终链接改 `rust-lld`（v1.0.27 上 `link.exe` 单这一步约 12 分钟）；toolchain 钉死 1.94.0；sccache 命中率低于 cargo `Compiling` 行数的 90% 就 fail。 |
| `8c99e92` | tag 构建注入 `GROK_VERSION`（`v1.0.27` → `1.0.27`），否则二进制回落到 Cargo.toml 里过期的 lockstep 版本。 |
| `db865e4` | 拆掉 sccache GHA 后端。rust-cache 用 `shared-key: build` / `release` 隔开。release 重新缓存 `target/`。 |

`abc2766` 的 Defender / rust-lld / 1.94.0 / `GROK_VERSION` 都留着，只是把 sccache 那一层拿掉。

## 第一次误判：job 名撞车

`Swatinem/rust-cache` 的 key 默认带 `GITHUB_JOB`（[README：Cache Details][swatinem-readme]；实现见 [`src/config.ts`][swatinem-key]：无 `shared-key` 时拼上 `GITHUB_JOB`，再加 `os.type()` / `os.arch()`）。两个工作流的 job 都叫 `linux` 和 `windows`，lockfile 和 toolchain 又一样，所以 **release 会把 build 刚存的 debug `target/` 整包还原回来**。

cargo 看 fingerprint：debug 产物帮不上 `--release`。Windows 每次打 tag 都把依赖树重编一遍，约 50 分钟。日志上 rust-cache 显示 hit，实际零收益。

当时的对策写在 `7a6d298`：sccache 按编译单元内容寻址，debug/release 不该互相污染；rust-cache 只留 `~/.cargo` 注册表（`cache-targets: false`），省 10 GB 配额。`workflow_dispatch` 用来在打 tag 前预热。

这个诊断（job 名撞车）是对的。选的药不对。

## 第二次误判：sccache 的 GHA 后端

mozilla/sccache 接 GitHub Actions cache 时，**每个编译单元单独写一条 cache**，不是 `@actions/cache` 那种一个 key 一个 tar。原文在 0.3 时代的 README / [PR #1528 写进 `docs/GHA.md` 的段落][sccache-gha-separate]：

> In contrast to the `@actions/cache` action, which saves a single large archive per cache key, `sccache` with GHA cache storage saves each cache entry separately. [...] These GHA caches are differentiated by their *version*.

维护者后来把这段从 `GHA.md` 里挪走了，行为没变：issue [#1762][sccache-1762]（2023，仍 open）写的是「caches each compilation result as a separate artifacts on GHA」，key 形态就是本仓库扫到的 `sccache/d/d/e/<hash>`。现行 [`docs/GHA.md`][sccache-gha] 只保留了一句：打到 GitHub cache 速率上限时，**构建继续、存储可能不写**。

本仓库 2026-09-16 中午用 `gh api --paginate /actions/caches` 扫到：

| | 条数 | 体积 |
|---|---|---|
| `sccache/<hash>` 碎片 | 5229 | 6090.6 MiB |
| rust-cache（5 条，linux/windows 混着 debug 残留） | 5 | 约 4.5 GiB |
| **合计** | **5234** | **10.6 GiB** |

GitHub 文档：[每个仓库缓存默认 10 GB][gha-cache-limit]，7 天未访问删除；超限后保存新条目，再按**上次访问时间从旧到新**逐出，直到回到上限以下。debug 构建天天跑、碎片天天写，release 那批对象最先被挤掉。

sccache 的 Rust hash 吃 rustc 可执行文件路径、host triple、sysroot、**解析后的 rustc 参数**（[`docs/Caching.md`][sccache-hash]）。`--release` 和 dev 的 opt-level、codegen-units、debuginfo 全不同，**debug 跑出来的缓存对 release 命中率就是 0**。两个工作流在抢同一份 10 GB，彼此帮不上。

sccache 自己也写了：增量编译的 crate **不能**进缓存；CI 必须关掉 incremental（[README Known Caveats][sccache-caveats]）。Cargo 侧对应的是 `CARGO_INCREMENTAL=0`（[Cargo Book：环境变量][cargo-incremental]）或 `CARGO_PROFILE_<name>_INCREMENTAL`（[boolean，见 config][cargo-profile-incr]）。`fd81836` 踩过：写成 `0`/`1` 时 profile 覆盖不生效，Cargo 要 `true`/`false`。

## 实测（都是 2026-09-16 的 run）

**release `35078943587`（tag v1.0.27）**

- Linux：09:21:27 → 09:30:32，约 9 分钟
- Windows：09:21:28 → 10:18:21，约 57 分钟
- Windows sccache：Compile requests 1264，executed 1079，**hits 0，misses 1074，命中率 0.00%**，平均 cache write 0.437 s

0.437 s × 1074 ≈ 8 分钟纯浪费在往已经满了的 GHA 缓存里写碎片。

**build `35076697980`（main，同一天早些）**

- Linux：约 14 分钟。rust-cache 还原 2835 MB（key `v0-rust-linux-Linux-x64-ecb9a127-61ebc5b7`）。sccache 399 hit / 4 miss，**99.01%**
- Windows：约 6 分钟（只 check + 三个可移植 crate 的 test）

Linux debug 的 99% 是真的：同一 profile、频繁跑、碎片还在。它掩盖了 release 完全 miss 这件事。

**v1.0.27 Windows 另一次（`abc2766` 的 commit message）**

cargo 编了 991 个 crate，sccache 服务端只看到 4 次请求，内存里的 stats 还报「100% - 3 hits」。服务在构建中途重置过。所以后来加了「sccache 请求数 < cargo Compiling 行数 × 90% 就 fail」。sccache 已经不可信之后打的补丁，治标。

**build `35093410275`（main，当天中午）**

- Windows：6 分钟，成功
- Linux：12:00:50 开工，卡在 `xai-grok-shell` 的 `cancel_drops_queued_spawns_before_coordinator`（12:24:13 开始报 running over 60 seconds），13:01:12 被 60 分钟 timeout 取消。post 步骤里 sccache stats 全 0（又一次服务端空了）。这是测试死锁，不是缓存问题，但同一天把 Linux job 空转了 37 分钟。

## 社区对照（2026-09-16 查的）

| 项目 | CI 缓存 | 出处 |
|---|---|---|
| Helix | 复合 action：`dtolnay/rust-toolchain` + rust-cache，`shared-key` 隔离。Windows / Linux 各自 native runner。release dist job 不缓存 `target/` | [`rust-setup/action.yml`][helix-setup]、[`release.yml` dist][helix-release] |
| clap | rust-cache，无 sccache GHA；`CARGO_INCREMENTAL=0`；`CARGO_PROFILE_DEV_DEBUG: line-tables-only` | [`ci.yml`][clap-ci] |
| starship | rust-cache；`CARGO_INCREMENTAL=0`、`CARGO_NET_RETRY=10`、`RUSTUP_MAX_RETRIES=10` | [`workflow.yml`][starship-ci] |
| Tauri | rust-cache，`key` 带 target triple | [`lint-rust.yml`][tauri-lint] |
| Polars | rust-cache，`save-if: github.event_name == 'push'`（PR 只读） | [`test-python.yml`][polars-ci] |
| rust-analyzer | `CARGO_INCREMENTAL=0`、`CARGO_NET_RETRY=10`、concurrency 取消旧 run。rust-cache **整段注释掉** | [`ci.yaml`][ra-ci] |

sccache 官方 [`GHA.md`][sccache-gha]：打到速率上限就继续编，存储可能不写。Depot 的评测（[2025-03-06][depot-sccache]）和 Linera 的事故单（[#5475][linera-5475]，2026-02）同一句话：sccache 接 GHA 后端还是那 10 GB 和分支隔离，真要跨 job 共享编译单元得上 S3/R2。

Windows 关 Defender：GitHub 官方 Windows runner **镜像构建脚本**就写了 `DisableRealtimeMonitoring = $true`（[`actions/runner-images` `Configure-WindowsDefender.ps1`][gha-defender]）。镜像里关过不等于 job 运行时还关着（[#14326][gha-defender-bug] 指出 Tamper Protection 会让部分 `Set-MpPreference` 静默失败），所以 release job 仍显式调一次。`abc2766` 里写「rust-lang 自己的 CI 也这么做」我没在 `rust-lang/rust` 的 workflow 里核对到对应步骤，以 runner-images 这份为准。

对这个 fork：没有 S3，工作区大，release 不频繁。正确做法是 **每 OS、每 profile 一块 rust-cache**，总量压进 10 GB。

## 最终方案（`db865e4`）

`.github/actions/ci-setup`：Windows 关 Defender → 钉死 rustc 1.94.0 → rust-cache → 装 protoc 31.1。

rust-cache：

- `prefix-key: v1-rust`（丢掉旧的 `v0-rust` 混装包）
- `shared-key: build` 或 `release`（不再用撞车的 job 名）
- `cache-targets: true`（release 重新存 `target/release`）
- `cache-on-failure: true`（测试挂了编译结果还在）
- `save-if`：只在 `main` 或 tag 上写，PR 只读

两边工作流：`CARGO_INCREMENTAL=0`、`CARGO_NET_RETRY=10`、concurrency 取消同 ref 旧 run。`cargo test` 已经覆盖的 crate 不再先 `cargo check`。Linux shell 测试 step timeout 20 分钟，避免再被那个死锁测试吃满 60 分钟。

Windows release 仍用 `rust-lld`（`CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER`），本地 dev 继续 `link.exe`。tag 构建仍注入 `GROK_VERSION`。

配额估算（sccache 碎片清掉之后）：

| blob | 大约 |
|---|---|
| linux-build `target/` | 2.8 GB（已测） |
| windows-build | 0.8 GB（已测） |
| linux-release | 1.5–2 GB |
| windows-release | 1.5–2 GB |
| 合计 | 约 7–8 GB，进得去 10 GB |

第一次跑会冷。冷跑存盘之后，同 lockfile 的下一次 tag 只重编改过的 crate。Windows 不应再接近 50 分钟。

## v1.0.29：修好投毒之后仍然是冷编译（2026-09-16 晚）

`97314a9` 之后的 tag `v1.0.29`（run `35130156625`）日志写的是 **No cache found**。Windows `cargo build --release` **991 crate / 46m 51s**，Linux 27m 21s。post 才把 `v1-rust-release-*` 各存了一份（Windows 1.13 GiB，Linux 1.16 GiB）。所以这次慢不是 rust-cache 失效，是投毒修复后的第一次完整存盘。

同一份日志还说明两件即使命中依赖缓存也快不了多少的事：

1. rust-cache 默认 `cache-workspace-crates: false`。checkout 把源码 mtime 设成 now，cargo 按 mtime 判定 workspace 脏了。96 个 workspace crate 每次都重编；Windows 上从 `xai-grok-pager` 开始编到 `Finished` 还有约 13 分钟（最终链接）。
2. `windows-latest` 编同一棵树比 `ubuntu-24.04` 慢将近一倍。依赖缓存救不了链接器。

所以下一轮不再在 Windows runner 上 native 编：Ubuntu 上 `cargo xwin` 交叉出 `x86_64-pc-windows-msvc`，release 打开 `cache-workspace-crates`，checkout `fetch-depth: 0` 之后 `git-restore-mtime`。native Windows 那份 1.13 GiB blob 交叉跑通后再删。

## 打 tag 前

```bash
gh workflow run release.yml --ref main
gh run watch
```

`workflow_dispatch` 走同一套 `--release` 构建，但不上传 GitHub Release。用它给 `shared-key: release` 预热。

## 同一天没修的

- `session::workflow::manager::tests::cancel_drops_queued_spawns_before_coordinator` 在 Linux CI 上死锁过一次。现在只靠 20 分钟 step timeout 止损，测试本身没改。
- `gh cache delete --all` 在 `db865e4` 推上去之后开始清 5000+ 条 `sccache/` 碎片。新工作流用 `v1-rust-*` key，不会去 restore 那些碎片；配额要等删除跑完才真正腾出来。
- Helix 那种 `git-restore-mtime` / `cache-workspace-crates` 已在 release.yml 上。rust-cache 默认仍不缓存 workspace crate：README 写「generally not effective」（[Cache Details][swatinem-readme]，讨论见 [#37][swatinem-37]），因为 checkout 把 mtime 设成 now。release 用 `chetan/git-restore-mtime-action@v2` 把 mtime 拉回最后一次提交时刻，workspace 产物才能复用。build.yml 不缓存 `target/`，所以没做。

## 依据与出处

官方文档（规范本身）：

| # | 说了什么 | 链接 |
|---|---|---|
| 1 | 仓库 Actions 缓存默认 10 GB；超限按 last-accessed 从旧到新逐出；7 天未访问删除 | [GitHub Docs: Usage limits and eviction policy][gha-cache-limit]（[中文][gha-cache-limit-zh]） |
| 2 | 各计划「缓存存储 / 每仓库」都是 10 GB | [GitHub Docs: Actions 限制][gha-limits-zh] |
| 3 | rust-cache 的 key 组成、`shared-key` 替换 job 名、默认不缓存 workspace crate、10 GB 限制沿用 GitHub | [Swatinem/rust-cache README][swatinem-readme]、[`src/config.ts`][swatinem-key] |
| 4 | sccache GHA：速率上限时构建继续、存储可能失败 | [mozilla/sccache `docs/GHA.md`][sccache-gha] |
| 5 | sccache GHA：每个编译结果一条独立 cache（旧文档原文 + 仍 open 的 issue） | [PR #1528 批注][sccache-gha-separate]、[#1762][sccache-1762] |
| 6 | sccache Rust hash 含 rustc 参数；增量编译不可缓存 | [`docs/Caching.md`][sccache-hash]、[README Known Caveats][sccache-caveats] |
| 7 | `CARGO_INCREMENTAL=0/1`；`CARGO_PROFILE_<name>_INCREMENTAL` 是 boolean | [Cargo Book: Environment Variables][cargo-incremental]、[Configuration: profile incremental][cargo-profile-incr] |
| 8 | `CARGO_TARGET_<triple>_LINKER` 覆盖链接器 | [Cargo Book: `target.<triple>.linker`][cargo-linker] |
| 9 | GitHub Windows runner 镜像构建时关 Defender 实时扫描 | [`Configure-WindowsDefender.ps1`][gha-defender] |

大项目工作流（对照，不是规范）：

| 项目 | 文件 |
|---|---|
| Helix | [`.github/actions/rust-setup/action.yml`][helix-setup]、[`.github/workflows/release.yml`][helix-release] |
| clap | [`.github/workflows/ci.yml`][clap-ci] |
| starship | [`.github/workflows/workflow.yml`][starship-ci] |
| Tauri | [`.github/workflows/lint-rust.yml`][tauri-lint] |
| Polars | [`.github/workflows/test-python.yml`][polars-ci] |
| rust-analyzer | [`.github/workflows/ci.yaml`][ra-ci] |

评测 / 事故单：

| 来源 | 结论 |
|---|---|
| [Depot, 2025-03-06][depot-sccache] | GHA 当 CAS 用：慢、10 GB、每个 rustc 调一次 cache API |
| [Linera #5475, 2026-02][linera-5475] | rust-cache 100% miss；sccache+GHA **帮不上**（同一套 cache API 和分支隔离）；要跨分支共享得上 S3 |

本仓库第一手数字：

| 来源 | 内容 |
|---|---|
| `gh api --paginate /repos/daidaiJ/grok-build-proxy/actions/caches`，2026-09-16 | 5234 条 / 10.6 GiB，其中 5229 条 `sccache/` |
| Actions run `35078943587`（release，v1.0.27） | Windows 57 min，sccache 0 hit / 1074 miss |
| Actions run `35076697980`（build，main） | Linux sccache 99.01%；rust-cache 还原 2835 MB |
| Actions run `35093410275` | Linux 死锁 `cancel_drops_queued_spawns_before_coordinator`，timeout 60 min |
| 提交 `7a6d298`、`abc2766`、`db865e4` | 三次对策的 commit message |

[gha-cache-limit]: https://docs.github.com/en/actions/using-workflows/caching-dependencies-to-speed-up-workflows#usage-limits-and-eviction-policy
[gha-cache-limit-zh]: https://docs.github.com/zh/actions/using-workflows/caching-dependencies-to-speed-up-workflows#usage-limits-and-eviction-policy
[gha-limits-zh]: https://docs.github.com/zh/actions/reference/limits
[swatinem-readme]: https://github.com/Swatinem/rust-cache/blob/master/README.md
[swatinem-key]: https://github.com/Swatinem/rust-cache/blob/master/src/config.ts
[swatinem-37]: https://github.com/Swatinem/rust-cache/issues/37
[sccache-gha]: https://github.com/mozilla/sccache/blob/main/docs/GHA.md
[sccache-gha-separate]: https://github.com/mozilla/sccache/pull/1528
[sccache-1762]: https://github.com/mozilla/sccache/issues/1762
[sccache-hash]: https://github.com/mozilla/sccache/blob/main/docs/Caching.md
[sccache-caveats]: https://github.com/mozilla/sccache/blob/main/README.md#known-caveats
[cargo-incremental]: https://doc.rust-lang.org/cargo/reference/environment-variables.html
[cargo-profile-incr]: https://doc.rust-lang.org/cargo/reference/config.html#profilenameincremental
[cargo-linker]: https://doc.rust-lang.org/cargo/reference/config.html#targettriplelinker
[gha-defender]: https://github.com/actions/runner-images/blob/main/images/windows/scripts/build/Configure-WindowsDefender.ps1
[gha-defender-bug]: https://github.com/actions/runner-images/issues/14326
[helix-setup]: https://github.com/helix-editor/helix/blob/master/.github/actions/rust-setup/action.yml
[helix-release]: https://github.com/helix-editor/helix/blob/master/.github/workflows/release.yml
[clap-ci]: https://github.com/clap-rs/clap/blob/master/.github/workflows/ci.yml
[starship-ci]: https://github.com/starship/starship/blob/master/.github/workflows/workflow.yml
[tauri-lint]: https://github.com/tauri-apps/tauri/blob/dev/.github/workflows/lint-rust.yml
[polars-ci]: https://github.com/pola-rs/polars/blob/main/.github/workflows/test-python.yml
[ra-ci]: https://github.com/rust-lang/rust-analyzer/blob/master/.github/workflows/ci.yaml
[depot-sccache]: https://www.depot.dev/blog/sccache-in-github-actions
[linera-5475]: https://github.com/linera-io/linera-protocol/issues/5475

