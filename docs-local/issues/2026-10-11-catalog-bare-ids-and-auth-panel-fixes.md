# v1.0.50 活回合三问题：成本列裸 id 无价 + /auth 提示行悬空 + 写回后 /model 不刷新

- **状态**：fixed（二次修复分支 `fix/local-catalog-bare-id-cache-dir`，随 v1.0.51
  重指重发；首修随 v1.0.51 首发但活回合复发，见文末「复发与二次修复」）
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

## 复发（2026-10-11 晚，v1.0.51 活回合）与二次修复

**现场**：/stats 成本列依旧只有 `deepseek/deepseek-v4.1-flash`（$8.07），
`glm-5.3-flash` 与 `gpt-6-luna` 仍为 `–`。当前会话 pid 8176（v1.0.51）
unified.jsonl：12:04:46 `model catalog: fetched` **entries=458** —— 与 v1.0.50
OpenRouter 单源条数一致，models.dev 主源一条未进；`~/.grok/prices.cache` 458 条
全部 `source: open_router`。

**根因（三件叠加）**：

1. **主源静默失败**：`fetch_one` 的 Err 只落 `tracing::debug!`，不进
   unified.jsonl（全量 grep ver=1.0.51 无任何 catalog 失败行）——models.dev 瞬时
   不可达时整库静默回退 OpenRouter，现场无从排查。复测两源直连均 200
   （models.dev 5.4MB / 5.1s，在 30s 超时内），确系瞬时。
2. **匹配器裸 id 分支只认裸条目**：84 档兜底写死 `!entry.id.contains('/')`；
   目录只剩 `vendor/model` 形态（OpenRouter）时裸 id 必失配——三十一期修复的
   前提（models.dev 供裸 id）被 1 掏空后裸 id 全灭。
3. **缓存位置**：prices.cache 写在 `~/.grok/` 根而非 `cache/` 专用目录（用户指出，
   同目录已有 model-usage / billing-samples）。

**二次修复（分支 `fix/local-catalog-bare-id-cache-dir`，PATCHES.md 三十二期；
按用户指示不切新 tag，v1.0.51 重指重发）**：

- 匹配：裸 id 对 `vendor/model` 条目按 model 段命中（`:variant` 条目键自带后缀
  不参与，`:free` 零价不冒充牌价）；匹配索引化（fetch 时预建 by_id /
  by_canonical / by_vendor / by_segment 倒排，查询 O(1) 候选，全库线性扫描退役）。
- 可见性：单源失败 warn 进 unified.jsonl（source/url/error），fetched 日志带
  `source` + `trigger`——再发生回退一眼可见。
- 缓存：迁 `~/.grok/cache/prices.cache`（version 2），首次成功取数后清除根下旧
  文件；进程常驻改「请求 id → 参数」薄表（冷热分离：磁盘 = 唯一全量副本，
  全量目录只在取数 / 薄表补齐瞬间物化，用完即弃）。
- 启动预热：进程拉起异步取数一次/天（TTL + 单飞 + 磁盘缓存去重），/stats 与
  /auth 首开即有热数据，避免重复下载。
- /auth 回填：目录 `reasoning` = Some(true) 时写 `supports_reasoning_effort = true`
  接入既有 effort 机制（context_window / max_completion_tokens 回填链路三十一期
  已有，此前因裸 id 失配而空转）。

**验证（真实数据仿真 `catalog-sim`，读本机 v1.0.51 遗留 OpenRouter-only 458 条
缓存 = 最坏情形）**：config `[model.*]` 表键 **8/8**、条目路由 slug **8/8**、账本
回显 id **6/6**（去重并集 11 个 id）——全部命中原厂/官方价；此前两 miss 现命中：
`glm-5.3-flash` → `z-ai/glm-5.3-flash`（84 档，$0.15/$0.50 per 1M）、
`gpt-6-luna` → `openai/gpt-6-luna`（84 档，$0.10/$0.50 per 1M）；
`grok-4.6` → `x-ai/grok-4.6`、`gpt-5.6-luna` → `openai/gpt-5.6-luna` 同按段命中。
crate 测试数字见 PATCHES.md 三十二期。待活回合：新客户端 /stats 三模型成本列 +
/auth 五步回填 + unified.jsonl 里 `source` 字段核对。

**补修（同 tag 重发内；用户复现「成本全 `–` 连 v4.1 flash 也无」）**：二次修复
首发的薄表 lazy 路径翻车——`completed_entries` 把匹配并回共享薄表，但**回包用
调用方旧快照**组包；首次 /stats 薄表尚空（启动预热只放空表）→ 回包全 null，
pager store 按「已知无价」缓存 6h 不重问 → 全部模型 `–`。修复 = 回包基于并回后
的合并视图（shell model_catalog 8/8，新增回归用例钉死）。按用户要求补运行时
价格日志：每次取价回包打 unified `model catalog: priced`（id → entry/score/
in/out + unpriced 列表 + cache 档位）。仿真（纯匹配层）当初全绿而活回合翻车，
因为翻车点在**回包组装**而非匹配——降级/缓存链路的验证必须覆盖请求→回包全程。
