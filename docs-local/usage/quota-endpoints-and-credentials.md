# 订阅套餐用量接口与凭据方式（调研记录）

> **状态（2026-09-27）：调研完成，未提交推送。**
> **后续（2026-10-10）**：施工方案见
> [`provider-quota-display-todo.md`](provider-quota-display-todo.md)（用户已拍板：
> 只支持与推理同一把 SK 的供应商、窗口粒度 5h>周>月、缓存默认 5 分钟）。
> **更新（同日）：Command Code 已用真实 key 端到端验证**（四个端点、真实字段名、月度上限
> 推导口径、订阅取消标记），配套只读探针 `scripts-local/commandcode_usage.py`（未入库）。
> 触发：研究 `lostexile/MyTokenDashboard`（macOS 菜单栏额度监控，Tauri 2 + Rust）的
> 「按提供商取用量百分比」策略，并评估 opencode go / Command Code（GOAT）/
> 火山 Ark Coding Plan / 百炼 Token Plan / 智谱·z.ai GLM Coding Plan 这类订阅能否便捷支持上。

可信度标注见 [`README.md`](README.md)（已实测 / 读源码核实 / 第三方记录）。

## 1. MyTokenDashboard 的取数策略

仓库极小（3 个提交、4 星、无 issue），但策略分型干净，共 4 类 + 1 个半自动入口：

| 类别 | 实现（文件） | 凭据 | 返回口径 |
| --- | --- | --- | --- |
| 官方 API key 直连 | `providers/zhipu.rs`、`providers/deepseek.rs` | 智谱：`Authorization: <裸 key>`（**无 Bearer 前缀**）；DeepSeek：`Bearer` | 额度 / 余额**金额**，不是百分比 |
| 控制台私有 API + Cookie | `providers/kimi.rs`、`providers/xiaomi.rs` | 浏览器 Cookie（整段粘贴） | 真实百分比 |
| 云厂商 AK/SK 签名 | `providers/volcengine.rs` | AccessKeyId + SecretAccessKey（自实现 SigV4） | 只有 token 用量、**无套餐剩余** → 界面只能出计量条 |
| 通用自定义 HTTP 源 | `providers/custom.rs` + `config.rs:CustomSourceSpec` | none / bearer / cookie / header 四种 | 兜底，且**只能出一行** |
| 内置登录 webview 抓包 | `capture.rs` | 用户在应用内登录，自动取 Cookie | 省去手抄 Cookie |

具体端点（读源码核实）：

- 智谱：`GET https://open.bigmodel.cn/api/monitor/usage/quota/limit`，`Authorization` 为裸 key
- DeepSeek：`GET https://api.deepseek.com/user/balance`，`Bearer`
- Kimi：`POST https://www.kimi.com/api/v2/kimi.gateway.membership.v2.MembershipService/GetSubscriptionStats`
  （带 `Referer`/`Origin`/`x-msh-*` 头），回退 `GET https://api.kimi.com/coding/v1/usages`
- 小米 MiMo：`GET https://platform.xiaomimimo.com/api/v1/tokenPlan/detail` 与
  `/usage`；`percent` 是 **0–1 的已用比**（`0.51` = 已用 51%）
- 火山 Agent Plan：`POST https://ark.cn-beijing.volcengineapi.com/?Action=GetUsageDetails&Version=2024-01-01`
  （**数据面域名**，自实现 SigV4；解析 `WithinPlan` / 非 `WithinPlan` 分账）
- 自定义源安全边界：强制 https、拦本机/内网/链路本地与危险重定向（SSRF 防护）、
  拒 HTML 响应（提示要抓 XHR 地址而非页面 URL）

`capture.rs` 的抓包手法（读源码核实）：Tauri 内建 webview 打开厂商控制台，注入拦截器
（`PerformanceObserver` 资源时间线 + 打补丁的 `fetch` / `XMLHttpRequest` / `sendBeacon`），
回传走自定义 scheme `tpcap://log/<json>` 的 `Image()` beacon（远程页面无法可靠用 Tauri IPC，
且 Image 不受 CORS 限制）；`grab()` 再用 `cookies_for_url` 取 Cookie，并把候选接口按
`usage|quota|balance|remain|credit|tokenplan|plan` 关键词排序取前 80 条。

