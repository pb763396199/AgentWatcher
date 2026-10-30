---
schema_version: "1"
protocol: "1.3.0"
artifact: "research"
artifact_id: "ar_51YQVBEYQZ5KTHC6V0BP3BT5B9"
work_item_id: "wi_5VYQZTXYZD0H5M6CN9TKTRFJC3"
created_at: "2026-10-29T08:06:42Z"
producer: "aes-research"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:6848ab9fb6f89246f58decd1503a13d728d7aa614109393c23af3b6f2ffae203
  artifacts: []
result: "complete"
topic: provider-audit-signals
---

# 六个 provider 的逐轮审计信号调查

回答一个问题：每个被监控 agent 的本地会话数据里，有没有「每一轮是谁在回答、什么档位、花了多少、有没有被换模型或重试」的痕迹。全部结论来自 2026-10-29 本机实测，每条给出数据源位置和字段名。

## 看了哪些地方

| provider | 数据源 | 本机状态 |
| --- | --- | --- |
| Claude Code | `~\.claude\projects\*\*.jsonl`（557 个文件，取最新一个 379 行实测） | 有活跃数据 |
| Codex | `~\.codex\sessions\2026\10\29\rollout-*.jsonl`（取 14:41 会话 1010 行实测）；`~\.codex` 目录结构 | 有活跃数据 |
| OpenCode | `~\.local\share\opencode\opencode.db`（10.6 GB，SQLite 只读） | 有活跃数据 |
| ZCode | `~\.zcode\cli\db\db.sqlite`（754 MB，SQLite 只读） | 有活跃数据 |
| Copilot Chat | `%APPDATA%\Code\User\workspaceStorage\*\chatSessions\*.jsonl` | 本机当前没有文件；字段口径来自 AgentWatcher 自己的解析代码和内嵌测试样例（lib.rs 约 16522 行的 fixture） |
| Copilot CLI | `%APPDATA%\{Code, Code - Insiders}\User\globalStorage\github.copilot-chat\session-store.db`（2.99 MB） | 有活跃数据 |

## 查到的：逐轮信号矩阵

| 信号 | Claude | Codex | OpenCode | ZCode | Copilot Chat | Copilot CLI |
| --- | --- | --- | --- | --- | --- | --- |
| 每轮实际模型 | 有（`message.model`，实测 `claude-opus-5-5`） | 有（`turn_context.model`，实测 `gpt-6.1-sol`） | 有（`message.data.modelID`，实测 `moonshotai/kimi-k3.1-pro`） | 有（`model_usage.model_id`，实测 `GLM-5.3`） | 有（`requests[].modelId` 与 `result.metadata.resolvedModel`） | 无 |
| 请求的模型 vs 实际模型 | 有（顶层 `requestedModel`，另有 `advisorModel`） | 无直接字段；轮间 `turn_context.model` 变化可观测 | 无请求字段；轮间 `modelID` 变化可观测 | 无请求字段；`variant` 列 + 轮间变化可观测 | 有（`modelId` 带 `copilot/` 前缀，`resolvedModel` 是解析后的） | 无 |
| 档位/变体 | 有（顶层 `effort`、`perTurnEffort`） | 有（`turn_context.effort`，实测 `high`） | 有（`data.variant`） | 有（`model_usage.variant`，实测 `max`） | 无 | 无 |
| token 用量 | 有（`usage` 全量，含 `thinking_tokens`、cache 读写） | 有（`token_usage_record`，该会话 117 条） | 有（`data.tokens`，含 `reasoning` 与 cache） | 有（`model_usage` 各 token 列，含 `reasoning_tokens`） | 有（`promptTokens`、`completionTokens`、`copilotCredits`） | 无 |
| 美元成本 | 无 | 无 | 有（`data.cost`，实测 0.0774） | 无（可由 token 推） | 无（credits 计） | 无 |
| 时延 | 有（`thinkingDurationMs`；usage 里 `iterations`） | 有（`token_usage_record` 的时间） | 有（`data.time.created/completed`） | 有（`duration_ms`、`time_to_first_token_ms`） | 有（`timestamp` 与 `modelState.completedAt` 相减） | 仅轮时间戳 |
| 重试/错误 | 有（`requestId`；usage `fallback_credit`、`speed`、`service_tier`） | 实时通知有（`model/rerouted`、`model/safetyBuffering/updated`），落盘无 | 无专门字段（`data.finish`） | 有（`retry_count`、`error_type/error_code`、`status`、`attempt_index`、`context_exceeded`） | 无 | 无 |
| 思考的痕迹 | 有（content 里 25 个 `thinking` 块，文本可见） | 有条件（`summary` 数组；实测该会话 `summary:"none"` 所以为空，原文永远只有 `encrypted_content` 密文） | 无文本（只有 `reasoning` token 数） | 无文本（`reasoning_tokens` 计数） | 无 | 无 |

## 查到的：关键细节

- ZCode 的 `model_usage` 表是六源里最完整的逐调用审计表，除上表字段外还有 `first_token_at`、`tool_call_count`、`provider_metadata_json`；另有 `turn_usage`（按轮聚合，含 `model_retry_count`、`tool_error_count`）和 `tool_usage`（按工具调用审计）两张表。
- Claude 每条 assistant 记录自带 `requestId`、`requestRef`，usage 里有 `service_tier: "standard"`、`speed: "standard"`、`fallback_credit: null`——这三个是计费侧的降级痕迹，值得原样展示。
- Codex 的 `model/rerouted {from_model, to_model, reason}`、`model/safetyBuffering/updated {faster_model, reasons, show_buffering_ui}`、`turn/moderationMetadata` 是 openai/codex 开源协议（`codex-rs/app-server-protocol/src/protocol/v2/model.rs`）里的通知类型，只在 app-server 连接存续时可见，rollout 文件不落盘。
- Copilot CLI 的 `turns` 表只有 `id, session_id, turn_index, user_message, assistant_response, timestamp` 六列——比 AgentWatcher 文档里「无用量」的口径更严：连模型名都没有。

## 推断的

- 六源都能映射到统一审计结构（缺的列填 null）；Copilot CLI 是唯一整源返回「不支持」的。
- 「模型被换了」在四个源上可直接或间接观测：Claude 的 `requestedModel` vs `message.model`、Copilot Chat 的 `modelId` vs `resolvedModel` 是请求与实际两侧都有；Codex、OpenCode、ZCode 靠轮间 `model` 列变化。
- Codex `turn_context` 每轮一条（实测 5 条对 5 个 `task_started`），可当作逐轮模型档案；这个推断实现时要再核。
- 没有任何源提供安全缓冲改写（safety buffering）的落盘痕迹；第一版审计以落盘数据为界，实时通知列为后续可选通道。

## 还有矛盾或没核到的

- Copilot Chat 本机当前没有会话文件，字段口径来自 AgentWatcher 自己的测试样例与解析代码（lib.rs 约 2296-2302 行同时收 `modelId` 与 `resolvedModel`）。实现阶段必须拿真实文件核对一次，AC-003 已把这条列为验收。
- Claude `usage.speed` 的取值集合只见到 `standard`，其它值（如降级到 `priority` 的反面）没有样本，先原样展示不参与判定。
