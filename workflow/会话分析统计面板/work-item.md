---
schema_version: "1"
protocol: "1.3.0"
id: "wi_3KXFZQMQ9AH2XQ8PGQBW299E2P"
short_id: "qk0rzhqz"
title: "会话分析统计面板"
status: "in_review"
created_at: "2026-10-14T02:59:52.232122400+00:00"
home_repository: "file://F:\\AiProject\\AgentWatcher"
kind: "feature"
branch_or_pr: "feature/会话分析统计面板"
---
# 会话分析统计面板

## 原始请求

用户原话（2026-10-14 第一轮）：

> 参考codex对它的会话的分析的设计，来优化agentwatcher对于任意会话的分析统计，先给出升级和可视化方案把，可用交互式UI来给出设计的mockup

用户原话（2026-10-14 第二轮，方向修正）：

> 你这个统计维度有问题啊，设计可以先保留，我要求的先去做每一个会话的全生命周期的分析，而不是把所有会话放到一个大统计表里，你目前的设计可以留着，但我要你先去细化每一个会话的。我提供的截图是t3 code它对于某一类模型的按时间轴的用量排序，我觉得这个形式你可以借鉴，用在单独一个会话的分析里。请重新给出原型

参考截图两张：Codex「分析」页（跨会话聚合形态，第一轮）；t3 code 的单模型用量详情（模型名加占比的头部、大数字指标行、按时间轴的面积图、按类型分段的底部条，第二轮，作为单会话分析的直接参照）。

## 目标

用户对任意一个会话点开就能看清它的完整生命周期：token 随时间怎么烧、上下文怎么涨、什么时候被压缩、模型和工具怎么分工——不用外部会话手工核算。

## 范围

分两阶段，单会话分析先行：

- 阶段 A（先行）：单会话全生命周期分析视图。从会话悬浮预览与卡片右键进入；头部身份区加指标行；按模型的用量时间轴（面积/柱状、粒度与模型过滤）；上下文占用曲线（含压缩回落）；按类型/按模型/按工具的结构分解；中英文、dark/light 同步；真实交互测试覆盖。
- 阶段 B（沿用本任务已归档的 design.md，实施在阶段 A 之后）：跨会话聚合面板（按天用量历史加会话排行）。
- 不做：不改各 AI 助手本身；不自建价目表估算成本（只显示数据源自带成本）；不做配额管理；Copilot CLI 数据源无用量字段，分析视图降级为轮次与时长。

## 强约束

- 隐私边界不变：分析数据只含用量数字与元数据，不含对话文本；正文不落 localStorage。
- 性能有界：时序聚合复用既有（路径、大小、修改时间）增量缓存或会话内 SQL，不引入全库扫描；单会话时序点数与 SQL 行数有预算，超限降级。
- 数据源没有的指标显示"—"，不显示 0（沿用上一任务口径）。
- ZCode 会话跳转维持既有拍板；分析视图内不引入跳转承诺之外的入口。
- 改窗口/交互必须跑 `npm run test:tauri:smoke`；改 provider 采集跑对应 `test:tauri:flow`。

## 验收条件

阶段 A（先行）：

- AC-001: 会话悬浮预览用量区块与卡片右键菜单提供「会话分析」入口，打开 analytics-panel 单会话模式；窗口样板与 capability 对齐既有面板。Verify: manual:从一个 ZCode 会话和一个 Claude 会话的预览与右键各进入一次
- AC-002: 头部显示会话身份（标题、提供商、工作区、模型、状态、跨度），指标行显示毛输入（含缓存占比）、输出、模型调用、工具调用、用户轮次、活跃时长、成本（数据源有才显示），缺项"—"。Verify: manual:对照六个 provider 各开一个会话
- AC-003: 用量时间轴按模型分色堆叠，粒度自动适配跨度（小时/6 小时/天）且可手动切换，模型可勾选过滤，悬停出当时间桶的各模型输入/输出/调用明细。Verify: manual:切粒度、勾掉一个模型、悬停一个高峰桶
- AC-004: 上下文占用曲线画出每次模型调用时的当前上下文；有窗口上限数据的源画上限参考线，压缩回落肉眼可辨。Verify: case:test_session_context_curve_points_monotonic_segments
- AC-005: 结构分解三组：按类型（毛输入/缓存读取/输出）、按模型、按工具 top，分段条加数值，各分解合计与总量一致。Verify: case:test_session_breakdown_sums_match_totals
- AC-006: 新增按会话的时序聚合命令，数据来自增量缓存或会话内 SQL，单会话时序点预算超限降级为累计口径；Copilot CLI 打开分析视图显示降级布局。Verify: case:test_session_timeline_budget_downgrades
- AC-007: 分析视图中英文齐全，dark/light 与主窗口同步，长标题/长工作区名/单点会话不破版；smoke 或专用 flow 覆盖打开与主要交互。Verify: manual:两种语言 × 两种主题过一遍并跑 npm run test:tauri:smoke
- AC-008: 轨迹泳道（2026-10-20 增补，设计 design/agent-trajectory-design-v2.md）：Codex 多智能体会话显示子代理泳道——默认只展开主代理，子代理组与开销排行可展开收起且无 DOM 残留；任一泳道点击展开合并明细（面积=毛输入、折线=上下文，高约泳道行高 4 倍内，细线不因拉伸变粗），绘图区与泳道像素对齐、时刻由共享轴与悬停读数承担；跨泳道模型切换参考线、编排调用标记与等待区间（估算标注）、并行度曲线、子代理效率列。子代理发现：thread_spawn 按 parent_thread_id 精确、guardian 按工作区+时间窗并标注估算。Verify: manual:用多智能体 Codex 会话（如 codex:01a15784…）打开分析，核対泳道数、合计与排行

阶段 B（已归档设计 design.md，实施顺序在后）：

- AC-101: analytics-panel 聚合模式与主窗口入口按钮可用。Verify: manual:从主窗口工具条打开
- AC-102: 汇总卡展示区间毛输入（含缓存占比）、输出、模型调用、活跃时长、涉及会话数。Verify: case:test_usage_analytics_summary_aggregates_providers
- AC-103: 按天堆叠柱状图支持 7/30 天、日期翻页、按提供商/模型分组，悬停出当日明细。Verify: manual:切 7/30 天、翻页、换分组
- AC-104: 会话排行按区间毛输入排序，行可展开模型/工具明细并触发既有跳转。Verify: manual:展开一个多模型会话并点击跳转
- AC-105: 聚合命令按天分桶来自增量缓存或会话内 SQL，不做全库扫描。Verify: case:test_usage_daily_buckets_reuse_incremental_cache
- AC-106: 聚合模式文案中英文齐全、双主题同步、空数据态不破版。Verify: manual:两种语言 × 两种主题
- AC-107: 真实交互测试覆盖聚合模式（smoke 或专用 flow）。Verify: manual:跑 npm run test:tauri:smoke 并核对面板截图