百分比口径（`types.rs`）：`percent = remaining / total`（`total > 0` 时）；`unit == "%"` 时
直接取该字段当剩余百分比；账号总体百分比 = 各行的**算术平均**，无加权。

## 2. 同类项目「不手动贴 Cookie」的取数方式矩阵

按用户负担从低到高（均为读源码/读文档核实）：

| 方式 | 代表实现 | 要点 |
| --- | --- | --- |
| 纯 API key（Bearer） | `openchamber`（opencode-go、command-code、deepseek、openrouter、zai、minimax…）、`RainyToken`、`MyTokenDashboard` | 无 Cookie，最省事 |
| 复用 agent CLI 已落盘的凭据 | `openchamber` 的额度模块文档列了完整发现矩阵：Claude Code Keychain/凭据文件、Codex `auth.json`、**OpenCode `auth.json`（`~/.local/share/opencode/auth.json`，2.x 先读其自身凭据库）**、Antigravity accounts 文件、xAI OAuth 条目 | 用户零输入；同一个 opencode 凭据供 `opencode-go` / `command-code` / `xai` 三家复用 |
| OAuth / 设备码 / SSO | Codex/ChatGPT、Claude Code、xAI OAuth；火山 `arkcli auth login volc-sso`（会话约 47h）；官方 `bl auth login --console`（浏览器登录，本地 127.0.0.1 回调） | 登录一次、长期复用 |
| 浏览器 Cookie 自动导入 | `steipete/CodexBar`：设置里 Cookie 来源为「Automatic / Manual」；`BrowserCookieProfiles` 枚举浏览器 profile（Chromium 系 primary/network 双库合并 + Safari），Chromium 族需解密（macOS 钥匙串 Safe Storage，Full Disk Access/授权交互，后台刷新走 no-UI 预检避免弹窗），命中域名后校验会话并缓存（带 fingerprint） | 本生态里做得最完整的一家；各 provider 有专用 importer |
| 内置登录 webview | `MyTokenDashboard` `capture.rs`；`cc-switch`（Tauri 同类） | 应用内登录即抓 |
| 委托官方 CLI | 火山 `arkcli usage plan`、百炼 `bl usage token-plan --output json`、`amp usage`、Grok `grok agent stdio` 的 `x.ai/billing` | 解析 CLI JSON，凭据留给 CLI 自己管 |

补充：**「直接解密本机浏览器 Cookie 库」这条路，本生态里目前只见到 CodexBar 一家在做**
（macOS 实现）；其余要么 API key，要么读 agent 自己的凭据文件，要么 webview 登录。
Windows 上的等价做法是 Chrome/Edge 的 `Local State` + DPAPI 解密，未见现成实现可抄。

## 3. 五家目标的端点与凭据

