---
schema_version: "1"
protocol: "1.3.0"
title: "CLI 裸自然键跨 provider 兜底查找"
status: "active"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
kind: "fix"
id: "wi_2QQA7FZVFDDFBHS9PPRMPZQR8S"
short_id: "gs323q4v"
created_at: "2026-11-24T09:08:01.069532100+00:00"
branch_or_pr: "feature/CLI-裸自然键跨-provider-兜底查找"
revision: 1
updated: "2026-11-24"
---
# CLI 裸自然键跨 provider 兜底查找

## 目标

用户拍板（2026-11-24）：裸 UUID（及其他不带 provider 前缀的自然键）也要能查。
现状是裸 UUID 一律按 Codex 解释，Codex 没有就直接 session_not_found——Antigravity
等新 provider 的会话用裸键完全查不到，明明白是代码能解决的事。

## 范围

- `agentwatcher-cli` 的 `find_session` 链路末尾加自然键兜底：前缀精确查、Codex
  归一化查、全历史定向查都落空后，全 provider 宽扫按自然键（id 去掉
  `provider:` 前缀的部分）匹配，取排序后的第一个命中；返回会话的 id 字段
  带真实 provider 前缀，不冒充用户原始输入的 provider。
- lib.rs 新增 pub 入口 `lookup_session_by_natural_key`（CLI 复用，GUI 不经过）。
- session_not_found 文案同步更新（提示前缀形态与兜底行为）。
- 不改 GUI、不改信封契约、不发版（随下一次发布走）。

## 强约束

- 兜底只在原有链路全部落空后触发；显式前缀命中的行为不变。
- 多 provider 撞同一自然键时取确定性首个命中（同一输入稳定同一结果），不报歧义错误。

## 验收条件

- AC-001: 真机上 `session show ffc7544b-6fa2-4568-8893-ddc08c694172`（裸 UUID，
  Codex 无此会话、Antigravity 有）命中并返回 `antigravity:ffc7544b-…` 的会话。
  Verify: manual:真机跑 agentwatcher-cli session show ffc7544b-6fa2-4568-8893-ddc08c694172，ok:true 且 id 为 antigravity 前缀
- AC-002: 带 provider 前缀且命中的查询行为不变；不存在的键仍报 session_not_found
  且退出码 1，文案含前缀提示。
  Verify: case:test_normalize_lookup_id_passthrough_non_codex 与真机 not_found 复验
- AC-003: `cargo test --manifest-path src-tauri/Cargo.toml --test cli_taxonomy --test cli_contract` 全绿。
  Verify: manual:在本机跑两组 CLI 测试全部通过
- AC-004: AGENTS.md 的「裸 UUID 一律按 Codex 解释」口径与 CHANGELOG 未发布条目同步。
  Verify: manual:grep 复核两处文档与实现一致

## 设计取舍

- 候选 A（采用）：链路末尾兜底。原语义全部保留，只在落空时多走一步；实现小、
  风险面小。多 provider 撞键取首个——裸 UUID 场景撞键概率近零，短键场景靠
  返回 id 的真实前缀自证。
- 候选 B（否）：入口处把裸键改成语义化多候选提示（报歧义让用户挑）。CLI 是给
  agent 用的，交互式挑选项不存在，多一轮往返不如直接给确定性结果。
- 候选 C（否）：把裸 UUID 的首选解释从 Codex 改成全 provider 并行。改变既有
  Codex 优先语义，回归面大。
