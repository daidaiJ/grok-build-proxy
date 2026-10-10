# 供应商套餐用量上 `/usage` 面板（TODO）

> **状态（2026-10-10）：设计定稿，供应商清单待拍板后开工。**
> 本回合按用户要求未发任何探测请求；端点/凭据/口径全部沿用
> [`quota-endpoints-and-credentials.md`](quota-endpoints-and-credentials.md)（下称「调研文档」）
> 的实测与读源码结论，可信度标注沿用[目录约定](README.md#可信度约定)。
> 基线：2026-10-10 `main`（v1.0.45 `350874c`）。分支拟 `feat/local-provider-usage-display`。

## 一句话

当前模型属于「能用推理同一把 SK 查套餐用量」的供应商时，Usage limit tab 改显该
供应商的套餐窗口用量并标注来源，替代 SuperGrok 额度块；失败回退 SuperGrok，不静默互换。

## 用户已拍板（2026-10-10）

1. **触发**：模型来源 = opencode go、Command Code 这类支持「plan SK 查套餐用量」的
   供应商 → 显示**供应商套餐的量**替代 SuperGrok 块，并**标注供应商来源**。
2. **窗口粒度**：有 5h 显 5h、有周显周；两者皆无才显月度（5h/周在时月度不渲染）。
3. **支持面收敛**：只支持**与推理同一把 SK** 的供应商（一把 key 直接 Bearer 查询）。
   要控制台账户/Cookie/第二套凭据（AK-SK/临时 token）的一律不给支持。
4. **缓存**：默认 **5 分钟**，允许用户配置，**单位分钟**。
5. 本回合先不实测端点；施工后按验收清单补活体。

## 供应商清单（按拍板标准收敛后）

| 供应商 | 窗口 | 凭据 | 可信度 | 状态 |
| --- | --- | --- | --- | --- |
| **OpenCode Go** | 5h 滚动 + 周 + 月 | Bearer，与推理同 key | 端点存在性已实测 + 读上游源码 | **首批**（点名） |
| **Command Code（GOAT）** | 5h + 周（美元额度）+ 月度 credits | Bearer，与推理同 key | **已端到端实测**（调研文档 §3） | **首批**（点名；本机 3 模型在用） |
| 智谱 / z.ai GLM Coding Plan | 5h + 周 | 同 key（Bearer 前缀有分歧：先裸 key，失败信封 `code:1001` 再换 Bearer） | 读源码核实 + host 存活实测 | 候选，待拍板 |
| Kimi For Coding | 5h + 周 + 月度池 | Bearer 同 key | 读源码核实（零实测） | 候选，待拍板 |
| MiniMax Coding Plan | 5h + 周（激活位门控） | Bearer 同 key | 读源码核实（零实测） | 候选，待拍板 |
| 火山 Ark Coding Plan | session/周/月 | **AK/SK 第二套凭据**（推理 key 实测被拒） | 读源码核实 + 推理 key 实测被拒 | **剔除**（违反拍板 3） |
| 百炼 / 千问 Token Plan | 5h + 周 | console 临时 token 两步链路 | 读源码核实 + 第三方记录 | **剔除**（违反拍板 3） |
| Cline（本机在用）/ Token Unlimited（本机在用） | 未知 | 未知 | 调研盲区（调研文档当时按收录标准剔除） | 要支持需先补调研 |

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

### 配置面

```toml
[model."deepseek/deepseek-v4.1-flash"]     # 既有块上加一行
base_url = "https://api.commandcode.ai/provider/v1"
usage_provider = "commandcode"             # 新增：opencode-go | commandcode | glm-coding | kimi-coding | minimax
```

- 显式配置，不做 base_url host 自动推断（防误判；host 一致性只作 warn 提示）。
- 未知值：warn 一次 + 视同未配置（仿 `use_proxy` 语义）。
- 凭据：复用该模型的解析链（`api_key` → `env_key` → `auth_provider`），不新增凭据输入。

### 数据模型（T1，`xai-grok-tools/src/provider_usage.rs`）

```rust
pub struct PlanUsageSnapshot {
    pub provider: UsageProviderId,       // 供应商枚举
    pub plan_name: Option<String>,       // data.planName / subscriptions.planId 等
    pub windows: Vec<PlanUsageWindow>,   // 适配器产出全量窗口
    pub fetched_at_ms: u64,
}
pub struct PlanUsageWindow {
    pub kind: UsageWindowKind,           // FiveHour | Weekly | Monthly
    pub used_pct: Option<f64>,           // 0–100，已用口径，适配器归一后
    pub resets_at: Option<u64>,          // epoch ms
    pub note: Option<String>,            // 原样透传（如 status、$ 金额）
}
```

- 适配器 = 「端点组装 + 请求头 + 响应 → Snapshot」的纯函数，HTTP 由调用方执行。
- 百分比语义归一在适配器内完成（调研文档 §4.2：opencode/GLM/Ark/百炼给已用；
  MiniMax 给剩余要反转；Kimi 给计数要换算）。
- Command Code：`windowLimits.fiveHour/weekly` 是美元 `used/cap` → `used_pct =
  used/cap*100`；`cap` 缺失/0 → `used_pct=None`，金额进 `note`；只认 `exceeded`
  作拦停，`limited` 仅透传 `note`；`credits.monthlyCredits` 不进窗口集（用户规则）。
- OpenCode Go：`usage.rolling → FiveHour`、`weekly → Weekly`、`monthly → Monthly`；
  `status` 语义未穷举，原样进 `note`。
- 端点解析：每适配器固定端点表（opencode.ai / api.commandcode.ai）；GLM 按
  base_url host 选站（含 `bigmodel.cn` → 国内站，否则国际站，不跨站回退）；
  base_url origin 与端点 host 不一致仅 warn，不阻断。
- 失败分类：401/403（key 无效或无订阅资格，如 OpenCode `EntitlementError`）/
  HTTP 错误 / 解析失败，三层错误进 UI 文案。

### 取数与缓存（T2，shell 效果层）

- 新 `Effect::FetchProviderUsage { model_id, provider }`，触发点复用 FetchBilling
  两处（modal 打开 + turn 结束静默）；GET + 5s 超时；代理语义随该模型 `use_proxy`
  既有决策（用量端点与推理同 host 场景居多）。
- 缓存：全局配置项（默认 **5**，单位分钟，clamp 1–120；落点 `xai-grok-config`，
  具体 section 名施工时对齐现有表结构，拟 `provider_usage_cache_minutes`）。
  命中期内重复打开不发请求；失败不写缓存（下次打开即重试）。

### 显示（T3，usage_modal）

- 当前模型无 `usage_provider` → 面板零变化。
- 有且快照可用 → 顶部「套餐用量（<供应商名>）」来源标注行 + 每窗口一条 bar
  （复用 BAR_WIDTH 渲染）+ `Resets` 行；**SuperGrok 额度块隐藏**（拍板 1）；
  周额度反推块保持现状（供应商模型下 NoLocalUsage 是常态，不冲突）。
- 渲染裁剪：窗口集含 5h 或周时月度不渲染（拍板 2）。
- 快照失败且无缓存快照 → 错误行（分类文案）+ 回退渲染 SuperGrok 块。
- `chat_kind` 网关会话跟随现状整块跳过（待定决策 6）。
- 文案进 i18n 中英两表（供应商名不翻译）；`Resets` 键已存在可复用。

## 分期

- **T1 适配器纯函数层**（~1 天）：schema + opencode-go / commandcode 两适配器 +
  固定 JSON 夹具单测（Command Code 夹具用调研文档逐字实测结构）。
- **T2 配置面 + 取数**（~1 天）：`ModelEntryConfig.usage_provider` + enum 校验 +
  `FetchProviderUsage` + TTL 缓存 + 全局配置键。
- **T3 UI 接线**（~1 天）：ctx 管道（当前模型 usage_provider）+ 分流渲染 + i18n。
- **T4 追加供应商**（每家 ~0.5 天）：按拍板结果加 GLM / Kimi / MiniMax 适配器。

## 验收标准

1. 夹具单测：各家固定响应 → 窗口归一断言（百分比语义反转/换算、窗口裁剪、
   `exceeded`/`limited`、GLM `code:1001` 信封、无订阅 403）。
2. `ctest.sh -p xai-grok-tools --lib` 全绿；pager 涉改模块过滤跑全绿。
3. 本机活体（Command Code）：面板窗口/百分比与既有探针
   `scripts-local/commandcode_usage.py` 输出一致。
4. OpenCode Go：无 Go 订阅资格的 key → 403 `EntitlementError` 显示错误行并回退
   SuperGrok 块。
5. 缓存：默认 5 分钟内重开面板零请求（debug 日志取证）；改配置后行为随之变化。
6. 未配 `usage_provider` 的模型：面板与改前逐字节一致。
7. `cargo check -p <涉改 crate> --all-targets` 0 error。

## 待定决策（拍板点）

1. **供应商清单**：GLM Coding Plan / Kimi For Coding / MiniMax 三家追加哪几家进
   T4（本回合等用户拍板）。
2. **Cline / Token Unlimited**（本机在用但接口形态未调研）：是否立项补调研。
3. 配置键名 `usage_provider` 定案（备选 `quota_plan`）。
4. Command Code 的 `monthlyCredits`（$ 余额）是否加一行辅助显示（当前按拍板 2
   不显）。
5. 失败负缓存（短 TTL 内不重试）做与否；当前设计为不写失败缓存。
6. `chat_kind` 网关会话是否渲染供应商块（当前跟随现状跳过）。
