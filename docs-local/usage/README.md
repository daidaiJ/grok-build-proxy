# 用量 / 额度专题（docs-local/usage）

> **状态（2026-09-27）：新建目录，内容为调研记录，尚未提交推送。**

## 用途

收录「各家 AI 编程订阅（Coding Plan / Token Plan / Agent Plan）与模型服务商的用量、
额度、百分比怎么拿」这一主题的调研记录：端点、凭据形式、返回口径、客户端取数模式，
以及落到本仓库面板（`/usage`）或同类第三方面板时的可行性结论。

## 范围

- **在范围内**：第三方额度面板怎么取数（含凭据获取方式）；各订阅套餐的用量接口与
  鉴权；「不手动贴 Cookie」的替代方案；某条路线是否值得实现。
- **不在范围内**：模型推理侧的计费/限流头解析（见
  [`../limit-inference.md`](../limit-inference.md)）；本机 `/usage` 面板的额度估算实现
  细节（见 [`../usage-quota-estimate-todo.md`](../usage-quota-estimate-todo.md)）。

## 文档索引

| 文档 | 内容 |
| --- | --- |
| [`quota-endpoints-and-credentials.md`](quota-endpoints-and-credentials.md) | 主记录：MyTokenDashboard 的取数策略归纳、同类项目的凭据获取方式矩阵、opencode go / Command Code / 火山 Ark Coding Plan / 百炼 Token Plan / 智谱·z.ai GLM Coding Plan 五家的端点与凭据，另补 Kimi / MiniMax 两家国产套餐（模型口碑与性价比达标；小米 MiMo 按约定除外），含已验证 vs 未验证标注与待确认清单。**Command Code 已用真实 key 端到端验证**（含配套探针 `scripts-local/commandcode_usage.py`） |
| [`provider-quota-display-todo.md`](provider-quota-display-todo.md) | 施工方案：供应商套餐用量上 `/usage` 面板（替代 SuperGrok 块 + 来源标注）。用户已拍板：窗口粒度 5h>周>月、只支持与推理同一把 SK（Ark/百炼凭据不符出局）、缓存默认 5 分钟可配（单位分钟）。含现状锚点 / 数据模型 / 分期 T1–T4 / 验收标准 / 待定决策；**供应商清单待拍板后开工** |

## 与本目录同主题的既有文档（仍在 docs-local 根目录）

尚未迁移——迁移需同步改 AGENTS.md 与相互引用的相对路径，留待后续统一处理：

- [`../quota-pct-jump-audit.md`](../quota-pct-jump-audit.md) — 统一池周用量 7%→76% 跳变排查
- [`../usage-quota-estimate-todo.md`](../usage-quota-estimate-todo.md) — `/usage` 周额度 token 估算（T1/T2/T3）
- [`../limit-inference.md`](../limit-inference.md) — 供应商 cache TTL / RPM / TPM 反推
- [`../opencode-go-gateway.md`](../opencode-go-gateway.md) — OpenCode Go 网关接入（UA / 会话头要求）
- [`../model-limits-config.md`](../model-limits-config.md) — 自定义模型 context / max_completion_tokens 兜底语义
- [`../status-line-perf-wiki.md`](../status-line-perf-wiki.md) — 状态行 ttft/tps（含 unified.jsonl 取证）

## 可信度约定

本目录文档统一使用三档标注：

- **已实测** — 本机发出的请求/命令有输出为证（记下日期与判断依据，如 401 vs 404 的对照）。
- **读源码核实** — 在可信来源（官方仓库、被广泛使用且已发版的项目）里读到该行为，注明文件路径。
- **第三方记录** — 来自他人 issue / 博客 / 未复现的实测，必须显式标注，不得直接当结论使用。
