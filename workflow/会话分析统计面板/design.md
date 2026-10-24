---
schema_version: 1
protocol: 1.3.0
artifact: design
artifact_id: ar_01M4W5SNZM000D2NJH05DYPT3N
work_item_id: wi_3KXFZQMQ9AH2XQ8PGQBW299E2P
created_at: 2026-10-14T03:10:00Z
producer: aes-brainstorm
result: proposed
supersedes: null
dependencies:
  work_item_contract_digest: sha256:288b282963a92773fb16e68c5fd502e3994c05fcdb071bb4aee77785c21e33b2
  artifacts:
    - artifact_id: ar_01M2HX0EEVEXZEF3NZYTJFG22H
      digest: sha256:38cde2a63b7167a062909990e45afe79f1fb4089bf2e4875b9f3565011efa826
      locator: ../agent-usage-metrics/research/usage-data-research.md
---

# 会话分析统计面板：跨会话用量历史与排行

> 2026-10-14 第二轮拍板：本设计保留为阶段 B（跨会话聚合），实施排在单会话全生命周期
> 分析视图（`design/session-lifecycle-design.md`）之后。验收条件对应 work-item 的
> AC-101 到 AC-107。

## 目标

用户在一个独立面板里看清全部 AI 助手会话的用量结构与分布：每天烧了多少 token、
按提供商和模型怎么分、哪些会话最贵，点一下能跳回那个会话。

## 背景

参考物是 Codex 桌面端的「分析」页（用户 2026-10-14 截图）：上半部分是按天堆叠的
用量历史柱状图，可以切 7/30 天、按功能或模型分组、日期前后翻页；下半部分是会话
用量排行表，每行可展开看模型占比和明细，带「打开聊天」跳转。

AgentWatcher 现状（上一任务 `agent-usage-metrics` 交付，2026-09-15）：单会话级用量
已经在卡片一行和悬浮预览区块里（`SessionUsage`，lib.rs:129-156），六个扫描源里五个
有真实 token 数据；该任务明确把「趋势图、按工作区汇总的统计面板」留作后续。本设计
补的就是这一块。

按天数据可行性（继承自上一任务的实测调研 + 本次代码核对）：

| 数据源 | 按天分桶怎么来 | 依据 |
| --- | --- | --- |
| Claude / Codex / Copilot Chat | 聚合器本来就要全量读 JSONL 行，聚合时顺手把每个 token/工具事件按当天落桶，存进既有增量缓存（键：路径、大小、修改时间，lib.rs） | 增量缓存未命中才重读，命中后按天数据零成本 |
| ZCode | `model_usage` 表每行有 `started_at`，SQL 按天 GROUP BY，只对已扫描会话（lib.rs:5368 已在用 `started_at`） | 上一任务实测 0.95ms/会话 |
| OpenCode | `message` 表有 `time_created`，assistant 消息 `data` 里带 tokens；按 session_id 索引范围聚合 | 明细聚合贵（工具 top 实测 92.7ms/会话），需要预算与降级 |
| Copilot CLI | 数据源没有任何 token 字段 | 恒为"—"，沿用上一任务结论 |

一个关键口径决定：会话的累计 token 不能整体记到「最后活跃那天」。跨天会话必须按
事件时间戳分摊到各天，否则图表回答不了「哪天烧的」——这正是 Codex 图表的核心价值。

## 非目标

- 不做配额、额度、预算管理（Codex 有 quota 百分比；AgentWatcher 没有配额概念，不造）。
- 不自建价目表估算成本，只显示数据源自带的成本字段（OpenCode 美元、Copilot credits）。
- 不做对话正文分析；面板里只有用量数字和元数据（标题、工作区、模型名）。
- 不改性能诊断面板（那是 AgentWatcher 自身性能，不是会话分析）。

## 方案对比

### 展示位置

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 新独立分析面板窗口 | 照 performance-panel 的样板加 `analytics-panel` 窗口（index.html:13374 创建样板、capabilities/performance-panel.json 权限样板），主窗口工具条加入口按钮 | 多一个窗口的几何记忆、i18n、主题同步 | 选定 |
| 复用性能诊断面板 | 在性能面板里加页签 | 语义混装：一个管自身性能、一个管会话用量，读者要区分两种「性能」 | 否 |
| 主窗口内嵌 | 主窗口切到分析视图 | 主窗口默认 396px 宽、置顶小卡片墙，塞图表两头都破坏 | 否 |
| 只做 CLI 报表 | agentwatcher-cli 加 analytics 命令 | 没有可视化，不满足本次诉求；可作为后续增量 | 否 |

### 用量历史图的数据

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 按天精确分桶 | JSONL 聚合顺带产出日桶并进增量缓存；ZCode/OpenCode 按天 SQL | 缓存值加一个 `Vec<(day, 桶)>` 字段；OpenCode 要控制查询行数 | 选定 |
| 按活跃天数近似分摊 | 把会话累计值按活跃时长比例摊到活跃日 | 失真：token 消耗在一天内可以极度倾斜，摊出来的图会误导 | 否 |
| 不做时间轴 | 只做排行和汇总 | 丢掉参考设计里信息量最大的部分 | 否 |

### 图表实现

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 手写 SVG/CSS 堆叠柱状图 | 单文件前端内直接画，hover 用原生事件 | 图表交互代码自己维护（预计 300 行内） | 选定 |
| 引入图表库（Chart.js / ECharts） | npm 依赖 + WebView2 离线打包 | 仓库是零框架单文件前端，为一张图引整套库不值；离线资源要本地化 | 否 |

