# Windows 测试门控体系（WIN-TEST-GATE）

> 定稿 2026-09-19。前提事实：上游基线是 Linux CI，~90% 测试用例与 Linux 行为耦合
> （POSIX 路径/权限/线程栈/终端探测/信号/pty）。**本机不是上游套件的运行环境**，
> 逐个排查是无底洞——本体系的思路是分层把"不该在本机跑的"挡在入口、把"能批量
> 救活的"用零成本手段救活，让每一次测试失败都自带分诊结论而不是重新考古。

## 流水线（先廉价发现，后按需深挖）

| 阶段 | 动作 | 成本 | 产出 |
|---|---|---|---|
| 0 特征发现 | `win_scan.py` 静态扫描全部测试代码（零编译） | 秒级 | `win-compat-scan.md`：COMPILE-BREAK / RISK / RISK-LIGHT / CLEAN 四档 + feature 名单 |
| 1 特征门控 | 按扫描档位补门控 | 零 | COMPILE-BREAK crate 测试目标本机永不跑（白名单天然拦截）；RISK 高危族登记 `win-skip.txt` |
| 2 抽样 | `win_sample.py` 幸存 crate 每 N 取 1 跑 | 每 crate 一轮编译 | 台账 `win-compat-ledger.md`：模块级 PASS/FAIL/HANG |
| 2.5 深排 | **只对抽样失败的模块**做 T0 A/B 归因 | 按需 | 环境族 → 登记 skip；真产品 bug → 修并登记 PATCHES.md |
| 3 速读 | 抽样 PASS 的模块快速读测试代码 | 每模块分钟级 | 读出静态特征没覆盖的 Linux 耦合（隐式 HOME 依赖、fork 语义等）→ skip 候选 |
| 4 分模块分诊 | 剩余不确定模块按 crate 逐个过 | 专用会话 | 终局结论「win 可信 / 需门控」→ 可信者晋升白名单 |

**抽样 PASS ≠ win 兼容**，只是"未见异常"；晋升白名单必须同时过阶段 3 速读。
`#[cfg(unix)]` 自门控的测试在 Windows 构建里根本不存在，零成本，永不处理；
`#[cfg(feature = ...)]` 门控的测试默认构建不编译，不计入风险也不算抽样证据
（报告单列 feature 名单，用 `--all-features` 抽样时再回来核对这些桶）。

## 分级手段（成本低→高，先低后高）

| 级 | 手段 | 成本 | 适用 |
|---|---|---|---|
| L0 | **入口白名单**：`ctest.sh` 只放行 `win-whitelist.txt` 内 crate；workspace 级与非白名单直接拒绝 | 零（一次写好） | 默认防线：上游套件根本进不来 |
| L1 | **环境修正**：`RUST_MIN_STACK=32MB` + `TMP/TEMP=D:\cargo-tmp`，ctest 自动注入 | 零 | 溢出族（0xc00000fd）整族消解；临时目录不走 C 盘 |
| L2 | **分族 skip**：`win-skip.txt` 每行一个 `--skip` 子串，ctest/win_sample 自动拼装 | 登记一行 | 静态特征或深排确认的环境族，族级生效 |
| L3 | **源码门控**：孤例加 `#[cfg(windows)] #[ignore = "WIN-GATE(<族>): see docs-local/WIN-TEST-GATE.md"]` + `// LOCAL:` | 中（同步要重放） | L2 按名字筛不掉的（hang/无规律命名）；恢复排查用 `--include-ignored` |
| L4 | **搬家**：WSL 或 Linux CI 跑上游套件 | 一次性投入 | 上游套件整包回归的唯一正解 |

## 工具卡

```bash
# 阶段 0：全量扫描（上游同步后重跑，报告可再生成）
python scripts-local/win_scan.py                # 全部；--crates a,b 只扫指定

# 日常测试（一律走入口门控，勿裸跑 cargo test）
scripts-local/ctest.sh -p xai-chat-state --lib

# 阶段 2：抽样（对扫描幸存的 crate；先 --list-only 看枚举量再决定跑不跑）
python scripts-local/win_sample.py -p <pkg> --every 10
python scripts-local/win_sample.py -p <pkg> --target "--test integration_name"

# 逃生门（分诊/统计用，不用于日常）
GATE_FORCE=1 scripts-local/ctest.sh -p <非白名单crate>   # 放行非白名单
GATE_DRYRUN=1 scripts-local/ctest.sh ...                 # 只打印最终命令不执行
```

**逃生门语义**：`GATE_FORCE=1` 放行 crate 但保留 L1/L2；`GATE_SKIP=0` 摘掉 skip；
`GATE_ENV=0` 摘掉环境注入。三个开关独立，方便 A/B 归因（比如 T0 时 `GATE_ENV=0`
还原原始环境对比）。

## 白名单晋升 / 退化

- **晋升**：阶段 2 抽样 PASS + 阶段 3 速读通过 → 台账记录结论 → `win-whitelist.txt` 加一行。
- **退化**：白名单 crate 在本机开始成族失败 → 先 T0 归因，环境族登记 skip，真回归才处理；
  连 skip 都兜不住的暂时移出白名单。
- 上游同步后必做：重跑 `win_scan.py`（报告 regenerate），diff 汇总表看新 crate/新档位。

## 与 PATCHES.md 的关系

PATCHES.md「Windows 测试环境族」的 T0–T3 表继续有效：T0（单测试 A/B 归因）用于
阶段 2.5 深排；T1 抬栈已升级为本体系 L1（ctest 自动注入，不再靠命令前缀纪律）；
T2 孤例补丁对应 L3，登记处仍在 PATCHES.md。

## L4 配方（WSL 跑上游套件）

```bash
# 关键坑：/mnt/d 下的 target/ 会被 Windows/Linux 两套构建互相踩，必须把产物指到 WSL 文件系统
wsl -e bash -c "cd /mnt/d/CODE/ai/grok-build-proxy && \
  CARGO_TARGET_DIR=~/target-grok TMPDIR=/tmp cargo test -p <pkg>"
```

WSL 内需自装 rustup（toolchain 跟随 rust-toolchain.toml）。跨 /mnt/d 的 IO 慢，
大套件建议 `git worktree` 到 WSL 文件系统内（`~/repos/grok`）再跑；本仓库的
上游同步测试回归在专用会话批量处理，不在开发会话里顺手做。

## 首轮扫描结论（2026-09-19，93 个 crate）

- **COMPILE-BREAK 7**：xai-fast-worktree, xai-grok-sandbox, xai-grok-shell,
  xai-grok-tools, xai-grok-update, xai-grok-voice, xai-tty-utils —— 测试目标
  Windows 编译不过，本机永不跑（与 PATCHES.md 已知问题清单吻合：shell 快照编译错、
  tools `parse_login_env_capture` 缺失）。
- **RISK 40 / RISK-LIGHT 3**：有未门控风险特征，本机默认不跑；确需跑的先进阶段 2 抽样。
- **CLEAN 43**：无静态特征，本地主题 crate 已在白名单；其余靠抽样+速读逐步晋升。
- feature 名单已入报告汇总表，`--all-features` 抽样时单独核对。