| 目标 | 端点 | 凭据 | 返回口径 |
| --- | --- | --- | --- |
| **opencode go** | `GET https://opencode.ai/zen/go/v1/usage` | **Bearer，与推理同一把 key** | `usage.{rolling,weekly,monthly}.{status,percent,resetsAt}`，`percent` = **已用**%，`resetsAt` = ISO 时间 |
| **Command Code（GOAT）** | `GET https://api.commandcode.ai/alpha/billing/credits`（另有 `/alpha/whoami`、`/alpha/billing/subscriptions`、`/alpha/usage/summary`、`/internal/usage?limit=100`） | **Bearer API key（与推理同一把，已实测）** | `credits.{monthlyCredits（本账期剩余）,purchasedCredits,freeCredits,belowThreshold,creditThreshold}` + `windowLimits.{limited,exceeded,fiveHour{used,cap,exceeded,resetAt},weekly{…}}` + 顶层 `sandboxAccess/sandboxMinutes`；**`planId` 在 subscriptions 里，credits 里没有** |
| **火山 Ark Coding Plan** | 控制面 `POST https://open.volcengineapi.com/?Action=GetCodingPlanUsage&Version=2024-01-01&Region=cn-beijing`（Agent Plan 同 host `GetAFPUsage`） | **AK/SK 签名 V4**（火山版 SigV4，`ark` service scope）——与推理 key 是两套凭据 | `Result.QuotaUsage[]`：`Level`（`session`/`weekly`/`monthly`）+ `Percent`（**已用**）+ `ResetTimestamp`（秒或毫秒） |
| **百炼 / 千问 Token Plan** | 控制台网关 `POST https://bailian-cs.console.aliyun.com/cli/api.json?action=BroadScopeAspnGateway&product=sfm_bailian&api=zeldaHttp.apikeyMgr.%2Ftokenplan%2Fpersonal%2Fapi%2Fv2%2Fusage`，表单 `params=<json>&region=cn-beijing` | `Authorization: Bearer <console access_token>`（**临时令牌**，不是 AK/SK 签名、也不是套餐 key） | `data.DataV2.data.data.{per5HourPercentage(0–1),per5HourResetTime(ms),per1WeekPercentage,per1WeekResetTime}` |
| **智谱 / z.ai GLM Coding Plan** | `GET {host}/api/monitor/usage/quota/limit`——国际站 `https://api.z.ai`，国内站 `https://open.bigmodel.cn` | **Coding Plan 的 API key**（与推理同一把；但**前缀有分歧**：裸 key vs `Bearer`，见下）；团队版另需 `bigmodel-organization` + `bigmodel-project` | `data.limits[]`：`type` ∈ `TOKENS_LIMIT`/`CREDIT_LIMIT`（Coding Plan 窗口，`percentage` = **已用** %）+ `TIME_LIMIT`（MCP 工具，月窗）；窗口靠 `unit`+`number` 区分（`unit:3`=小时，`unit:6`=周）；`data.level`/`planName` = 套餐档位 |

### opencode go

- **已实测**：无 key → `401` + `{"type":"error","error":{"type":"AuthError","message":"Missing API key."}}`；
  乱路径（`/zen/go/v1/definitely-not-real-xyz`）→ `404`，故此路由确实存在、只受鉴权保护。
  `/zen/go/v1/models` → `200`（Go 套餐的模型清单，`minimax-m3` / `kimi-k3` 等）。
- **读源码核实**（上游 `packages/console/app/src/routes/zen/go/v1/usage.ts`）：用 Bearer key 查
  `KeyTable` 关联 workspace/user；无 key → `401 AuthError`，key 有效但无 Go 订阅行 → `403`
  `EntitlementError: "OpenCode Go subscription required."`；`percent` 取 `usagePercent`（已用），
  `resetsAt = now + resetInSec`。三个窗口分别是 5h 滚动 / 周 / 月。
- 取 key 的自动化路径（读源码核实，OpenChamber）：从 OpenCode 自己的凭据库（回退
  `~/.local/share/opencode/auth.json`）按 provider id `opencode-go` 读 `key`/`token`；
  请求时还带上 `x-opencode-session`、自定义 UA（与本仓库
  [`../opencode-go-gateway.md`](../opencode-go-gateway.md) 记的网关客户端标识要求一致，
  但该用量接口本身不强制）。

### Command Code（GOAT）

**已用真实 key 跑通（2026-09-27，GOAT `individual-goat` 账号）。**

- **已实测**：`/alpha/billing/credits`、`/alpha/whoami`、`/alpha/billing/subscriptions`、
  `/alpha/usage/summary` 四个端点均可用；无 key → `401` + `{"success":false,"error":{"code":"UNAUTHORIZED",…}}`；
  乱路径（`/alpha/definitely-not-real-xyz`）→ `404`，路由真实存在。
  **推理 key（`api.commandcode.ai/provider/v1` 那把）与控制面同一把，实测可查用量。**
