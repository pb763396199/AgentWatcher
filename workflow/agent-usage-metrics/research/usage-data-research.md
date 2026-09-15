---
schema_version: 1
protocol: 1.3.0
artifact: research
artifact_id: ar_01M2HX0EEVEXZEF3NZYTJFG22H
work_item_id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
created_at: 2026-09-15T06:46:07Z
producer: aes-research
result: complete
topic: usage-data
supersedes: null
dependencies:
  work_item_contract_digest: sha256:b1d0e0503375d88153a6711d42195454acdbf4b2117b2b4a4a6cca41a6fcc8d9
  artifacts: []
---

# 六个扫描源的用量数据调研

回答两个问题：每个扫描源能拿到哪些用量指标；AgentWatcher 现有代码在哪里接这些指标。
全部结论来自本机真实文件的实测（2026-09-15），复核脚本在
`F:\AiProject\AgentWatcher\.tmp\usage-research\`（01-schema.cjs 到 10-zcode-semantics.cjs，全部只读）。

## 看了哪些地方

| 数据源 | 位置 | 实测规模 |
| --- | --- | --- |
| Claude Code 会话 | `C:\Users\YUMEI\.claude\projects\**\*.jsonl` | 3 个文件：6.5MB、24.0MB（6269 行）、48.8MB（6285 行） |
| Codex 会话 | `C:\Users\YUMEI\.codex\sessions\2026\09\**\rollout-*.jsonl` | 3 个深挖（0.3MB、4.1MB、32.6MB/2895 行）+ 35 个文件测尾距 |
| ZCode 数据库 | `C:\Users\YUMEI\.zcode\cli\db\db.sqlite` | 89.2MB，23 会话 / 2945 消息 / 10982 part |
| ZCode rollout 目录 | `C:\Users\YUMEI\.zcode\cli\rollout\model-io-*.jsonl` | 两次列目录对比，当前 3 个文件 |
| OpenCode 数据库 | `C:\Users\YUMEI\.local\share\opencode\opencode.db` | 4.46GB，15868 会话 / 188323 消息 / 860950 part |
| Copilot Chat 会话 | `...\Code\User\workspaceStorage\*\chatSessions\*.jsonl` | 308 个文件，深挖 6 个（最大 59MB） |
| Copilot Chat 转录 | 同上 `GitHub.copilot-chat\transcripts\*.jsonl` | 6.7MB / 11434 行 |
| Copilot CLI | `...\globalStorage\github.copilot-chat\session-store.db` | 2.99MB，2 个 copilotcli 会话（41+31 轮） |
| AgentWatcher 源码 | `src-tauri/src/lib.rs`、`ui/index.html` | 实际 11872 行 / 16267 行 |
| 来源 Codex 会话 | `...rollout-2026-09-14T17-44-24-01a09f4d-*.jsonl` | 19.6MB / 1343 行（用户此前的 ZCode 用量分析） |

## 查到的：指标可得性总矩阵

「全扫」指头尾采样拿不到、必须读全文件；「SQL」指数据库聚合查询；「尾扫」指从文件尾部
反向读一段就能拿到。"—"表示数据源没有这个字段。

| 指标 | Copilot Chat | Copilot CLI | Claude | Codex | OpenCode | ZCode |
| --- | --- | --- | --- | --- | --- | --- |
| 累计输入 token | 有（每轮 promptTokens 求和，全扫） | — | 有（去重后求和，全扫） | 有（最后一条 total_token_usage，尾扫≥5MB） | 有（session 表汇总列，SQL） | 有（model_usage 表求和，SQL） |
| 累计输出 token | 有（每轮 completionTokens 求和） | — | 有（output_tokens 求和） | 有（output+reasoning_output） | 有（tokens_output 列） | 有（output_tokens 列） |
| 当前上下文 | 有（最新轮 promptTokens） | — | 有（最后一条调用 input+cache_creation+cache_read） | 有（last_token_usage，另有 model_context_window） | 有（最后 assistant 的 tokens.total） | 有（最后 assistant 的 tokens.input） |
| 工具调用数 | 有（toolCallRounds/toolInvocationSerialized，全扫） | 仅文件级（session_files 表，缺终端类工具） | 有（tool_use 块计数，全扫） | 有（function_call+custom_tool_call 计数，全扫） | 有（part type='tool' 计数） | 有（tool_usage 表计数） |
| 用户轮次 | 有（requests 数，排除 isSystemInitiated） | 有（turns 计数） | 有（user 记录过滤注入和 tool_result） | 有（role=user 过滤注入前缀） | 有（role='user' 直数，无注入） | 有（semantics.origin='real_user'，可排除合成消息） |
| 模型调用次数 | 有（toolCallRounds 累计） | 只能用轮次近似 | 有（distinct message.id） | 有（distinct response_id） | 有（assistant 消息计数） | 有（model_usage 行数） |
| 模型名 | 有（modelId/resolvedModel） | — | 有（message.model，可混合多模型） | 有（turn_context.model，可中途切换） | 有（最后 assistant 的 modelID；session.model 列仅 167/15868 有值） | 有（model_usage.model_id+provider_id） |
| 时长 | 有（elapsedMs、首尾时间戳） | 有（created/updated） | 有（首尾 timestamp） | 有（首尾 timestamp） | 有（time_created/time_updated） | 有（墙钟跨度；活跃时长 sum(duration_ms) 另有） |
| 成本 | 部分（copilotCredits，6/39 文件有） | — | — | —（仅 rate_limits.used_percent） | 有（session.cost，实测 $0.43–$199.53） | —（订阅制，cost 恒为 0） |

## 查到的：各源的关键语义和坑

### Claude Code

- assistant 记录的 `message.usage` 是每次 API 调用的增量。同一次调用按 content block 拆成多条记录，
  usage 完全相同：24MB 文件 2225 条 assistant 只有 808 个 distinct `message.id`，不去重会多算约 2.75 倍。
- 公式：累计输入 = Σ(input + cache_creation + cache_read)；累计输出 = Σoutput；
  当前上下文 = 最后一条调用的 input+cache_creation+cache_read（压缩后自动回落，语义正确）。
- 子 agent 在独立文件 `<sessionId>/subagents/agent-*.jsonl`，扫主文件天然不含。
- 头 128KB+尾 256KB 只覆盖 5/808 个调用，累计值必须全量扫。全解析实测 48.8MB 用 65ms（node），
  usage JSON 只占文件约 5%。

### Codex

- `event_msg/token_count` 与 `token_usage_record` 每个 API response 各发一条；`total_token_usage`
  全会话累计、严格单调不减、跨 compaction 不重置；`last_token_usage` 是最近一次请求的量。
  32.6MB 文件 404 条事件终值 input 98.2M（其中 95.8M 为 cached）。
- 当前上下文取 last_token_usage 时要跳过压缩刚结束后的全零 input 事件。
- 35 个文件实测：最后一个 token 事件距文件尾在 256KB 内的只有 10 个，5MB 内 35 个全命中，
  最远 3990KB。累计值读最后一条即可，无需求和。
- `compacted` 记录内嵌压缩后的替代历史，单行可以非常大，是尾采不到 token 事件的主因。
- 2/35 文件（subagent relay 线程）完全没有 token 事件，需按无数据降级。
- `thread_token_usage` 与 `total_token_usage` 终值差约 0.9%，展示口径二选一并注明。

### ZCode

- db 里有专门观测表：`model_usage`（每次模型请求一行，含 input/output/reasoning/cache 分列、
  duration_ms、model_id、query_source）、`tool_usage`（每次工具调用一行）、`turn_usage`。
  `schema_migration` 显示 `0010_usage_observability` 自 0.15.0 存在，本机 db（0.16.5 写入）全历史覆盖。
- assistant 消息的 `data` JSON 也有 `tokens`（cache.read 含在 input 内，total=input+output）和 `cost`
  （恒为 0，订阅制）。
- 合成用户消息可识别：314 条 user 中 250 条 `synthetic:true`（todo_reminder 242 条等），
  `semantics.origin='real_user'` 是最准的过滤键。
- 聚合实测：model_usage 求和 0.95ms/会话，工具 top5 0.15ms，80 会话约 40ms。
- rollout 目录的 model-io 文件每行内嵌完整请求体（130–500KB/行），且两次列目录（间隔 4 分钟）
  文件集合随会话生死变化，结束会话的文件已消失。

### OpenCode

- session 表直接有汇总列：`cost / tokens_input / tokens_output / tokens_reasoning /
  tokens_cache_read / tokens_cache_write`（13789/15868 会话有值），80 会话读取 15.7ms。
- 消息级同构 ZCode，但 total=input+output+reasoning+cacheRead，cacheRead 不含在 input 里（与 ZCode 相反）。
- 无合成消息体系（全库 0 条），role='user' 直数即可。
- 明细聚合贵：单会话工具 top5 的 JSON GROUP BY 92.7ms（6789 part），消息表全表 GROUP BY 9.2s。
  有 (session_id) 覆盖索引的 COUNT 都在 1ms 内。

### Copilot Chat

- chatSessions JSONL 是键值补丁日志：首行全量快照，之后是单点补丁或全量重写（单行可达 22MB）。
  每轮的 `promptTokens / completionTokens / copilotCredits / elapsedMs / modelId` 以补丁行出现。
- promptTokens 是每轮的上下文快照，求和是计费口径，与去重上下文是两回事。
- 尾 256KB 在 50MB 文件拿得到最新一轮指标，59MB 文件要 ≥1MB 尾窗（最后指标距 EOF 1.2–1.7MB）。
- transcripts 是纯事件日志（user.message / assistant.turn_start / tool.execution_start 等），
  没有任何 token/模型字段（全文搜索 0 命中），但有精确的工具调用事件计数。

### Copilot CLI

- session-store.db 全部 13 张表无任何 token/usage/model/cost 列；turns 正文搜索 `usage`/`prompt_tokens`
  0 命中。能拿到的只有轮次数、时长、文件级工具痕迹（session_files 的 tool_name）。

### 来源 Codex 会话的结论（用户此前的分析）

- 该会话核算出 5 个根会话：真实用户输入 34 条、主 Agent 模型请求 1518 次、工具调用 2247 次、
  毛 token 4.18 亿（其中缓存读取 4.09 亿）。它的口径是「每次请求的完整上下文累加」。
- 它当时用 `~/.zcode/cli/rollout/model-io-*.jsonl` 取每请求 token，认为 db 没有 token 字段。

## 查到的：AgentWatcher 仓库挂点

| 挂点 | 位置 | 现状 |
| --- | --- | --- |
| 会话结构体 | lib.rs:95-123 `AgentSession` | 唯一统计字段 message_count:u32；serde camelCase；无任何 token/usage 代码（全文件搜索证实） |
| 各源摘要结构体 | lib.rs:424/450/478/488/505/546 | 六个 summary 结构，需同步加字段 |
| ZCode 摘要 | lib.rs:4284 | 已有精确 `count(*) from message` 先例（lib.rs:4290）；message/part 采样 32 行，budget=8 会话/次 |
| OpenCode 摘要 | lib.rs:3610 | message/part 采样 24 行，budget=4；budget 外走 lightweight 降级（拿不到消息数据） |
| JSONL 采样常量 | lib.rs:24-30 | 头 128KB、尾 256KB、Claude prompt 扫 8MB、Copilot 16MB、transcript 2MB |
| 卡片渲染 | index.html:15599 `createSessionCard` | footText=branch 或 workspaceDiscriminator 或 "N msgs"（15633 行）；compact-cards（<110px）隐藏整个 card-foot |
| 悬浮预览 | index.html:14973 `renderSessionPreview` | 两个 section（最后用户输入/最后 AI 回复）；payload 由前端 `sessionPreviewData`(14953) 从 session 数据拼装，经 `agentwatcher-preview-data` 事件发子窗口 |
| i18n / 主题 | index.html:6344（en）/6391（zh）；body data-theme、data-lang | `t(key)` 取值；新增文案两处字典各加键 |
| 扫描触发 | 文件事件 250ms 防抖（lib.rs:761）+ 前端 15s 轮询；opencode/zcode 有 10s 结果缓存 | 用量采集挂进既有 scan 管线即可，事件机制不用改 |

## 由此推断

- ZCode 的 rollout model-io 文件是活跃会话的滚动调试缓冲，不是持久数据；用量功能应建在
  `model_usage` 表上。依据：文件集合随会话生死变化、无配置开关、db 内观测表数据完整且便宜。
- OpenCode 的明细指标（模型名、当前上下文、工具 top）适合在用户点开预览时单独查询，
  不放进每次 scan。依据：92.7ms/会话 × 80 会话最坏 7.6s，而 session 汇总列只要 16ms。
- Claude/Copilot/Codex 的累计值聚合需要按（路径、大小、修改时间）做进程内缓存，只在文件变化时
  重算。依据：单文件全解析 37–65ms，无缓存时 80 会话每次扫描都在秒级，轮询间隔只有 15 秒。
- 展示毛输入 token 时应同时给缓存占比，否则会重复用户在 ZCode 桌面端已经踩过的困惑
  （4.18 亿毛 token 里 98% 是缓存读取）。依据：来源会话的完整归因分析。
- Copilot CLI 的用量区块会几乎全空（只有轮次和时长），这是数据源上限，做不了假数据。

## 哪里还有矛盾

- 来源 Codex 会话说「ZCode db 没有 per-message token」，实测有（model_usage 表 + message.data.tokens，
  schema 0010 自 0.15.0）。它 4.18 亿的毛 token 口径与 model_usage 聚合应当一致，未逐会话对数，
  属判断。
- OpenCode 的 session.model 列覆盖率 167/15868，与消息级 modelID 的可得性矛盾，
  模型名应取最后一条 assistant 消息。
- Codex thread_token_usage 与 total_token_usage 差 0.9%（32.6MB 文件实测），
  原因推测含 compaction 调用，未逐条核对，展示时选 total_token_usage 并在文案里写清口径。
