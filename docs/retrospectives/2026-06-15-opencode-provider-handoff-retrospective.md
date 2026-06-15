---
title: OpenCode Provider 与可携带接续验证复盘
purpose: retrospective
status: done
date: 2026-06-15
language: zh-CN
evidence_refs:
  - F:/AiProject/AgentWatcher/src-tauri/src/lib.rs
  - F:/AiProject/AgentWatcher/ui/index.html
  - F:/AiProject/AgentWatcher/tools/tauri-realtest/cli.mjs
  - F:/AiProject/AgentWatcher/tools/tauri-dev.mjs
  - F:/AiProject/AgentWatcher/.tmp/tauri-realtest/20260612-105708/result.json
  - F:/AiProject/AgentWatcher/.tmp/tauri-realtest/20260612-113344/result.json
---

# OpenCode Provider 与可携带接续验证复盘

## summary

本次提交把 OpenCode 纳入 AgentWatcher 的正式 provider 体系：扫描本机 OpenCode SQLite 会话，渲染 OC badge 和状态卡片，支持设置、诊断、todo、接续目标，并为跨 provider handoff 导出可携带的来源 transcript 文件。

任务闭环状态：已完成。变更范围较大，覆盖 Rust 扫描/状态映射、前端 UI、真实 Tauri 测试 CLI、开发启动脚本和项目文档。OpenCode Desktop 没有稳定的外部 session deep link，因此卡片打开采用 web/server 路径，接续依赖导出的来源文件保证目标代理可读。

## changes

- 后端新增 OpenCode 数据源扫描与 `AgentSession` 映射，读取 SQLite 会话并转换为统一卡片数据。
- 前端新增 OpenCode provider 元信息、OC badge、筛选/设置/诊断展示和 handoff provider 文案。
- handoff 增加 OpenCode 来源导出文件，prompt 明确“来源会话 A 只是参考，新任务优先”，并列出目标代理读取来源文件的方式。
- AgentTask / todo / performance 相关路径同步识别 OpenCode，避免新 provider 只在主列表出现而在派发或诊断里缺席。
- `tools/tauri-dev.mjs` 与 `tools/tauri-realtest/cli.mjs` 扩展真实 Tauri 开发和验证能力，新增 OpenCode 卡片与接续上下文流程。

## validation

- 原提交记录的验证包括：`cargo test --manifest-path src-tauri/Cargo.toml`、`node --check tools\tauri-realtest\cli.mjs`、`node .tmp\bridge-handoff-routing-test.cjs`、`npm run build:ui`、`npm run test:tauri:flow -- opencode-handoff-context --attach --port=9223`、`npm run test:tauri:flow -- settings-performance --attach --port=9223`、`npm run test:tauri:smoke -- --attach --port=9223`、`git diff --check`。
- OpenCode 真实卡片流程通过，记录在 `.tmp/tauri-realtest/20260612-105708/result.json`，卡片 session 为 `opencode:ses_154681e80ffeVoVm9lOJTbiE52`，toast 为“已打开会话”，并标记 `OpenCodeWeb旁路=true`。
- OpenCode 接续上下文流程通过，记录在 `.tmp/tauri-realtest/20260612-113344/result.json`，验证 prompt 包含来源文件，且覆盖 `agents`、`code-chat`、`claude-panel`、`codex-app`、`opencode-cli` 五种目标模式。

## blockers

无阻塞项。主要外部限制是 OpenCode Desktop 当前不暴露“按 session id 打开已有会话”的原生 deep link；因此 AgentWatcher 只能打开项目/网页路径，不能保证外部应用直接选中某个 OpenCode session。

## required_follow_ups

- 如果 OpenCode Desktop 后续提供稳定 session deep link，应替换当前 web/server 旁路打开策略。
- SQLite schema 和 OpenCode 数据目录属于外部应用实现细节，后续升级 OpenCode 后需要回归扫描字段、状态推断和来源导出。
- OpenCode 作为 provider 已进入主路径，后续新增筛选、诊断或任务派发逻辑时必须把它纳入默认测试矩阵。

## 反思

这次提交的风险不是单个功能点，而是 provider 横切面很宽：扫描、状态、UI、打开、接续、todo、诊断、测试都要同时对齐。最容易犯的错是只让 OpenCode 卡片“显示出来”，但忘记它在 handoff 或 AgentTask 中没有可靠来源；本次用可携带来源文件解决跨 provider 可读性，是正确的边界处理。

另一个关键判断是没有假装 OpenCode Desktop 支持不存在的能力。卡片打开路径承认外部应用限制，用 web/server 旁路和明确风险说明兜底；handoff 则把 source transcript 导出成普通文件，让 Copilot、Claude、Codex、OpenCode CLI 都能通过文件读取上下文。这比把 provider 私有 UI 状态塞进 prompt 更稳，也更符合隐私边界。

不足在于提交体量偏大，多个横切模块同时落地，审查成本高。后续类似 provider 接入最好拆成“扫描模型”“主 UI 展示”“打开路径”“handoff 可携带来源”“真实 Tauri flow”几个可独立验证的提交，降低单次回归和复盘负担。

## doc_contract_status

pass

## artifact_purpose

retrospective

## artifact_language

zh-CN

## canonical_path

docs/retrospectives/2026-06-15-opencode-provider-handoff-retrospective.md

## legacy_violations

[]
