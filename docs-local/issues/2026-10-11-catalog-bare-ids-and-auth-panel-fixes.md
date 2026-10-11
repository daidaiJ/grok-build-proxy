# v1.0.50 活回合三问题：成本列裸 id 无价 + /auth 提示行悬空 + 写回后 /model 不刷新

- **状态**：fixed（分支 `fix/local-catalog-bare-ids-auth-panel`，随 v1.0.51 发版）
- **严重度**：P2（成本列半数模型无价）+ P3（面板布局）+ P2（写回后需重启才见新模型）
- **发现方式**：用户活回合截图反馈（v1.0.50 客户端实跑 /stats 与 /auth）
- **环境**：`main` `08dffe2`（v1.0.50 发版后）；发现日 2026-10-11

## 现场与归属

三个问题全部出在 LOCAL 特性（T1+特性B 成本列 / T2+T3 /auth 面板，PATCHES 二十八～
三十期），与上游无关。

**① 成本列：`glm-5.3-flash` / `gpt-6-luna` 显示 `–`，`deepseek/deepseek-v4.1-flash`
有 $8.07**（截图 `D:/Program/PixPin/Temp/PixPin_2026-10-11_17-35-50.png`）

- 归属：LOCAL 实现缺陷。`fetch_catalog` 按序尝试、首个成功即返回——OpenRouter 主源
  下生效目录全是 `vendor/model` 形态 id，裸网关 id（`glm-5.3-flash`、`gpt-6-luna`）
  只剩 canonical 反查一条路，必失配。unified.jsonl 佐证：当日 17:34:59
  `model catalog: fetched` entries=458（OpenRouter 单源）。
- 用户拍板（多轮收敛，详见 design doc 二轮拍板 8–10）：models.dev 主源（直连稳定，
  裸 id + openrouter 镜像双覆盖）、只留原厂定价（白名单过滤，转售条目整库不留）、
  品牌归属平局序 + 剥壳/变体/部署戳容忍匹配。

**② /auth 面板提示性内容没贴底栏**

- 归属：LOCAL 布局缺陷。按键提示嵌在内容流里（内容顶对齐跟随），其他面板惯例是
  固定贴面板底部（页脚行之上）。修：提示行固定渲染倒数第二行。

**③ /auth 配置后要重启 + /model 才看到新模型**

- 归属：LOCAL 实现缺陷（设计声称的热重载不存在）。`TaskResult::ModelEntriesPersisted`
  只推进面板，无人触发模型重载；`ConfigFileWatcher` 仅 `run_leader` 装配，
  TUI 的 embedded/headless 模式整条 watcher→reloader→reload_models 链路不存在。
  unified.jsonl 佐证：17:34 启动进程（12 模型）→ 17:38:53 退出，期间零热重载
  日志；17:38:55 新进程启动即 15 模型。修：写回成功后 pager 显式发
  `x.ai/internal/reload_models` → agent 重读 config → 广播 `x.ai/models/update`。

## 修复验证

- 纯函数层：`ctest.sh -p xai-grok-model-catalog --lib`（原厂过滤/平局/剥壳/戳容忍/
  prices 序换/零价归无价，见 PATCHES.md 三十一期用例清单）。
- shell：`--lib model_catalog`（磁盘缓存往返/版本守卫/时钟回拨）+
  `ext_method_reload_models`（裸方法名路由 + `{models: n}` 线上契约）。
- pager：`--lib auth_modal`（提示行贴底渲染断言）+ `dispatch::tests::auth`（写回
  Ok → ReloadAgentModels effect、Err 不触发、重载 Err 落完成页）。
- 匹配仿真：本机 10 个在用 model id 去重后 8 命中全为原厂/官方价，2 miss 均为
  sensenova（models.dev 无商汤白名单 provider，用户豁免）。
- 待活回合验证：/stats 三模型成本列、/auth 五步写回后不开重启直接 /model、
  prices.cache 落盘与断网回退。

## 备注

- models.dev 白名单核对与国内延迟采样数据见 design doc 二轮拍板 8；样本与仿真脚本
  为会话临时产物（`D:/cargo-tmp/`），未入库。
- `/tmp` 落盘坑（MINGW64 curl -o 文件消失）已另录全局记忆，与本 issue 无关。