- 真实响应结构（逐字取自实测日志，值已脱敏）：

  | 端点 | 结构要点 |
  | --- | --- |
  | `/alpha/whoami` | `{success, user:{id,name,email,userName}, org:<null>}`——`org` 为空，故订阅端点不需要 `?orgId=` |
  | `/alpha/billing/credits` | `credits:{belowThreshold,creditThreshold,monthlyCredits,purchasedCredits,freeCredits}` + `windowLimits:{limited,exceeded,fiveHour{used,cap,exceeded,resetAt},weekly{…}}` + 顶层 `sandboxAccess`/`sandboxMinutes` |
  | `/alpha/billing/subscriptions` | `data:{id,status,currentPeriodStart,currentPeriodEnd,cancelAtPeriodEnd,canceledAt,cancelAt,planId,quantity,priceId,metadata{…},pendingPhase}` |
  | `/alpha/usage/summary` | `totalCount,totalCost,averageCost,successRate,completedCount,failedCount,totalTokensIn, totalTokensOut,totalTokens,totalCredits,totalMonthlyCredits,totalFreeCredits,totalPurchasedCredits,periodBasis` |

- 口径要点：`credits.monthlyCredits` = **本账期剩余**（美元）；窗口 `used`/`cap` 也是美元，
  实例 `cap` 5h = 14、周 = 35，正对 GOAT 档；`summary.totalMonthlyCredits` = **本账期已用**
  （实例 8.89，与 `70 − 61.10` 吻合）；`periodBasis="billing-period"` 说明 summary 按账期累计。
- **月度上限不在任何接口里**，两条推得途径（`scripts-local/commandcode_usage.py` 依次尝试）：
  1. **◇ 剩余 + 本账期已用**：实测 `61.10 + 8.89 = $69.99`，与 GOAT 文档口径 $70 吻合——
     两路独立互证，故该推导可信；
  2. **★ 文档口径表兜底**：`planId` → Go $10 / GOAT $70 / Pro $80 / Max10 $150 / Max20 $300。
- `windowLimits.limited=true` 在用量仅 1–2.7%、`exceeded=null` 时也出现。社区实现注解
  `exceeded` 才是权威拦停判据（直接指出被拦窗口）、`limited` 只是标志位——**官方口径未确认**。
  面板展示建议：只按 `exceeded` 报警，`limited` 单独标注为标志位。
- ⚠️ **实测发现 `canceledAt` 已置位**（`cancelAt` = 本周期结束 2026-10-20）：该订阅已排定
  「到期不续订」。这是账号状态而非接口问题，但面板若要展示订阅信息应带上取消/到期标记。
- 其他实现对照（读源码核实）：`MAXeaglet/commandcode-usage`（Cloudflare Worker，对
  `fiveHour`/`five_hour`/`rolling5h`、`cap`/`limit`、`resetAt`/`resetsAt` 防御式归一）、
  `CATMIAOZHI/RainyToken`（Kotlin）、`openchamber`（凭据取 `COMMAND_CODE_API_KEY` 或
  OpenCode `auth.json`）；CodexBar 走 Cookie 路线（`CommandCodeCookieImporter`，域 `commandcode.ai`）。
  两条路都通，API key 更省事。

配套工具：[`scripts-local/commandcode_usage.py`](../../scripts-local/commandcode_usage.py)
（未入库；手动传 key，`--debug` 输出请求元信息 + 响应结构骨架，密钥只出 `len=… sha256_12=…` 指纹，
`--raw` 的 JSON 亦按键名脱敏）。

### 火山 Ark Coding Plan

- **读源码核实**：`farion1231/cc-switch` v3.16.4 release notes 明写「方舟控制面 OpenAPI
  （`open.volcengineapi.com`）要求账号级 AccessKey 签名、而非推理 API key，用量脚本新增了
  独立的 AK/SK 输入区」；`src-tauri/src/services/coding_plan.rs` 实现签名并注明
  「实测复用推理 Bearer Key 会被网关以 `400 InvalidAuthorization` 拒绝」，字段注释标注
  2026-06-21 实测 `Level` 为 `session`/`weekly`/`monthly`。探测顺序：先 `GetAFPUsage`
  （Agent Plan），未订阅再 `GetCodingPlanUsage`（Coding Plan）。
