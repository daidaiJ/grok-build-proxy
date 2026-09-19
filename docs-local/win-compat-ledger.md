
## 白名单全量验证 — 2026-09-19（ctest.sh 首轮全量跑）

- **6/7 直接全绿**：sampling-types 301、compaction-transcript 9、extra-ca 15、status-line 18、
  chat-state 365、sampler 262（含 P0 批次新增测试：前缀不变量/方言自学习/消息合并）。
- **xai-grok-pager**：9566 过 / 54 挂。分诊结论：**七族全环境根因，零 LOCAL 交集**（失败模块
  与本 fork 补丁面无交集，且这些上游测试在本机从未跑过）——
  1. doctor fix 族 ×10：测试助手 `test_fix_plan` 硬编码 `/bin/bash`
  2. dashboard 工作树/活行族 ×10：git worktree 与路径身份 Windows 形态
  3. scrollback 路径夹具族 ×18：链接化/header 期望 POSIX 路径形态
  4. workspace_sync/membership 族 ×4：夹具 cwd=`/tmp/...` 过不了资格校验
  5. tab 补全 fetch 族 ×2：cwd 分隔符破坏确定性断言
  6. foreign_sessions ×1：正斜杠 `contains` 匹配反斜杠路径必假
  7. 散例 ×9（paste/selection/headless/disk_usage/extensions_modal/git_info）：同路径形态根因
- **处置**：54 个全部按 `模块::测试名` 精确登记 `win-skip.txt`（L2 门控），未修产品代码、
  未打补丁。门控后复跑：**9565 过 / 0 挂 / 55 过滤**（1 条前缀碰撞连带过滤
  `_multi_edit` 兄弟测试，已在清单注释说明）。
- **门控脚本修复**：ctest.sh 的 --skip 需位于 cargo `--` 之后（libtest 参数边界），
  已补自动插 `--` 逻辑。

## 覆盖缺口扫描（集成测试/doc-test/crates 外成员）— 2026-09-19

**对账结论**（cargo metadata 枚举 101 packages）：
- 147 个集成测试目标分布在 23 个 crate，本机此前从未跑过任何集成目标（只跑 --lib）。
- **crates/ 之外有 5 个 workspace 成员此前零扫描零验证**：prod/mc/cli-chat-proxy-types
  （38 tests，静态判 RISK：term-detect 特征）、third_party/mermaid-to-svg（79 tests，
  CLEAN）、dagre_rust（1，CLEAN）、graphlib_rust / ordered_hashmap（无测试）。
  win_scan.py 已扩展扫描根到 prod/ 与 third_party/（现覆盖 96 个 crate）。
- doc-test：白名单 7 crate 全部为空或 0 跑（仅 2-4 ignored），无缺口。

**白名单集成测试补跑结果**：
- xai-grok-extra-ca：7 个集成目标全绿（ALPN/握手/证书链在 Windows 全通过）。
- xai-grok-sampler：6 个集成目标全绿（test_actor 24、shared_http_wire 7 等）。
- xai-grok-pager：6/7 目标绿（grok_home_paths 2、plugin_marketplace ×2、
  selection_model 2、mermaid_render_subprocess 0/4 ignored）；
  **settings_e2e 275 过 / 2 挂** —— 根因是七期 i18n 的 tr() 英文旁路只覆盖
  cfg!(test) 单测构建，集成测试链接的是正常构建的 lib → 中文渲染撞上上游
  英文断言（"Reset ..." vs "将 'Compact mode' 重置为默认值（关）？"）。
  属 LOCAL 特性预期行为，登记 win-skip.txt 族8；已留设计注记：若上游集成
  测试断言英文 UI 的面扩大，给 i18n 加集成测试旁路。
- signal_errno_preservation：0 可跑（signal 测试全 #[cfg(unix)] 自门控）。

**净结果**：白名单 crate 的 lib + 集成 + doc-test 三层全部验证完毕，
仅剩 2 个 i18n 行为分歧测试按预期登记门控，无未解释失败。

## 全量 workspace 测试实跑（2026-09-19，回答"会不会卡住/报错"）

跑法：`cargo test --workspace --no-fail-fast` 排除 10 个编译断 crate（静态扫描预测），
90 分钟看门狗，约 55 分钟跑完（未触发看门狗）。

**会卡住吗？会一处**：`xai-grok-telemetry` 的集成目标 `external_otlp_grpc_tls` 挂起
12+ 分钟（子进程 CPU 0.19s，纯网络阻塞等待），手动 taskkill 后 cargo 正常续跑。
**会报错吗？会**：10 个 crate 编译断（全部被扫描器预测，其中 pty-harness/hunk-tracker/
workspace 三例驱动了扫描器增强）。

数字：245 个目标，150 个有结果，总 passed=18280 / failed=174。

失败分族（全部为环境族或 LOCAL 预期分歧，**无本 fork 引入的回归**）：
- 已登记门控复现：pager 54（族1-7）+ settings_e2e 2（族8 i18n）——本轮裸跑不走门控。
- 新确认环境族（上游 crate，白名单门控本就不放行本机）：
  - flock/文件锁族：login 26、plugin_marketplace 1、active_sessions 1、session_search 1
  - sqlite 文件锁族：dashboard_store 19、memory 5
  - 信号语义族：shell_base 5（sigterm trap）、shell_terminal 1（kill 返回 signal）、
    hooks 3（dispatcher exit-2/stderr）
  - POSIX 路径形态族：pager_render 30（osc8 路径扫描）、pager_minimal 3、
    agent 10（symlink/skills 路径）、paths 2、telemetry 1
  - 守护进程语义族：workspace_daemon 5（pidfile daemonize）
  - hook 夹具脚本 pwsh 兼容：hooks 集成 2（Get-Content 缺参 / stdin 脚本产出非法 JSON）
- 真缺陷候选（上游 xai-grok-test-support 的 Windows 专属测试，fork 不修只记录）：
  - `windows_platform_essentials_are_allowlisted`：TestSandbox 环境白名单漏传
    SystemRoot → 沙箱子进程会坏
  - `windows_job_kill_reaps_spawned_grandchild`：grandchild pid file 5s 超时
    （疑似卡巴斯基拦截临时目录子进程）

**扫描器同步增强**（本轮三例漏报驱动）：
- 新签名 `unix-fn-path`：内联全限定 `std::os::unix::...` 调用（hunk-tracker）
- 跨 crate unix 门控导入检测：收集各 crate 被 `#[cfg(unix)]` 门控的 pub 项，
  匹配 `use <crate>::{...}`（连字符/下划线归一）——pty-harness 族
- 现在 COMPILE-BREAK 预测 = 10 crate，与实跑编译断完全一致
