---
schema_version: 1
protocol: 1.3.0
id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
short_id: 4967qf3h
title: 会话卡片与悬浮预览展示用量指标
status: done
created_at: 2026-09-15T06:46:07Z
home_repository: https://github.com/pb763396199/AgentWatcher.git
kind: feature
branch_or_pr: feature/agent-usage-metrics
base_revision: bc915a208539f73abc4964919eabcfb69e66bbf6
---

# 会话卡片与悬浮预览展示用量指标

## 原始请求

用户原话（2026-09-15）：

> 以下对话在尝试分析 ZCode 某些对话的 token 消耗和工具调用数量等状态。我希望将这个可视化的能力在 Agent Watch 中也展现出来。具体形式可以在每一个 Session Card 的悬浮窗上（每一个provider都要支持），或者在 Session Card 上直接预览，展现它的 token 使用量、工具调用数、用户输入轮数、模型调用次数等你觉得非常重要的、值得展示的信息。先去研究一下到底应该展现哪些信息，以及以怎样的形式展现吧？

来源会话：Codex 桌面端 `01a09f4d-b2ce-78c0-85b3-9b1d57da2a27`（工作区 F:\AiProject\aes-workflow），该会话用本机原始文件核算过 ZCode 的 token 消耗与工具调用。

## 目标

用户不打开各 AI 助手自己的界面，在 AgentWatcher 的卡片和悬浮预览里就能看清每个会话消耗了多少 token、调用了多少次工具和模型、进行了几轮对话。

## 范围

- 做：六个扫描源（Copilot Chat、Copilot CLI、Claude、Codex、OpenCode、ZCode）在扫描端采集用量指标；悬浮预览窗新增用量区块（全部扫描源统一布局）；卡片在密度允许时显示紧凑用量行；中英文、dark/light、横竖版同步；真实交互测试覆盖。
- 不做：不改各 AI 助手本身；不自建价目表估算成本（只显示数据源自带的成本字段）；不做趋势图和历史曲线；不做按工作区汇总的统计面板。

## 强约束

- 隐私边界不变：用量数字随既有 scan_sessions 返回值和预览事件走，正文仍然不落 localStorage。
- 扫描性能有界：前端默认 15 秒轮询加文件事件触发，JSONL 全量聚合必须带按文件（路径、大小、修改时间）的增量缓存，不能每次全量重读。
- 数据源没有的指标显示"—"，不显示 0（ZCode 订阅制下 cost 恒为 0，0 会误导）。
- 改卡片、预览、窗口交互必须跑 `npm run test:tauri:smoke`，改 OpenCode/ZCode 采集必须跑对应 `test:tauri:flow`。

## 验收条件

- AC-001: scan_sessions 返回的每个会话带用量字段（累计输入/输出 token、当前上下文、工具调用数、用户轮次、模型调用次数、模型名、时长），数据源没有的项为 null。Verify: case:test_agent_session_usage_fields_serialize
- AC-002: 悬浮预览窗显示用量区块，六个扫描源布局一致，缺数据显示"—"。Verify: manual:打开悬浮预览，对照六个 provider 的卡片各看一张
- AC-003: 卡片在非紧凑密度下显示一行紧凑用量，紧凑密度（compact-cards 及更小）下不显示且不破坏布局。Verify: manual:拖动卡片列数滑杆，观察卡片从宽到窄的变化
- AC-004: 用量文案中英文齐全，dark/light 两套主题下颜色和对比度正常。Verify: manual:切换语言和主题各看一遍卡片与预览
- AC-005: 聚合开销有增量缓存：同一文件未变化时第二次扫描不重读文件内容，scan 总耗时不高于现状加 200ms。Verify: case:test_jsonl_usage_cache_skips_unchanged_files