- **已实测**：用本机 `glm-5.3-flash` 的**推理 key**打控制台路由
  `POST https://console.volcengine.com/api/top/ark/cn-beijing/2024-01-01/GetCodingPlanUsage`
  → `401` + `ResponseMetadata.Error.Code = "NotLogin"`（推理 key 不能用于查询）。
- 备选路径（读源码核实）：控制台 Cookie 路线——
  `POST https://console.volcengine.com/api/top/ark/cn-beijing/2024-01-01/GetCodingPlanUsage`，
  带 `Cookie` + `x-csrf-token`（从 cookie 里的 `csrfToken=` 取）+ 可选 `x-web-id`，
  body `{"ProjectName":"default"}`（`HannibalWangLecter/volcengine-coding-plan-monitor`）；
  或官方 `arkcli usage plan`（SSO 登录）。
- **坑（值得记）**：那个 VS Code 扩展在 AK/SK 路线上失败，是因为把请求签到了**数据面**域名
  `ark.<region>.volcengineapi.com`；能通的是**控制面**网关 `open.volcengineapi.com`。
  这解释了社区里「AK/SK 读不到 Coding Plan」的说法。
- 注意：Coding Plan 与 Agent Plan 是**两个独立订阅**，额度不共享；`/api/coding/v3` 是
  Coding Plan 的推理入口，`/api/plan[/v3]` 是 Agent Plan 的。

### 百炼 / 千问 Token Plan

- **读源码核实**（官方 CLI `modelstudioai/cli`，535 星）：临时 console token 过期后用 AK/SK
  换新——`packages/core/src/auth/refresh-token.ts` 里 `API_ACTION = "GenerateCLIAccessToken"`、
  `API_PATH = "/modelstudio/cli/generateAccessToken"`、`API_VERSION = "2026-02-10"`，host 为
  `modelstudio.cn-beijing.aliyuncs.com`（国际站 `modelstudio.ap-southeast-1.aliyuncs.com`），
  ACS3-HMAC-SHA256 签名，返回 `cliAccessToken`。登录模式有三：`--api-key`、
  `--console`（浏览器登录，本地 127.0.0.1 回调）、`--open-api --access-key-id/--access-key-secret`。
- **读源码核实**（CodexBar 的 `AlibabaTokenPlan*` / `QwenCloud*` 与 docs）：这一家它只用
  Cookie 或已登录 CLI——「Bailian CLI auth: reuses an already signed-in `bl` executable
  without importing browser cookies」「Cookie-based auth: uses browser cookies or a pasted
  `Cookie:` header」，并在 Limitations 里明写「API-key auth … not supported」。它全仓库的
  AK/SK 只出现在 Bedrock 与 Doubao 两个 provider。团队版走
  `GetSubscriptionSummary`（`bailian-cs.console.aliyun.com` / `bailian-singapore-cs.alibabacloud.com`）；
  个人版走 `usage`/`subscription`/`quota-config`（国际站 `home.qwencloud.com`，form 里
  `product=sfm_bailian`、`action=IntlBroadScopeAspnGateway`、`region=ap-southeast-1`，
  外加从 dashboard HTML / `sec_token` cookie / `/tool/user/info.json` 解析出的 `sec_token`）。
- **第三方记录**（cc-switch issue #7484，open、未实现，作者自测 macOS `bl` 1.26.0）：套餐
  `sk-sp-...` key 打控制台网关是 HTTP 200 但 `success:false` / `errorCode=BailianGateway.Login.NotLogined`
  （同一把 key 打模型端点正常）→ **套餐 key 只授权模型调用**；RAM 子账号还需先加入百炼业务空间，
  否则 `BailianGateway.Team.NotAuthorised`；`per5HourPercentage` 缺失表示该窗口可能不限量。
  cc-switch 暂未实现的原因：现有 `execute_usage_script` 是单请求模型，跑不了「签名换 token → 再查用量」两步链路。

