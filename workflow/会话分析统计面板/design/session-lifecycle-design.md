---
schema_version: 1
protocol: 1.3.0
artifact: design
artifact_id: ar_01M4W7K8S60002YW4JZC6E9A5E
work_item_id: wi_3KXFZQMQ9AH2XQ8PGQBW299E2P
created_at: 2026-10-14T08:20:00Z
producer: aes-brainstorm
result: accepted
supersedes: null
dependencies:
  work_item_contract_digest: sha256:288b282963a92773fb16e68c5fd502e3994c05fcdb071bb4aee77785c21e33b2
  artifacts:
    - artifact_id: ar_01M2HX0EEVEXZEF3NZYTJFG22H
      digest: sha256:38cde2a63b7167a062909990e45afe79f1fb4089bf2e4875b9f3565011efa826
      locator: ../agent-usage-metrics/research/usage-data-research.md
---

# 单会话全生命周期分析视图

## 目标

用户点开任意一个会话就能看清它从头到尾的用量轨迹：token 什么时候烧的、上下文怎么涨、
什么时候被压缩、模型和工具怎么分工，一次讲完这个会话的一生。

## 背景

用户第二轮原话把方向从「所有会话放进一个大统计表」修正为「先做每一个会话的全生命周期
分析」，并给了 t3 code 的单模型用量详情作直接参照：模型名加占比的头部、四个大数字
指标、按时间轴的面积图（折线加半透明填充，小时级刻度）、底部「按类型」分段条。
跨会话聚合面板的设计保留在本任务 `design.md`，实施排在本视图之后。

单会话时序数据可得性（继承上一任务实测调研加本次代码核对）：

| 数据 | Claude | Codex | Copilot Chat | OpenCode | ZCode | Copilot CLI |
| --- | --- | --- | --- | --- | --- | --- |
| 每次模型调用的 token | 有（assistant 记录 usage，按 message.id 去重） | 有（token_count 事件） | 有（每轮 promptTokens/completionTokens） | 有（assistant 消息 data.tokens） | 有（model_usage 每行，lib.rs:13836 列清单） | — |
| 每次调用的时间戳 | 有（记录 timestamp） | 有（事件 timestamp） | 有（补丁行时间） | 有（message.time_created，lib.rs:4944） | 有（started_at/completed_at，lib.rs:5368） | — |
| 每次调用的当前上下文 | 有（input+cache_creation+cache_read） | 有（last_token_usage；窗口上限 model_context_window 也有） | 有（promptTokens 即上下文快照） | 有（assistant tokens.total） | 有（input_tokens 毛口径） | — |
| 压缩/回落信号 | 有（上下文骤降即压缩后） | 有（compacted 记录） | 有（全量重写行） | 有（上下文回落） | 有（上下文回落） | — |
| 工具调用时间点 | 有（tool_use 块时间戳） | 有（function_call 事件） | 有（transcripts 工具事件） | 有（part.time_created，lib.rs:4983） | 有（part.time_created） | 仅文件级 |

结论：五个源都能逐调用画出时间轴，ZCode 最完整（观测表每行就是一次调用），
Copilot CLI 只能降级显示轮次与时长。

## 非目标

- 不做对话正文的回放或搜索；时间轴上只标事件，不展开内容。
- 不自建价目表把 token 折算成钱；成本只显示数据源自带的（OpenCode 美元、Copilot credits）。
- 不做跨会话对比；那是阶段 B 的事。
- 不实时流式刷新单会话视图；随既有 250ms 防抖事件整图重算即可。

## 方案对比

### 视图载体

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| analytics-panel 单会话模式 | 同一个分析窗口，带会话 ID 打开进入单会话模式；聚合模式留作阶段 B | 一个窗口两种模式，状态隔离要小心 | 选定 |
| 独立 session-analytics 窗口 | 再开一类窗口 | 多一套 capability 与几何记忆；与聚合模式割裂 | 否 |
| 塞进悬浮预览 | 预览窗加长 | 预览是轻量悬停件，塞时间轴违背其定位 | 否 |

### 时间轴主图形态

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 按模型分色堆叠面积图 + 上下文曲线双图 | 参照截图的面积形态，上图画「每时间桶毛输入 token」按模型堆叠，下图画「当前上下文」折线与窗口上限参考线 | 两张 SVG | 选定 |
| 单图双轴 | token 与上下文画同一张图共用时间轴、双 y 轴 | 双轴读图负担重，量纲差异大 | 否 |
| 甘特泳道（每次调用一条） | 每次模型调用一个线段 | 调用上千次时泳道不可读 | 否 |

### 时间粒度

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 自动 + 手动三档 | 跨度 ≤2 天用小时、≤7 天用 6 小时、更长用天；用户可手动覆盖 | 一档切换控件 | 选定 |
| 只有自动 | 少一个控件 | 高峰分析时用户想换粒度，做不了 | 否 |

### 数据命令

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 新命令 `get_session_lifecycle({ id })` | 一次返回该会话全部时序点（每次调用：时间、模型、输入、缓存、输出、当前上下文）加分解汇总；JSONL 源走既有增量缓存（聚合时顺带保留逐调用数组），ZCode/OpenCode 会话内 SQL | 缓存值变大（逐调用数组）；单会话点数预算 5000，超限把时间轴降级为分桶口径、曲线只留峰值点 | 选定 |
| 复用 get_session_usage_detail 扩展 | 往懒加载命令里塞时序 | 该命令是预览悬停路径，变重会卡悬停 | 否 |
| 前端逐桶拉取 | 分页拉时间轴 | 交互代码复杂，单会话数据量不需要 | 否 |

