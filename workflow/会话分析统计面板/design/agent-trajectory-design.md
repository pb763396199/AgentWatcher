---
schema_version: 1
protocol: 1.3.0
artifact: design
artifact_id: ar_01M5C2MDZ5000D571EWBPNCG2V
work_item_id: wi_3KXFZQMQ9AH2XQ8PGQBW299E2P
created_at: 2026-10-20T08:05:00Z
producer: aes-brainstorm
result: superseded
supersedes: null
dependencies:
  work_item_contract_digest: sha256:288b282963a92773fb16e68c5fd502e3994c05fcdb071bb4aee77785c21e33b2
  artifacts:
    - artifact_id: ar_01M4W7K8S60002YW4JZC6E9A5E
      digest: sha256:ff6352642554acccf159ede71a3efaa1251f88838a118c1200c2484c550c301c
      locator: session-lifecycle-design.md
---

# 轨迹泳道：子代理派发与分时段模型归因

## 目标

多智能体会话打开分析视图就能看清编排全貌：什么时刻派了哪些子代理、每个子代理
用什么模型烧了多少 token、并行度如何——不用翻 rollout 目录手工对账。

## 背景

用户以 codex:01a15784（F:\AiProject\adt-scene-agent，多智能体编排会话）实测暴露
缺口：现有视图只有主代理的聚合时序，子代理与模型切换不可见。参照
F:\AiProject\DeepSeekHarness 的轨迹视图（Chrome Network 式三泳道时间轴：
input/model/tools，时间比例 span、滚轮缩放、拖选聚焦、subtool 嵌套、TTFT/解码
分段）。

数据侧实测（2026-10-20，本机真实文件）：

| 数据 | 位置 | 说明 |
| --- | --- | --- |
| 子代理 rollout | 同日期目录 sibling 文件 | `session_meta.payload.source.subagent` 非空即子代理 |
| 精确回链 | `source.subagent.thread_spawn.parent_thread_id` | 直接等于父会话 thread id（实测 16/19 个 thread_spawn 挂在本父会话） |
| 审查者 | `source.subagent.other = "guardian"` | 无 parent id，用 cwd+时间窗关联（Codex 桌面自动审查，模型 codex-auto-review） |
| 子代理模型 | 子 rollout 的 turn_context | 与父不同：本例 worker 早期 gpt-6.1-sol、06:36 后切 gpt-6-astra，guardian 用 codex-auto-review |
| 子代理开销 | 子 rollout 的 total_token_usage | 本例 16 worker 合计约 88M 毛输入、11 guardian 约 11M |
| 编排调用 | 父 rollout function_call | spawn_agent×17、send_message×27、wait_agent×17，带时间戳 |

ZCode 的 `task_type='subagent_child'` 会话同构此需求，留作第二批源。

## 非目标

- 不解析子代理间通信内容（send_message 的 message 是加密串）。
- 不做 DeepSeekHarness 的拖选聚焦/滚轮缩放第一版（视图复杂度大；先泳道+悬停，
  交互增强按反馈排）。
- 非 Codex 源第一版只显示主代理泳道与编排标记（无子代理数据则不渲染泳道区）。

## 方案对比

### 子代理视图形态

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 泳道时间轴（借轨迹视图） | 主代理一条泳道+每子代理一条，颜色=模型，左标签带名字/模型/毛输入 | 窗口高度随子代理数增长 | 选定 |
| 只做开销表 | 排行表（原型底部已含） | 丢失时间关系与并行度 | 否（作为配套保留） |
| 甘特折叠树 | 可展开树+甘特 | 交互重 | 否 |

### 子代理发现

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| thread_spawn.parent_thread_id 精确匹配 + guardian 按窗 | Codex 索引 sibling 目录，source.subagent 非空者按 parent id 或（cwd+父时间窗）归属 | 同目录扫描已有先例（rollout 索引 5s TTL 缓存） | 选定 |
| 只 parent_thread_id | guardian 漏掉 | 丢失审查开销 | 否 |
| 时间窗全收 | 会串到同时段其他父会话（实测同目录另有 3 个） | 虚高 | 否 |

### 泳道规模控制

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| worker 一条一泳道 + guardian 聚合成一条 | 本例 18 条泳道可读 | 超大 swarm（>30）需处理 | 选定，>30 时按输入排序截断并给「还有 N」 |
| 全部独立 | 本例 28 条 | 太长 | 否 |

## 选定方案

在现有分析窗口的用量时间轴之下新增「轨迹泳道」区（AC 待补 AC-008）：

1. 主代理泳道：全程 span（模型色），上方细竖线标 spawn/send/wait 编排调用时刻，
   复用既有用户轮次菱形。
2. worker 泳道：每个 thread_spawn 子代理一条，标签=序号+模型+毛输入；span 起点=
   子 rollout 首条时间戳、终点=末条；颜色=模型（复用模型色板，模型过滤徽章联动）。
3. guardian 泳道：同角色聚合成一条（×N + 合计），可一键隐藏。
4. 子代理开销排行表：角色/模型/毛输入/输出/轮次/工具/活跃区间，按毛输入排序，
   带合计行；与泳道联动模型过滤。
5. KPI 行新增：派生子代理数、guardian 数、并行峰值、子代理合计毛输入。
6. 数据命令：`get_session_lifecycle` 返回体扩展 `agents: [{id, role, model,
   startMs, endMs, inputTokens, outputTokens, turns, toolCalls, usage?}]` 与
   `orchestration: [{tMs, kind: spawn|send|wait}]`；Codex 走 sibling 扫描
   （复用 rollout 索引缓存），每个子 rollout 复用既有 codex 聚合与 timeline 解析；
   子代理数量预算 40，超限按毛输入截断标注。

可交互原型：`mockups/agent-trajectory.html`（真实数据：本例 16 worker + 11
guardian + 父会话编排时间戳；泳道悬停明细、guardian 显隐、模型过滤、双语双主题）。

## 边界与失败

- 同目录扫描在会话目录极大时按既有索引缓存节流；子 rollout 缺 token 事件时该
  泳道照画、开销显示"—"。
- 无子代理的会话：整区不渲染。
- guardian 归属用时间窗近似，可能混入同时段其他会话的 guardian，标注「按时间窗
  估算」；thread_spawn 类精确无此问题。

## 影响面

| 项 | 数字 | 依据 |
| --- | --- | --- |
| Rust | lib.rs：sibling 扫描+归属+agents 聚合（约 200 行）+ 结构体扩展 | codex_rollout_file_index 先例 lib.rs:1695 |
| 前端 | ui/index.html：泳道区+排行表+KPI 扩展（约 350 行） | 既有分析窗口 |
| 需重跑 | cargo test、smoke、codex 相关 flow | 仓库规则 |

## 怎么算做对

- 本例复验：agents 数 16（thread_spawn 精确）+ guardian 按窗；worker 合计毛输入
  ≈88M 与逐文件对账一致；06:36 后泳道颜色切换为 gpt-6-astra。
- 无子代理会话（如 3DGS 会话）不渲染泳道区，视图不回归。

## 未决问题

1. guardian 混入风险是否可接受（方案：时间窗近似+标注）——或第一版只做
   thread_spawn 精确匹配、guardian 后续再议。
2. 泳道排序：按派发时间（推荐，时间叙事）还是按毛输入（开销叙事）。
3. DeepSeekHarness 式拖选缩放是否列入本期（建议后续迭代）。