### 智谱 / z.ai GLM Coding Plan

**没有 Cookie 路线——这是这一家里最省事的一点**（CodexBar docs/zai.md 首句：
「z.ai and China-mainland GLM Coding Plan are API-token based. No browser cookies.」）。

- **入口与分区**（读源码核实，CodexBar + cc-switch 一致）：

  | 区域 | quota 端点 | 说明 |
  | --- | --- | --- |
  | 国际站 | `GET https://api.z.ai/api/monitor/usage/quota/limit` | Global / z.ai |
  | 国内站 | `GET https://open.bigmodel.cn/api/monitor/usage/quota/limit` | BigModel CN |

  按「你训练/推理用的 base_url」选站：cc-switch 的 `zhipu_quota_base()` 规则是
  `base_url` 含 `bigmodel.cn` → 国内站，否则 → 国际站（**不做跨站回退**，因为两站 key 不通用）；
  CodexBar 用显式「API region」设置 + `Z_AI_API_HOST` / `Z_AI_QUOTA_URL` 覆盖，并禁止跨区覆盖。
- **鉴权：裸 key 还是 `Bearer`，社区实现有分歧**（这是本轮最需要注意的一点）：
  - 裸 key（不加前缀）：cc-switch `query_zhipu()` 明确注释「注意：智谱不加 Bearer 前缀」，
    个人版/团队版都用裸 key；MyTokenDashboard `providers/zhipu.rs` 同样是裸 key（注释 "per Musage notes"）。
  - `Bearer <key>`：OpenChamber 的 `zai.js` / `zhipuai-coding-plan.js`、CodexBar 的
    docs/zai.md（`authorization: Bearer <token>`）都是 Bearer。
  - 可行做法：**先发一种，命中失败信封再换另一种**（成本一次请求）。注意返回**不是** 401/403——
    见下。
- **失败形态特殊（已实测）**：两个 host 缺 Authorization 时都是 **HTTP 200 + JSON 错误信封**：
  `{"code":1001,"msg":"Header中未收到Authorization参数，无法进行身份验证。","success":false}`
  （国际站同码，msg 为英文）。而且**乱路径也返回同样的 1001**——说明鉴权校验先于路由，
  因此**不能用「401 vs 404」判断路径是否存在**（与 Command Code / opencode 的对照法不同）。
  实际判断只能依据响应体 `code`：`1001` = 未带凭据，其他 code = 业务层。
- **响应结构**（读源码核实，三份实现一致）：`data.limits[]`，每条含
  `type`、`percentage`（**已用** %）、`nextResetTime`（epoch ms），窗口身份靠 `unit` + `number`：

  | `type` | 含义 | 关键字段 |
  | --- | --- | --- |
  | `TOKENS_LIMIT` / `CREDIT_LIMIT` | Coding Plan 的 5h / 周窗口（**两者语义相同**，后者是新名字） | `unit:3` = 小时（`number:5` → 5 小时窗口）；`unit:6` = 周（`number:1` 或 `7` 都算每周）；`usage`=总额、`currentValue`=已用、`remaining`（信用卡式额度才有） |
  | `TIME_LIMIT` | MCP 工具额度，月窗 | `percentage`、`nextResetTime`（`unit:5` 在本仓库实现里被当作 1 个月） |

  套餐档位：`data.planName`（或 `plan` / `plan_type` / `packageName` / `level`）。
- **边界处理（照抄 CodexBar 的约定，都是踩过的坑）**：
  - `limits` 为空或无法识别 → **不能显示为 0%**（表示未知，不是没用）；
  - `percentage` 要求整数；若同时有 `usage` 与 `currentValue`/`remaining`，用计数算百分比，最后 clamp 0–100；
  - 多条 Coding Plan 窗口时按窗口时长排序（最长的作次窗口），时长未知的排最后；
  - `TIME_LIMIT` 只作 MCP 独立泳道，**不要**据此编造「月度 Coding Plan 窗口」；
  - 5 小时窗口的 `nextResetTime` 若超出「5h + 1 分钟时钟偏斜」就丢弃（不猜时区），百分比照常显示。