## 选定方案

### 视图结构（自上而下，对照 AC-001 到 AC-007）

1. 头部：会话标题、提供商徽标、工作区、状态、时间跨度（首条到最近一条内容时间），
   右侧关闭。多模型时列出模型名与各自占比。
2. 指标行：毛输入（旁标缓存占比）、输出、模型调用、工具调用、用户轮次、活跃时长
   （旁标跨度）、成本（有则显示，标单位）。缺项"—"。
3. 用量时间轴：按模型分色堆叠面积图（或柱状，桶内多模型堆叠），y 轴毛输入 token，
   x 轴会话生命期；图例即模型勾选过滤器；悬停出当桶各模型输入/输出/调用数。
   桶数随粒度变化，空桶保留刻度。
4. 上下文占用：折线画每次模型调用时刻的当前上下文 token；有窗口上限的源画上限参考线；
   压缩回落（骤降 >40%）在回落点标注「压缩」。
5. 活动节奏条：时间轴下方一条窄带，标用户轮次（菱形）、工具调用密度（按桶深浅）、
   活跃/空闲区间（活跃段着色，空闲段留白，5 分钟阈值与既有口径一致）。
6. 结构分解（底部一行三组，参照截图的分段条）：按类型（毛输入/缓存读取/输出）、
   按模型（各模型 token 与调用占比）、按工具 top5（次数横条）。

可交互原型稿：`mockups/session-lifecycle.html`（样式取自 ui/index.html 真实 CSS 变量，
顶部控制条可切语言、主题；样本数字取自上一任务实测口径：286.4M 毛输入、98% 缓存、
1,180 次模型调用、2,247 次工具调用、26.7 小时跨度 4.7 小时活跃）。

### 数据侧

- `get_session_lifecycle({ id })` 返回 `{ identity, kpis, timeline: [{ t, model, in, cache, out, context }], buckets, breakdown: { byType, byModel, byTool } }`。
- JSONL 源：聚合器在增量缓存值里保留逐调用精简数组（时间、模型、四类 token、当前上下文），
  只对发生变化的文件重算；缓存键不变。
- ZCode：`model_usage` 按 session 全量行读取（实测毫秒级），`part` 表给工具时间点；
  OpenCode：assistant 消息按 time_created 读 token，行数进预算。
- 超预算（5000 点）降级：时间轴改为按桶聚合返回，上下文曲线降为桶末值；头部标注「时序已降级」。
- 刷新：打开时拉一次，随 `agentwatcher-sessions-changed` 重拉；localStorage 只存窗口几何。

## 边界与失败

- 单点会话（只有一次调用）：时间轴画单柱，曲线画单点，不报错。
- 跨度超过 30 天的长寿会话：默认「天」粒度起步，图表仍可读；不截断数据。
- Copilot CLI：隐藏时间轴与上下文图，保留头部、指标行里可用的轮次/时长与分解里的"—"。
- 模型名缺失的源：时间轴单色「未知模型」一层。
- 查询失败：视图显示可重试错误条，主窗口与预览不受影响。

## 影响面

| 项 | 数字 | 依据 |
| --- | --- | --- |
| Rust 改动 | 1 个文件：lib.rs（三个 JSONL 聚合器保留逐调用数组、两处 SQLite 加会话内时序查询、新命令与注册） | 聚合挂点 lib.rs:1334/1491/1663、注册表 lib.rs:3656 |
| capabilities | 1 个新文件 analytics-panel.json（阶段 A、B 共用） | 照 performance-panel.json 样板 |
| 前端改动 | 1 个文件：ui/index.html（窗口、视图 UI、两张 SVG 图与节奏条、i18n 两处字典各约 30 键、预览与右键入口） | 预览渲染 index.html:14973、右键菜单为待补样板 |
| 需重跑的测试 | cargo test 全量；npm run test:tauri:smoke；改 OpenCode/ZCode 采集时对应 flow | 仓库 AGENTS.md 规则 |

## 怎么算做对

- ZCode 样本会话：时间轴各桶相加等于 286.4M 毛输入；上下文曲线末点等于卡片显示的当前上下文。
- Claude 24MB 样本：逐调用数组与独立脚本按 message.id 去重的结果一致。
- 压缩回落：找一个发生过 compaction 的 Codex 会话，回落点标注出现且幅度与 compacted 记录吻合。
- Copilot CLI 降级布局不破版；中英文、dark/light、长标题各过一遍。

## 未决问题

1. 时间轴堆叠的 y 轴口径：毛输入（含缓存读取，推荐，与卡片和阶段 B 一致）还是净输入。
   净输入会让 98% 缓存的会话看起来「没怎么烧」，但毛输入的高峰几乎全是缓存重读，
   两层信息靠「类型分解」补齐。
2. 上下文曲线与时间轴是否联动（悬停同步竖线）：推荐做，代价是两图共享一个悬停层。
3. 活动节奏条是否第一版就做：它承担「空闲占比」的可视化，信息量大但纯新增；
   备选是第一版只在指标行给「活跃 4.7h / 跨度 26.7h」文字。
