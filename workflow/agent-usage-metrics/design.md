---
schema_version: 1
protocol: 1.3.0
artifact: design
artifact_id: ar_01M2HX0EKBWVB8P9ZMGGJMPG2T
work_item_id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
created_at: 2026-09-15T06:46:07Z
producer: aes-brainstorm
result: accepted
supersedes: null
dependencies:
  work_item_contract_digest: sha256:b1d0e0503375d88153a6711d42195454acdbf4b2117b2b4a4a6cca41a6fcc8d9
  artifacts:
    - artifact_id: ar_01M2HX0EEVEXZEF3NZYTJFG22H
      digest: sha256:38cde2a63b7167a062909990e45afe79f1fb4089bf2e4875b9f3565011efa826
      locator: research/usage-data-research.md
---

# 会话用量指标的采集与展示

## 目标

用户在卡片和悬浮预览里直接看清每个会话的 token 消耗、工具调用数、对话轮次、模型调用次数，
不用打开各 AI 助手自己的界面。

## 背景

调研记录 `research/usage-data-research.md`（ar_01M2HX0EEVEXZEF3NZYTJFG22H）实测了六个扫描源：
五个有真实 token 数据（Copilot Chat、Claude、Codex、OpenCode、ZCode），只有 Copilot CLI 完全没有。
数据库源的聚合查询在毫秒级；JSONL 源的累计值必须全量读文件，单文件 37–65ms（node 实测），
需要增量缓存。本需求的起因是用户此前用 Codex 分析 ZCode 会话的 token 爆炸（4.18 亿毛 token、
1518 次模型请求、2247 次工具调用），这类核算以后在 AgentWatcher 里直接可见。

## 非目标

- 不自建价目表估算成本，只显示数据源自带的成本字段。
- 不做趋势图、历史曲线、按工作区汇总的统计面板。
- 不改各 AI 助手本身，也不解析它们的私有内存状态。

## 方案对比

### 展示位置

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 预览区块 + 卡片紧凑行 | 悬浮预览加「用量」区块放全量指标；卡片在非紧凑密度加一行短指标 | 卡片信息密度上升，紧凑密度下要隐藏 | 选定 |
| 只做预览区块 | 卡片不动 | 少一层改动；但用户不悬停就看不到，与「盯着状态」的产品定位不符 | 否 |
| 卡片直接放全量指标 | 卡片塞下所有数字 | 正方形卡片最小 60px，放不下；紧凑档连 card-foot 都隐藏 | 否 |
| 什么都不做 | 维持现状 | 用户继续靠外部会话核算用量 | 否 |

### JSONL 源的采集（Claude、Codex、Copilot Chat）

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 全量聚合 + 增量缓存 | 进程内按（路径、大小、修改时间）缓存聚合结果，文件没变不重读 | 首次扫描每个文件 37–65ms；要维护缓存失效 | 选定 |
| 尾扫近似 | 只读尾部拿最新值 | Codex 累计可得（尾窗要 ≥5MB）；Claude 和 Copilot 的累计值原理上拿不到，只能放弃 | 否 |
| 只显示当前上下文 | 放弃累计值 | 数字含义弱，回答不了「这个会话总共花了多少」 | 否 |

### SQLite 源的采集（ZCode、OpenCode）

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 汇总进 scan + 明细懒加载 | ZCode 用 model_usage/tool_usage 聚合（80 会话约 40ms）；OpenCode 用 session 汇总列（约 16ms）；OpenCode 的模型名、当前上下文、工具明细在用户点开预览时用新命令单独查 | 新增一个 tauri command | 选定 |
| 全部进 scan | OpenCode 明细也每次扫 | 明细聚合 92.7ms/会话，80 会话最坏 7.6s | 否 |
| 全部懒加载 | 卡片和预览都现查 | 卡片要显示用量行，现查会造成悬停卡顿 | 否 |