- **团队版（Team Plan）**：只在**国内站**存在。同一 quota 路径加 `?type=2`（小时级模型用量加 `?type=3`），
  并且必须同时带 `Authorization`(+key) + `bigmodel-organization` + `bigmodel-project` 三个头；
  **少一个也会「成功但 limits 为空」**（CodexBar 实测），所以两个 ID 都要当作必填。
  这两个 ID 从 `https://bigmodel.cn/coding-plan/team/usage-stats` 的 XHR 里取。
- 与本仓库的关系：当前 `~/.grok/config.toml` 里的 `glm-5.3-flash` 走的是**火山 Ark** 的
  `/api/coding/v3`，不是智谱 Coding Plan；要查这套接口得另有一把智谱 / z.ai 的 Coding Plan key。

## 4. 其他国内厂商的套餐用量接口（小米 MiMo 除外）

> **收录标准**：模型口碑 + 套餐性价比都值得看的国产厂商。按此标准，本轮只补 **Kimi 与
> MiniMax** 两家（智谱 GLM / 阿里百炼 / 火山 Ark 已在 §3）。讯飞、腾讯云 TokenHub、
> 百度千帆、阶跃 StepFun 模型口碑不足，Cline / ZenMux 不是国内模型厂商（后者是聚合网关），
> 均已剔除。

### 4.1 端点与凭据（读源码核实）

| 厂商 / 套餐 | 端点 | 凭据 | 窗口与口径 |
| --- | --- | --- | --- |
| **Kimi For Coding**（月之暗面） | `GET https://api.kimi.com/coding/v1/usages` | `Authorization: Bearer <Kimi Code API Key>`（控制台 `kimi.com/code/console`；国际站 `kimi.ai/code/console`） | 5 小时 + 周 + 月度 Total 池；cc-switch 解析 `limits[].detail{limit,remaining,resetTime}` → 5h、根 `usage{limit,remaining,resetTime}` → 周；**接口给的是 limit/remaining 计数，百分比要自己按 `(limit-remaining)/limit` 算** |
| **MiniMax Coding Plan** | `GET https://api.minimaxi.com/v1/api/openplatform/coding_plan/remains`（国内）/ `https://api.minimax.io/v1/api/openplatform/coding_plan/remains`（国际） | `Authorization: Bearer <key>` | `model_remains[]` 里挑编程桶（cc-switch 取 `model_name == "general"`，OpenChamber 用 `/^minimax-m/i`）→ `current_interval_remaining_percent`（5h）、`current_weekly_remaining_percent`（需 `current_weekly_status == 1` 才算激活，否则该套餐无周限）、`end_time`；**返回剩余百分比，要反转**；业务错误在 `base_resp.status_code` |

MiniMax 的两条实现细节（来自实现注释，非本机实测）：

- **国内新推理域名 `api.minimax.cn` 没有额度接口的公开出处**，cc-switch 沿用旧域名
  `api.minimaxi.com`（同一账号体系与 key）；国际站为 `api.minimax.io`。
- `remains_time` 单位是**毫秒**（OpenChamber 注释：实测 9664502 ms ≈ 2.68h，与其
  `remaining_percent` 自洽），可用它反推窗口长度。

Kimi 的完整凭据矩阵（CodexBar docs/kimi.md）：**API key（首选）/ Kimi Code CLI 会话 /
自动浏览器 Cookie / 手动 Cookie** 四种。另有 Moonshot 开放平台（按量付费）：
**只有余额、没有窗口**（CodexBar 原话 "no session or weekly window"）。

### 4.2 百分比语义对照（实现时最容易踩的一项）

| 语义 | 厂商 |
| --- | --- |
| 接口给**已用**百分比 | opencode `percent`、Ark `Percent`、百炼 `per*Percentage`、GLM `percentage` |
| 接口给**剩余**（百分比/比例），需反转 | MiniMax `current_*_remaining_percent` |
| 接口给**计数**（limit/remaining），需自己算 | Kimi `limit`/`remaining`、火山 Ark Agent Plan `GetUsageDetails`（只有用量、无上限）、`GetAFPUsage`（Quota/Used） |

