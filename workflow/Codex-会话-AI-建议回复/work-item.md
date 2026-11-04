---
schema_version: "1"
protocol: "1.3.0"
id: "wi_01M6JVHHKF000D34EX7V9M74VE"
short_id: "598bcdg3"
title: "Codex 会话 AI 建议回复"
status: "active"
kind: "feature"
created_at: "2026-11-04T09:00:00.000000000+00:00"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
branch_or_pr: "feature/Codex-会话-AI-建议回复"
---
# Codex 会话 AI 建议回复

## 原始请求

用户 2026-11-04 拍板："先保 CLI 通道，该提交的提交。然后直接去做 codex 的"。ZCode 建议功能已合入 dev（任务 38rcwjdf），复用同一套架构给 Codex 会话。

## 目标

Codex 会话的悬浮预览窗显示 AI 建议回复输入框，与 ZCode 建议功能同构：悬停出预览窗、输入框常驻、AI 建议可生成并填入、点发送经 Codex CLI 送回原会话。

## 范围

做：

- Codex 会话的预览窗回复输入框（复用 ZCode 的 UI 组件，按 provider 切换显示）
- Codex 发送通道：`codex exec resume <session> "text" --json`（CLI 公开可用）
- Codex 建议：扫描后对新进入 waiting 的 Codex 会话自动生成（v5 改造提示词，上下文从 JSONL 尾部提取）
- AGENTS.md 补 Codex 建议口径

不做：

- 不改 ZCode 建议功能（已合入）
- 不做 thread/sample 官方预测引擎（Codex app-server 的 thread/sample 可作为后续优化）
- 不做自动作答（Codex 的 AskUserQuestion 检测已有，护栏逻辑复用）

## 强约束

- 预览窗 UI 按 provider 路由：ZCode 卡片显示 ZCode 建议，Codex 卡片显示 Codex 建议
- 发送走 `codex exec resume`（公开 CLI，无授权墙）
- 全量测试不回退

## 验收条件

- AC-001: Codex 会话悬停预览窗出现回复输入框，打字+发送可用。
  Verify: manual:Codex 会话悬停预览窗测试
- AC-002: Codex 会话 waiting 时自动生成建议并填入输入框。
  Verify: manual:Codex waiting 会话建议测试
- AC-003: 发送后消息落库，模型回复出现。
  Verify: manual:对真实 Codex 会话完成一次往返
- AC-004: 全量测试绿。
  Verify: case:cargo_test_full_suite