### 排行表明细

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 懒加载复用 `get_session_usage_detail` | 列表行用 scan 已有累计值；展开行时现查 topTools（lib.rs:1995） | 展开行有毫秒级延迟 | 选定 |
| 全量预取 | scan 时把明细全部算好 | OpenCode 明细 92.7ms/会话 × 80 会话最坏 7.6s（上一任务实测） | 否 |

### 分组维度

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 提供商 + 模型两档分组，工作区做过滤器 | 图例与 Codex 的「功能/模型」对应；工作区进顶部下拉，复用主窗口语义 | 少一个分组档 | 选定 |
| 加工作区第三分组档 | 图例可切三档 | 工作区几十个、名字长，堆叠层数和图例都会爆版 | 否 |

## 选定方案

### 面板结构（自上而下）

1. 工具条：时间窗 7/30 天切换、日期前后翻页（回到今天）、分组切换（按提供商 /
   按模型）、工作区过滤下拉、数据时间戳。
2. 汇总卡一行：区间毛输入（旁标缓存占比）、输出、模型调用次数、活跃时长、
   涉及会话数。缺项显示"—"。
3. 用量历史：按天堆叠柱状图，纵轴 token（毛输入口径，与卡片行一致），每天一根柱、
   分组色块堆叠；图例带区间占比；hover 任意一天弹出当天各分组明细（输入、输出、
   调用数）。
4. 会话排行：按区间毛输入降序。行：提供商徽标、会话标题、工作区短名、区间毛输入、
   占区间总量的百分比条。展开：模型分布（token 占比）、top 工具（懒加载）、
   轮次 / 工具调用 / 模型调用、活跃时长、跳转按钮（走既有 `open_session`，
   ZCode 返回「暂不支持」提示）。

可交互原型稿：`mockups/analytics-panel.html`（样式取自 ui/index.html 真实 CSS 变量，
顶部控制条可切语言、主题；数字为示意样本）。

### 数据侧

- 新 Tauri 命令 `get_usage_analytics({ rangeDays, offsetDays, groupBy, workspace? })`
  返回 `{ totals, dailySeries, topSessions }`；`dailySeries` 每天带各分组的
  inputTokens / outputTokens / modelCalls；`topSessions` 每行带区间内 token、
  缓存占比、模型列表。
- 按天分桶落在扫描层：JSONL 聚合缓存值扩展日桶字段；ZCode 一条按天 GROUP BY SQL；
  OpenCode 按已扫描会话读 assistant 消息聚合，单会话消息行数超过预算（5000 行）
  时该会话日桶降级为不可用，累计值不受影响。
- 刷新时机：面板打开时拉一次，之后挂既有 `agentwatcher-sessions-changed` 事件
  （250ms 防抖）重拉当前窗口。
- 隐私：面板数据只含用量数字与元数据，正文不经过面板、不落 localStorage
  （localStorage 只存窗口几何，沿用 `agentwatcher.analyticsPanelGeometry` 键）。

## 边界与失败

- 数据源没有的指标显示"—"，不显示 0（沿用上一任务口径）。
- 窗口内没有活动的天画零高度柱并保留日期刻度，不缩轴。
- OpenCode 日桶降级的会话在排行展开区标注「按天明细不可用」，行本身照常显示。
- 面板查询失败：显示可重试的错误条，不阻塞主窗口。
- 首次扫描大文件期间面板可以先出 SQLite 源的数据，JSONL 源标「聚合中」。

## 影响面

| 项 | 数字 | 依据 |
| --- | --- | --- |
| Rust 改动 | 1 个文件：lib.rs（3 个 JSONL 聚合器加日桶、2 处 SQLite 查询加按天聚合、新命令与注册） | 聚合器挂点 lib.rs:1334/1491/1663（activity_timestamps 已在收集）、注册表 lib.rs:3656 |
| capabilities | 1 个新文件 analytics-panel.json | 照 performance-panel.json 样板 |
| 前端改动 | 1 个文件：ui/index.html（窗口创建、面板 UI、SVG 图表、i18n 两处字典各约 25 键） | performance-panel 创建样板 index.html:13374 |
| 需重跑的测试 | cargo test 全量；npm run test:tauri:smoke；改 OpenCode/ZCode 采集时跑对应 flow | 仓库 AGENTS.md 的窗口与 provider 改动规则 |
| 已跟踪文件里提到「性能面板」入口的样板点 | 2 处工具条（index.html:5432、5697） | grep performanceToggle |

## 怎么算做对

- 挑一个跨多天的 ZCode 会话：面板按天明细相加等于该会话累计值；与
  `model_usage` SQL 直查一致。
- 挑一个 Claude 大文件会话：日桶与独立脚本按事件时间戳分桶的结果一致。
- Copilot CLI 会话出现在排行里但全列"—"，不破版。
- 7/30 天切换、翻页、换分组、展开行、跳转在中文/英文、dark/light 下都过一遍。

## 未决问题

1. 排行默认排序：毛输入 token（计费口径，推荐）还是输出 token 或活跃时长。
   选项都做的话列头可点切换，代价是多一档交互。
2. OpenCode 按天明细的消息行数预算：本设计取 5000 行/会话，超限降级。
   数字可调，先按这个实现，实测后回来修订。
3. 面板入口按钮：与性能诊断按钮并排放在主窗口工具条（推荐）。若工具条空间紧张，
   备选是收进设置菜单，但入口变深。
