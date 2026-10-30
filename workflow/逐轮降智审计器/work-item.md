---
schema_version: "1"
protocol: "1.3.0"
id: "wi_5VYQZTXYZD0H5M6CN9TKTRFJC3"
short_id: "fnv6stb3"
title: "逐轮降智审计器"
status: "in_review"
created_at: "2026-10-29T08:02:42.284163900+00:00"
home_repository: "file://F:\\AiProject\\AgentWatcher"
kind: "feature"
branch_or_pr: "feature/逐轮降智审计器"
---
# 逐轮降智审计器：给 AgentWatcher 的会话加上逐轮模型与用量审计

## 原始请求

用户原话：「按"逐轮降智审计器"的思路设计，是否每个agent都支持，给出完整设计文档」。
背景：前一轮调查确认了 ChatGPT 桌面端的逐轮降级信号（每轮真实模型、推理档、token 用量、模型被重路由与安全缓冲改写的痕迹），用户要把它做成 AgentWatcher 的能力。

## 目标

每个被监控 agent 的会话，每一轮都留下「当时到底是谁在回答、什么档位、花了多少 token」的记录，让模型质量变化可以从数据上核对，而不是靠体感。

## 范围

- 做什么：六个 provider（Copilot Chat、Copilot CLI、Claude Code、Codex、OpenCode、ZCode）逐轮审计信号的提取、存储、展示与导出；先出设计文档（含每个 provider 的支持度结论与实测证据），设计认可后再实现。
- 不做什么：不抓取任何 provider 服务端的原始思维链原文（前一轮已确认拿不到）；不做自动发送或干预；不新增对外网络请求。

## 强约束

- 遵守 AgentWatcher 插件宿主硬规则与隐私约束（审计数据不经 localStorage 存正文，事件通道传递）。
- 数据全部来自本机已有的会话落盘文件与数据库，只读打开。
- CLI（agentwatcher-cli）若暴露审计数据，必须走既有信封契约与内容分级。

## 验收条件

- AC-001: 设计文档给出六个 provider 的逐轮可审计信号矩阵（支持/部分/不支持），每条结论附本机实测证据（文件或数据库、字段名）。
  Verify: manual:对照设计文档「统一结构」表与调查记录逐源核对字段名和证据位置
- AC-002: 对判定为支持的 provider，实现逐轮提取真实模型标识、推理档（数据源有时）、token 用量、轮次时间，输出结构稳定、跨次调用一致。
  Verify: case:turn_audit_groups_calls_into_turn_windows
- AC-003: 逐轮提取在真实本机会话数据上验证：Claude、Codex、OpenCode、ZCode、Copilot Chat 至少四个 provider 通过，Copilot CLI 若无数据源则明确记录「不支持」的原因。
  Verify: case:zcode_session_timeline_reads_model_usage_rows_and_stamps
- AC-004: 审计结果在 UI 可见（会话卡片或详情），中文/英文、dark/light 都正常；正文仍不写 localStorage。
  Verify: manual:悬停一张卡片看预览摘要行，打开会话分析看逐轮审计表，中英文与深浅色各过一遍
- AC-005: 降智判定可解释：模型变化、档位变化、用量异常、重路由或安全缓冲标记四类信号各自单独成列，不合成单一分数。
  Verify: case:turn_audit_flags_reliability_signals
- AC-006: cargo test 全部通过；改到 UI 时对应 npm run test:tauri:flow 通过。
  Verify: manual:跑 cargo test --manifest-path src-tauri/Cargo.toml 与 npm run test:tauri:smoke，全绿
