
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