### token 口径

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 双轨 + 缓存占比 | 累计毛输入（含缓存读取）与当前上下文都显示，毛输入旁边给缓存占比 | 文案要多解释一句 | 选定 |
| 只显示当前上下文 | 一个数字 | 回答不了总消耗；ZCode 桌面端的困惑就是这个口径引起的 | 否 |
| 只显示累计 | 一个数字 | 分不清「总量大」和「快满了」 | 否 |

### 成本显示

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 有则显示 | OpenCode 显示美元（session.cost，实测 $0.43–$199.53）；Copilot Chat 显示 credits（仅 6/39 文件有）；其余显示"—" | 同一区块内口径不统一，要标单位 | 选定 |
| 一律不显示 | 全部隐藏 | OpenCode 用户损失一个现成的高价值数字 | 否 |

## 选定方案

### 数据结构

`AgentSession`（lib.rs:95-123）新增 `usage: Option<SessionUsage>`，serde camelCase 自动生效：

```text
SessionUsage {
  total_input_tokens: Option<u64>     毛输入（含缓存读取）
  cached_input_tokens: Option<u64>    其中缓存读取的部分
  total_output_tokens: Option<u64>    输出（含推理 token）
  context_tokens: Option<u64>         当前上下文占用
  context_window_tokens: Option<u64>  上下文窗口上限（仅 Codex 有 model_context_window）
  user_turns: Option<u32>
  tool_calls: Option<u32>
  model_calls: Option<u32>
  models: Vec<String>                 去重后的模型名，多的显示「+N」
  duration_ms: Option<u64>            首尾时间跨度
  active_duration_ms: Option<u64>     模型活跃时长（仅 ZCode 有 sum(duration_ms)）
  cost_usd: Option<f64>               OpenCode 的 session.cost
  copilot_credits: Option<f64>        Copilot Chat 的 copilotCredits
}
```

None 与 0 语义分开：None 显示"—"，0 只在数据源真给出 0 时出现。

### 各源的取数公式

| 源 | 累计 | 当前上下文 | 轮次/工具/模型调用 |
| --- | --- | --- | --- |
| Claude | 按 distinct message.id 去重后求和 usage | 最后一条调用的 input+cache_creation+cache_read | user 记录过滤注入；tool_use 块计数；distinct message.id |
| Codex | 最后一条 token_count 的 total_token_usage | last_token_usage（跳过全零事件），对照 model_context_window | role=user 过滤注入前缀；function_call+custom_tool_call；distinct response_id |
| Copilot Chat | 每轮 promptTokens/completionTokens 求和 | 最新轮 promptTokens | requests 排除 isSystemInitiated；toolCallRounds 累计；每轮 modelId |
| ZCode | model_usage 求和 | 最后 assistant 的 tokens.input（过滤 in-flight 行） | semantics.origin='real_user'；tool_usage 计数；model_usage 行数 |
| OpenCode | session 表汇总列 | 懒加载：最后 assistant 的 tokens.total | role='user'；part type='tool'；assistant 计数 |
| Copilot CLI | 全部 None | None | turns 计数；工具仅文件级，不显示；模型调用用轮次近似，不显示 |

### 展示形式

卡片（非紧凑密度）在 card-foot 上方加一行短指标，内容「轮次 · 工具 · token」，例如
`12 轮 · 34 工具 · 890k`；token 用累计毛输入。compact-cards（<110px）及更小密度整行隐藏，
与现有 card-foot 的隐藏规则一致。

悬浮预览在「最后 AI 回复」下方新增「用量」区块，两列小格：

```text
用量
累计输入   330.7M（缓存 98%）     累计输出   459.4k
当前上下文 376k / 1,000k          对话轮次   8 轮
工具调用   887（Bash 724 · Edit 64） 模型调用  829 次
模型       GLM-5.3 +1             活跃时长   4.7h（跨度 26.7h）
成本       —
```

工具明细（top 工具分布）与 OpenCode 的模型名、当前上下文走新命令
`get_session_usage_detail({ id })` 懒加载，预览窗打开时现查。ZCode 的工具 top 便宜
（0.15ms/会话）直接进 scan；OpenCode 的（92.7ms/会话）只在懒加载里做。