## 5. 若要支持上：结论
- **最省事（一把 key，完全不用 Cookie）**：opencode go、Command Code、**GLM Coding Plan**
  （后者唯一的小坑是 Bearer 前缀要探一次）。
- **需要第二套凭据（AK/SK）**：火山 Ark Coding Plan——签名代码可复用（本仓库
  `MyTokenDashboard` 式实现已有同类），只需换 host（`open.volcengineapi.com`）+ Action，
  外加一个 AK/SK 输入区 + IAM 密钥管理页链接。
- **最麻烦（两步链路）**：百炼 Token Plan——AK/SK 只能换临时 token，真正的查询靠 token；
  或退回 Cookie / CLI 会话。
- **「绝不贴 Cookie」能否成立**：opencode go / Command Code / GLM 天然成立；Ark 走 AK/SK 成立；
  百炼走 AK/SK（两步）或依赖已登录 `bl` CLI，否则必须 Cookie。
- **落点注意**：MyTokenDashboard 的「通用自定义源」只出一行，而这几家都是多窗口
  （5h/周/月）——要支持就得做专 provider，照它现有 `volcengine.rs` 的风格（多行 +
  bar/meter + `reset_at`）。百分比语义要按「已用」再转剩余（opencode `percent`、
  Ark `Percent`、百炼 `per*Percentage`、小米 `percent`、GLM `percentage` 都是**已用**）。

## 6. 待确认清单（各一条命令即可定案，尚未执行）

- [ ] opencode go：本机那把 key 是否有 Go 订阅资格（免费层/无订阅预期 `403 EntitlementError`）。
- [x] Command Code：~~本机 `api.commandcode.ai/provider/v1` 的推理 key 能否过 `/alpha/*`~~
      **已确认可用**（2026-09-27，GOAT 账号实测，无 key 401 / 乱路径 404 对照）。
- [ ] 火山 Ark：拿到 AK/SK 后实测 `GetCodingPlanUsage` 的字段与 `ResetTimestamp` 秒/毫秒。
- [ ] 百炼：无账号，`GenerateCLIAccessToken` 与 `per5HourPercentage` 语义未复现。
- [ ] GLM Coding Plan：有智谱 / z.ai key 后确认 ①`Authorization` 到底要不要 `Bearer` 前缀
      （先用裸 key，失败信封 `code:1001` 再换 Bearer）、②真实 `limits[]`（几个窗口、
      `unit/number` 取值、有无 `CREDIT_LIMIT`）、③团队版三头缺一的空 `limits` 现象。
- [ ] Kimi / MiniMax：端点与字段均来自源码阅读（cc-switch + OpenChamber / CodexBar），
      未用真实 key 跑过。

## 7. 未验证 / 存疑提示

- 上述 issue #7484 与 VS Code 扩展的实测细节均为第三方记录，本机未复现。
- CodexBar 的 Alibaba/Qwen 实现与文档只做了源码/文档阅读，未实际运行。
- CodexBar 的浏览器 Cookie 自动导入是 macOS 实现，Windows 等价方案未逐一核实。
- GLM Coding Plan：**只用真实请求验证到「host 存活 + 缺凭据返回 200/code:1001」**；
  `limits[]` 的具体形态、`Bearer` 前缀、团队版行为均来自 CodexBar docs/zai.md、
  cc-switch `coding_plan.rs`（含单测）与 OpenChamber `zai.js` / `zhipuai-coding-plan.js`
  三份实现的交叉阅读，本机无 Coding Plan key，未实测。
- §4 Kimi / MiniMax：全部为**源码/文档阅读**（cc-switch `coding_plan.rs`、
  CodexBar docs/{kimi,minimax}.md、OpenChamber `minimax-shared.js`），本机没有这两家的
  套餐 key，**零实测**。
