# 供应商套餐用量上 `/usage` 面板（TODO）

> **状态（2026-10-10）：T1–T3 全部落地，已随 `v1.0.46` 发版（merge `afd4058`，
> build run 38065337768 / release run 38067181846 双绿，4 资产）；本机客户端已部署
> v1.0.46，面板活体核验由用户人工进行**（面板 vs
> `scripts-local/commandcode_usage.py` 对账、OpenCode Go 403 回退、缓存 debug 日志）。
> T1 适配器纯函数层已落地（新 crate
> `xai-grok-provider-usage`，5 家适配器 + 夹具单测 54 绿）；T2 shell 扩展已落地
> （`x.ai/providerUsage`，base_url 匹配 + TTL 缓存）；T3 pager 接线进行中。
> 端点/凭据/口径全部沿用
> [`quota-endpoints-and-credentials.md`](quota-endpoints-and-credentials.md)（下称「调研文档」）
> 的实测与读源码结论，可信度标注沿用[目录约定](README.md#可信度约定)。
> 基线：2026-10-10 `main`（v1.0.45 `350874c`）。分支 `feat/local-provider-usage-display`。
> 解析参照源码：cc-switch `src-tauri/src/services/coding_plan.rs` +
> yetone/magpie `internal/provider/planquota.go`、`commandcode_plan.go`（magpie 更新：
> Kimi 完整结构、MiniMax 新路径 `/v1/token_plan/remains`、OpenCode 0% 占位坑）。

## 一句话

当前模型属于「能用推理同一把 SK 查套餐用量」的供应商时，Usage limit tab 改显该
供应商的套餐窗口用量并标注来源，替代 SuperGrok 额度块；失败回退 SuperGrok，不静默互换。

## 用户已拍板（2026-10-10）

1. **触发**：模型来源 = opencode go、Command Code 这类支持「plan SK 查套餐用量」的
   供应商 → 显示**供应商套餐的量**替代 SuperGrok 块，并**标注供应商来源**。
2. **窗口粒度**：有 5h 显 5h、有周显周；两者皆无才显月度（5h/周在时月度不渲染）。
3. **支持面收敛**：只支持**与推理同一把 SK** 的供应商（一把 key 直接 Bearer 查询）。
   要控制台账户/Cookie/第二套凭据（AK-SK/临时 token）的一律不给支持。
4. **缓存**：默认 **5 分钟**，允许用户配置，**单位分钟**（`[provider_usage] cache_minutes`，
   缺省 5，读取 clamp 1–120）。
5. 本回合先不实测端点；施工后按验收清单补活体。
6. **供应商清单**：智谱 GLM / Kimi / MiniMax 三家**全部进首批实现**（原 T4 并入 T1）。
7. **Cline / Token Unlimited 放弃**（用户不会长期订阅，不立项）。
8. **供应商判定 = base_url 匹配**（「拿 baseurl 去匹配就行」）：不做显式
   `usage_provider` 配置；模型 `base_url` host/路径匹配到供应商即启用，匹配不到 =
   未配置（面板零变化）。判定实现：`provider_for_base_url`
   （`xai-grok-provider-usage/src/base_url_match.rs`）。

## 供应商清单（按拍板标准收敛后）

| 供应商 | 窗口 | 凭据 | 可信度 | 状态 |
| --- | --- | --- | --- | --- |
| **OpenCode Go** | 5h 滚动 + 周 + 月 | Bearer，与推理同 key | 端点存在性已实测 + 读上游源码 | **首批**（点名；已实现） |
| **Command Code（GOAT）** | 5h + 周（美元额度）+ 月度 credits | Bearer，与推理同 key | **已端到端实测**（调研文档 §3） | **首批**（点名；本机 3 模型在用；已实现） |
| **智谱 / z.ai GLM Coding Plan** | 5h + 周 | 同 key（先裸 key，`code:1001` 信封再换 Bearer） | 读源码核实 + host 存活实测 | **首批**（拍板 6；已实现） |
| **Kimi For Coding** | 5h + 周 | Bearer 同 key | 读源码核实（零实测） | **首批**（拍板 6；已实现） |
| **MiniMax Coding Plan** | 5h + 周（激活位门控） | Bearer 同 key | 读源码核实（零实测） | **首批**（拍板 6；已实现） |
| 火山 Ark Coding Plan | session/周/月 | **AK/SK 第二套凭据**（推理 key 实测被拒） | 读源码核实 + 推理 key 实测被拒 | **剔除**（违反拍板 3） |
| 百炼 / 千问 Token Plan | 5h + 周 | console 临时 token 两步链路 | 读源码核实 + 第三方记录 | **剔除**（违反拍板 3；2026-10-10 复核官方 CLI 源码：单 key 仍不支持，见调研文档 §3） |
| Cline（本机在用）/ Token Unlimited（本机在用） | 未知 | 未知 | 调研盲区（调研文档当时按收录标准剔除） | **放弃**（拍板 7，不会长期订阅） |

端点、响应结构、百分比语义（已用 vs 剩余 vs 计数）、各家坑（GLM 站点选择与失败
信封、MiniMax 周窗激活位、Command Code `exceeded` 才是权威拦停判据等）见调研文档
§3/§4.2，此处不复述。

## 现状锚点

1. `crates/codegen/xai-grok-pager/src/views/usage_modal.rs:755` `usage_limit_lines`
   —— 分流点：`chat_kind` 守卫 / `balance` 分支（:779 调 `allowance_lines`）。
2. `usage_modal.rs:810-937` `allowance_lines` —— SuperGrok 额度条 + LOCAL 周额度
   估算块（:877 起）；bar 渲染（BAR_WIDTH=30，:828-841）可直接复用。
3. `usage_modal.rs:70-81` `UsageInfoContext` —— 字段只有 `session_id /
   usage_visible / chat_kind / billing_redirect_url / subscription_tier`，
   **没有当前模型**；T3 需补当前模型的 `usage_provider`（+供应商显示名）管道。
4. `crates/codegen/xai-grok-pager/src/app/dispatch/status.rs:45-80`
   `open_usage_info_modal` —— ctx 构造点；`app.subscription_tier` 同源可取当前模型
   信息（modal 打开时 agent/session 均在手）。
5. 触发先例：`Effect::FetchBilling`（`event_loop.rs:2916` dispatch、
   `effects/mod.rs:4803` 执行；modal 打开 + turn 结束静默拉，见
   `../usage-quota-estimate-todo.md` 锚点 3）——供应商用量沿用同两处触发。
6. `crates/codegen/xai-grok-shell/src/agent/config.rs:3827` `ModelEntryConfig` ——
   `api_key` / `env_key` / `auth_provider` 解析链现成；`use_proxy`（v1.0.43）是
   「per-model 枚举字段 + 非法值 warn 丢弃」的先例。
7. `xai-grok-tools`（`quota_estimate` / `model_usage_ledger`）—— 适配器纯函数层的
   落位先例（白名单 lib 测试可跑）。
8. 本机 config 实况：`[model.*]` 现有 Command Code（`api.commandcode.ai/provider/v1`
   ×3）、Ark coding/v3 ×2、Cline ×2、Token Unlimited ×1。

## 设计草案

### 供应商判定与凭据（施工定稿：base_url 匹配）

```toml
[model."deepseek/deepseek-v4.1-flash"]     # 既有块不动，不加任何配置
base_url = "https://api.commandcode.ai/provider/v1"   # host 命中 → 自动启用
```

- **判定**：取数时对模型 `base_url` 做 host/路径匹配
  （`provider_for_base_url`，规则见该模块 doc：opencode.ai+`/zen/go` /
  `api.commandcode.ai` / `bigmodel.cn`+`api.z.ai` / `api.kimi.{com,ai}`+`/coding` /
  `api.minimax.{i.com,io,cn}`）；匹配不到 = 未配置，面板零变化。
- 凭据：复用该模型的解析链（`own_credential`：`api_key` → `env_key`），
  不新增凭据输入；`auth_provider` 托管型（无静态 key）按 MissingKey 报错。

### 数据模型（T1，落地为独立小 crate `xai-grok-provider-usage`）

```rust
pub struct PlanUsageSnapshot {
    pub provider: UsageProviderId,       // 供应商枚举
    pub plan_name: Option<String>,       // data.level / credits.planId 等
    pub windows: Vec<PlanUsageWindow>,   // 适配器产出全量窗口
    pub fetched_at_ms: u64,
}
pub struct PlanUsageWindow {
    pub kind: UsageWindowKind,           // FiveHour | Weekly | Monthly
    pub used_pct: Option<f64>,           // 0–100，已用口径，适配器归一后
    pub resets_at_ms: Option<u64>,       // epoch ms（秒/毫秒/RFC3339 宽容解析）
    pub note: Option<String>,            // 原样透传（如 status、$ 金额、limited/exceeded）
}
```

- 适配器 = 「端点组装 + 请求头 + 响应 → Snapshot」的纯函数，HTTP 由调用方执行；
  按**一家一文件**拆分（`providers/{opencode_go,commandcode,glm,kimi,minimax}.rs`）。
- 百分比语义归一在适配器内完成（调研文档 §4.2：opencode/GLM 给已用；
  MiniMax 给剩余要反转；Kimi 给计数要换算；Command Code 由美元 used/cap 换算）。
- Command Code：`windowLimits.fiveHour/weekly` 是美元 `used/cap` → `used_pct =
  used/cap*100`；`cap` 缺失/0 → `used_pct=None`，金额进 `note`；每窗 `exceeded`
  与顶层 `limited` 都进 `note`；`credits.planId` 进 `plan_name`；
  `monthlyCredits` 不进窗口集（用户规则）。
- OpenCode Go：`usage.rolling → FiveHour`、`weekly → Weekly`、`monthly → Monthly`；
  `status` 语义未穷举，非 "ok" 原样进 `note`；percent=0 时 resetsAt 是占位值，丢弃。
- 多次尝试建模为 `PreparedRequest` 列表：GLM 裸 key → Bearer（`code:1001` 信封触发）、
  MiniMax 新路径 `/v1/token_plan/remains` → 旧路径（404 触发）。
- 失败分类：401/403（key 无效或无订阅资格，如 OpenCode `EntitlementError`）/
  HTTP 错误 / 解析失败，三层错误进 UI 文案（`error_kind` 随回包给 UI 分层）。

### 取数与缓存（T2，shell 扩展 `x.ai/providerUsage`）

- `Effect::FetchProviderUsage { agent_id, model_id, nonce }`，**只在 usage 模态打开时
  触发**（shell 侧 TTL 缓存使命中即回，turn 结束静默刷新省略——与方案初稿的差异，
  缓存已覆盖该场景）；GET + 5s 超时；代理语义随该模型 `use_proxy`
  （直连 client 为默认，与采样路径一致）。
- 缓存：shell 进程级 `HashMap<(model_id, provider), (snapshot, Instant)>`；
  TTL = `[provider_usage] cache_minutes`（默认 **5**，单位分钟，clamp 1–120）。
  命中期内重复打开不发请求；失败不写缓存（下次打开即重试）。

### 显示（T3，usage_modal）

- base_url 无匹配（回包 `provider: null`）→ 面板零变化。
- 有匹配且快照可用 → 「套餐用量（<plan 名>） — <供应商名>」来源标注头 + 每窗口
  一条 bar（与 SuperGrok 块共用 `usage_bar_line`）+ `Resets` 行 + `note` 行；
  **SuperGrok 额度块隐藏**（拍板 1）。
- 渲染裁剪：窗口集含 5h 或周时月度不渲染（拍板 2）。
- 失败且无缓存快照 → 错误行（分类文案）+ 回退渲染 SuperGrok 块。
- 首开在途（回包未到）→「正在加载套餐用量…」行 + SuperGrok 块照常渲染。
- `chat_kind` 网关会话跟随现状整块跳过（待定决策 6）。
- 文案进 i18n 中英两表（供应商名不翻译）；`Resets` 键已存在复用。

## 分期（施工实况）

- **T1 适配器纯函数层**——已完成（commit `ef57a48` + `c2940a8`）：新 crate
  `xai-grok-provider-usage`（types/requests/parse/providers×5/base_url_match），
  夹具单测 54 绿（`ctest.sh -p xai-grok-provider-usage --lib`，已入 win-whitelist）。
- **T2 shell 扩展**——已完成（commit `35fb708`）：`extensions/provider_usage.rs`
  （`x.ai/providerUsage`，acp_agent 注册）+ `[provider_usage] cache_minutes` 配置节；
  `cargo check -p xai-grok-shell` 0 error。
- **T3 UI 接线**——进行中：`Effect::FetchProviderUsage` + `TaskResult::ProviderUsageFetched`
  （nonce 守卫 settle 自己这代模态）+ `usage_limit_lines` 供应商分流 + i18n。

## 验收标准

1. 夹具单测：各家固定响应 → 窗口归一断言（百分比语义反转/换算、窗口裁剪、
   `exceeded`/`limited`、GLM `code:1001` 信封、无订阅 403）。
   —— 已过：`ctest.sh -p xai-grok-provider-usage --lib` 54/54。
2. pager 涉改模块 lib 测试全绿（usage_modal 供应商分流/回退/loading 用例）。
3. 本机活体（Command Code）：面板窗口/百分比与既有探针
   `scripts-local/commandcode_usage.py` 输出一致。
4. OpenCode Go：无 Go 订阅资格的 key → 403 `EntitlementError` 显示错误行并回退
   SuperGrok 块。
5. 缓存：默认 5 分钟内重开面板零请求（debug 日志 `provider usage: cache hit` 取证）；
   改 `[provider_usage] cache_minutes` 后行为随之变化。
6. base_url 无匹配的模型（Ark/Cline/Token Unlimited/xAI 本家）：面板与改前一致。
7. `cargo check -p <涉改 crate> --all-targets` 0 error。

## 待定决策（拍板点）

1. ~~供应商清单~~ **已拍板**：GLM / Kimi / MiniMax 全进首批（拍板 6）。
2. ~~Cline / Token Unlimited~~ **已拍板**：放弃（拍板 7）。
3. ~~配置键名~~ **已拍板**：不做显式配置，base_url 匹配（拍板 8）。
4. Command Code 的 `monthlyCredits`（$ 余额）是否加一行辅助显示（当前不显，
   窗口 note 里已带每窗 $ used/cap）。
5. 失败负缓存（短 TTL 内不重试）做与否；当前设计为不写失败缓存。
6. `chat_kind` 网关会话是否渲染供应商块（当前跟随现状跳过）。