可交互原型稿：`mockups/usage-display.html`（样式取自 ui/index.html 真实 CSS，样本数字取自
调研实测；顶部控制条可切换主题、语言、每行卡片数（1–16，能走到 compact/dense/micro 三档）
和拍板点 1–4 的选项；已过视觉复核与 DOM 溢出断言，9 张样本卡覆盖六个扫描源）。

原型复核带出的两条实现注意：真实主窗口默认 396px 宽、2 列，卡片约 185px，用户加列后才进
compact 档，用量行随 card-foot 一起隐藏；预览状态胶囊在 light 主题下的红色（#f14c4c，
对比度 4.6:1）是现有样式遗留，实现时对齐 lane header 的 #a4262c。

### 增量缓存

JSONL 聚合结果按（路径、大小、修改时间）缓存在扫描线程的进程内 Map：文件未变化直接复用；
变化则重算该文件。扫描由文件事件 250ms 防抖和 15s 轮询触发，活跃会话每次事件只重算自己。
Claude 按 message.id 去重、Codex 读最后一条 total、Copilot 按轮求和的规则都在聚合器里实现。

## 边界与失败

- 数据源没有的指标是 None，界面显示"—"，不显示 0。
- 数字格式化：1k / 12.3M / 330.7M，超过窗口上限不特殊处理。
- 首次扫描大文件超时或失败：该会话 usage 置 None，下一轮扫描补齐，不阻塞其他会话。
- Codex 的 subagent relay 线程（2/35 文件）无 token 事件，按无数据降级。
- OpenCode 明细懒加载失败：预览窗对应格子显示"—"，卡片汇总列不受影响。

## 影响面

| 项 | 数字 | 依据 |
| --- | --- | --- |
| 代码文件 | 2 个：src-tauri/src/lib.rs、ui/index.html | 挂点见调研记录「仓库挂点」表 |
| Rust 改动点 | AgentSession + 6 个 summary 结构体 + 6 个扫描函数 + 1 个新 command + 聚合缓存 | lib.rs:95-123、424-546、各 scan 函数 |
| 前端改动点 | 卡片一行指标、预览一个区块、i18n 两处字典各约 10 键 | index.html:15599、14973、6344、6391 |
| capabilities | 0 个文件 | 新命令走核心 invoke 权限，调研已核实 |
| 扫描耗时增量 | ZCode 约 +40ms、OpenCode 约 +16ms、JSONL 增量命中后趋近 0、首扫单文件 37–65ms | 调研实测（node 口径，Rust 预期不慢于 node，判断） |
| 需重跑的测试 | cargo test 全量；flow：smoke、opencode-session、zcode-session、filtering | 仓库 AGENTS.md 的 UI/provider 改动规则 |

## 怎么算做对

- AC-001 到 AC-005 按 work-item 逐条验。
- 抽查对数：ZCode 挑来源会话分析过的 sess_743d9fde，AgentWatcher 显示的累计输入与
  model_usage 表 SQL 直查结果一致；Claude 挑 24MB 样本文件，与按 message.id 去重的
  独立脚本结果一致。
- 数字可解释：预览里毛输入旁必须带缓存占比，避免重蹈 ZCode 桌面端「4.18 亿」的无上下文数字。

## 未决问题

2026-09-15 用户拍板「可以，做吧」，未指定选项，五个点全部采纳推荐：

1. 卡片紧凑行显示哪三个指标 → 轮次+工具+token（方案 A）。
2. 累计与当前上下文谁是主数字 → 累计毛输入（预览主格），当前上下文进网格并带窗口上限。
3. 成本显示范围 → 都显示：OpenCode 美元、Copilot credits 标注单位，其余显示「—」。
4. Copilot CLI 空区块 → 显示，缺项为「—」，附一句无数据说明。
5. 首扫策略 → 渐进填充：每次扫描限量聚合（预算常量），未聚合的会话先显示「—」，后续扫描补齐。
