---
title: VS Code Bridge 会话重开重试修复复盘
purpose: retrospective
status: done
date: 2026-06-15
language: zh-CN
evidence_refs:
  - F:/AiProject/AgentWatcher/src-tauri/src/lib.rs
  - F:/AiProject/AgentWatcher/.tmp/tauri-realtest/20260612-113344/result.json
  - F:/AiProject/AgentWatcher/.tmp/tauri-realtest/20260613-104048/result.json
---

# VS Code Bridge 会话重开重试修复复盘

## summary

本次修复针对 session 卡片通过 VS Code Bridge 打开会话时的时序问题：VS Code 需要先聚焦目标 workspace，Bridge URI 太早触发时可能找不到刚切过去的目标上下文，导致用户点击卡片后没有稳定落到对应 session。

任务闭环状态：已完成。代码层面只改 `src-tauri/src/lib.rs` 的 `open_session()` 与 `open_session_with_bridge()` 路径，不改 session 扫描、状态机或 provider 数据结构。

## changes

- 在 Bridge open 成功路径上保存 direct session deep link，并在 Bridge URI 之后延迟触发一次 direct deep link fallback。
- `open_session_with_bridge()` 从单次延迟触发 Bridge URI 改为带常量控制的重复触发，让 VS Code 聚焦 workspace 后仍有机会接收 Bridge open。
- 当 Bridge 路径失败且没有 session deep link 时，仍按原策略退回 `open_agents_page()`，错误信息保留 Bridge 失败与 fallback 失败两段上下文。

## validation

- 该提交自身没有单独新增专项自动化；原提交记录明确写明由后续 OpenCode/Tauri 回归覆盖。
- 后续真实 Tauri 验证中，OpenCode 接续上下文流程通过：`.tmp/tauri-realtest/20260612-113344/result.json`。
- 后续真实 Tauri smoke 通过，覆盖主窗口、性能诊断、接续面板识别和 AgentTask 切换：`.tmp/tauri-realtest/20260613-104048/result.json`。

## blockers

无阻塞项。主要证据缺口是没有为“VS Code 聚焦后重试 Bridge open”单独增加可复现的专项测试；该行为依赖外部 VS Code 激活时序，当前以真实 Tauri 回归和后续 OpenCode 打开/接续流程作为间接覆盖。

## required_follow_ups

- 后续如果测试 harness 能控制 VS Code 或 Bridge ack，应增加一个专门模拟“workspace 先聚焦、Bridge 后到达”的回归。
- URI 重试次数和延迟目前是保守固定值；如果用户反馈打开变慢或重复聚焦，应再用真实机器数据调整常量。

## 反思

这类问题的本质不是“链接打不开”，而是跨进程协议启动的时序竞争。最初如果只把失败归因成 Bridge extension 或 deep link 格式错误，很容易误改协议本身；实际代码显示，session resource 和 Bridge link 都能生成，风险点在 VS Code 先打开 workspace 后，Bridge command 需要等目标窗口可接收。

本次处理选择了最小干预：不改 session 解析，不改 provider 状态，不扩展后端 command schema，只在打开链路上补延迟重试和 direct deep link 兜底。这个方向是对的，因为用户要的是卡片点击在慢启动 VS Code 上更稳，而不是重新设计 Bridge。

不足也很明确：验证主要依赖后续集成回归，缺少针对这一个时序问题的单点测试。以后凡是涉及外部应用激活、URI protocol、Bridge ack 的问题，都应把“时序条件”写进测试设计，而不是只验证最后窗口存在。

## doc_contract_status

pass

## artifact_purpose

retrospective

## artifact_language

zh-CN

## canonical_path

docs/retrospectives/2026-06-15-vscode-bridge-reopen-retrospective.md

## legacy_violations

[]
