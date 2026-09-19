
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
