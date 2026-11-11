---
schema_version: "1"
protocol: "1.3.0"
id: "wi_7FY75QZZZZ3Z0YTX8PZVK8N0BN"
short_id: "4hmwwkw0"
title: "cli会话命令接受codex深链与裸uuid"
status: "done"
kind: "fix"
created_at: "2026-11-11T07:15:43.148758900+00:00"
home_repository: "file://F:\\AiProject\\AgentWatcher"
branch_or_pr: "feature/cli会话命令接受codex深链与裸uuid"
---
# cli会话命令接受codex深链与裸uuid

## 原始请求

用户原话（2026-11-11）："sess_44b48522-25d8-445c-892b-e6f2438da375 查清楚为什么这个会话报深链接找不到，折腾一段时间后才成功，是我们工具的问题吗"，看完调查结论后："修"。

## 目标

`agentwatcher-cli` 的按 ID 查询命令（`session show` / `session usage` / `handoff export`）对 Codex 会话接受 `codex://threads/<uuid>` 深链与裸 UUID 形态，与 suggest 命令同一套口径，不再误报 `session_not_found`。

## Why

现状三条命令只认 `session list` 给出的 `codex:<uuid>` 形态（`commands.rs` 的 `find_session` 拿原始串对 `session.id` 精确匹配，全历史兜底 `lookup_session_full_history` 把 `codex://threads/…` 剥成 `//threads/…` 后更匹配不上）。ZCode 会话 `sess_44b48522-…` 里的 agent 拿 Codex 桌面端的深链形态查询被拒，多绕两轮 `session list` 才自己发现要换形态。suggest 链路经 `codex_session_key` 已接受三种形态，同一 CLI 两套口径是真实的可用性坑。

## 范围

做：

- CLI 命令入口统一归一化：`session show` / `session usage` / `handoff export` 的 ID 参数先过 Codex 形态归一化（复用 lib 的 `codex_session_key`），`codex://threads/<uuid>` 与裸 UUID 收敛为 `codex:<uuid>`，`find_session` 与 `session_usage_detail` 两条解析路都拿到规范形态。
- `codex_session_key` 从私有改 `pub` 供 CLI 复用，逻辑不动。
- CLI crate 单元测试覆盖归一化 helper（深链、裸 UUID、其他 provider 原样通过）；契约测试补"不存在 UUID 的深链/裸形态仍报 session_not_found、退出码 1"的断言。
- 同步口径文档：`AGENTS.md` 的 CLI 一节、内嵌技能文档 `src-tauri/src/bin/agentwatcher-cli/skill/SKILL.md`（`version:` 2→3）。

不做：

- 不改 GUI 的 `open_session`（ZCode 跳转不实现是另一个拍板）。
- 不动 suggest 链路（已接受三形态）。
- 不为其他 provider 引入深链/裸键形态（各家的自然键无统一深链标准）。
- 不改 `session list`（无 ID 参数）与全历史扫描缓存。

## 强约束

- 归一化只认 UUID 形状（8-4-4-4-12 十六进制），其他 provider 的 `provider:key` 原样精确匹配，现有行为不变。
- 信封契约不变：json 模式一条命令一个信封、五键齐全；`session_not_found` 退出码仍为 1。
- 内容分级不变：`session list` 无正文键的契约断言不动。
- 裸 UUID 一律按 Codex 解释（与 suggest 现口径一致），不做多 provider 猜测。

## 自主边界

| 不用问直接定 | 必须停下来问 |
| --- | --- |
| 归一化放 CLI 三个命令入口共用的 helper，不改 lib 的全历史兜底 | 无。修法方向在调查结论里已给出，用户拍板「修」 |
| 单元测试放 CLI crate（`commands.rs` 内 `#[cfg(test)]`） | |
| SKILL.md `version:` 2→3 | |

## 验收条件

- AC-001: `session show`、`session usage`、`handoff export` 传 `codex://threads/<uuid>` 深链形态时，命中与 `codex:<uuid>` 相同的会话。Verify: case:test_normalize_lookup_id_accepts_codex_forms
- AC-002: 三个命令传裸 UUID 形态时同样命中 `codex:<uuid>` 的会话。Verify: case:test_normalize_lookup_id_accepts_bare_uuid
- AC-003: 非 Codex 形态（`zcode:`、`claude:` 等非 UUID 键与带其他前缀的串）不做归一化，原样精确匹配。Verify: case:test_normalize_lookup_id_passthrough_non_codex
- AC-004: 不存在的会话 ID（含深链与裸 UUID 形态）仍报 `session_not_found`、信封五键、退出码 1。Verify: case:not_found_deep_link_and_bare_uuid_still_fail_cleanly
- AC-005: 全量 Rust 测试绿：`cargo test --manifest-path src-tauri/Cargo.toml` 含 `cli_taxonomy`、`cli_contract`。Verify: manual:在本机跑 cargo test --manifest-path src-tauri/Cargo.toml，全部通过（含 cli_taxonomy 与 cli_contract 两个集成测试）

## 访谈记录

| 时间 | 谁说的 | 内容 | 结论 |
| --- | --- | --- | --- |
| 2026-11-11 | 用户 | "查清楚为什么这个会话报深链接找不到，折腾一段时间后才成功，是我们工具的问题吗"，随后 "修" | 调查结论（契约上不算 bug、体验上是 CLI ID 容错不一致）与修法方向（入口归一化 + 补契约测试）当面给出，用户拍板执行 |

## 设计取舍

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 归一化放 CLI 命令入口 | 三个命令入口各调一次 normalize helper，`find_session` 与 `session_usage_detail` 都拿到规范形态 | 三处调用点；lib 需把已有函数改 `pub` | 采用 |
| 归一化塞进 lib 的 `lookup_session_full_history` | 改 lib 兜底一处生效 | GUI 的 `open_session` 走另一套请求结构（provider 字段独立传），为 CLI 改 lib 兜底影响面大于收益 | 不采用 |
| 只改报错文案提示正确形态 | 不改行为，报错里写"请用 codex:<uuid>" | 调用方仍要多跑一轮命令换形态，深链明明是可无损换算的 | 不采用（用户已拍板直接修行为） |
| 只收深链、拒绝裸 UUID | 规避跨 provider 歧义 | suggest 已收裸 UUID，同一 CLI 两套口径的问题只解决一半；各 provider 的 UUID 空间独立，误命中风险可忽略 | 不采用 |

## revision 历史

| 版本 | 日期 | 为什么改 |
| --- | --- | --- |
| 1 | 2026-11-11 | 首版 |
