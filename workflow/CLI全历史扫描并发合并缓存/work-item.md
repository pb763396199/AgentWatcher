---
schema_version: "1"
protocol: "1.3.0"
title: "CLI全历史扫描并发合并缓存"
id: "wi_3NM7QSRDWZHE0QCAXWTDJ2VNPX"
short_id: "w39h002z"
status: "done"
kind: "feature"
created_at: "2026-11-06T01:58:31.578661400+00:00"
home_repository: "file://F:\\AiProject\\AgentWatcher"
branch_or_pr: "feature/CLI全历史扫描并发合并缓存"
---
# CLI全历史扫描并发合并缓存

## 原始请求

用户 2026-11-06："直接给完整方案干，做设计时用第一性原理好好反思自己的方案是否最简最优"。起因：会话 sess_d3a5e9ee（adt-scene-agent 仓库）派 4 个并行 subagent，每个都跑 `agentwatcher-cli session list --active-days 0 --include-archived --limit 20000`，同一份全历史扫描并发 4 份，叠加刚删除约 100GB 文件后的系统负载，机器卡死。

## 目标

短时间窗口内参数相同的 CLI 全历史扫描只真实执行一次，并发与紧随其后的相同调用共享这一份结果，磁盘 I/O 不随并发调用数成倍增长。

## 范围

做：`session list` 在全历史模式（`--active-days 0` 或 `--include-archived`）下的跨进程结果缓存与单飞（single-flight）合并；缓存失效、绕过开关与故障回退；AGENTS.md 的 CLI 一节口径同步。

不做：GUI 扫描路径（有自己的防抖与配额）；默认活跃窗口模式；`show`/`usage`/`handoff export` 的定向查找；不改三个 pub 扫描入口签名。

## 强约束

- CLI 输出契约不变：json 模式仍是一条信封、五键、退出码语义不变；契约测试「列表不得含 lastUserMessage/lastAiMessage」断言不动。
- 缓存文件只落在 `<临时目录>\AgentWatcher\` 下，总量有上限，进程退出不残留锁。
- 缓存对调用方透明：命中缓存与真实扫描的输出逐字段等价。

## 验收条件

- AC-001: 同时启动 N（≥3）个参数相同的全历史 `session list`，只发生一次真实扫描，全部进程成功退出且结果一致。
  Verify: case:test_concurrent_full_history_scan_merges
- AC-002: 缓存超过 TTL 后的调用重新扫描；`--no-cache` 时直接扫描且不读缓存。
  Verify: case:test_cache_ttl_expiry_and_no_cache_flag
- AC-003: 缓存文件损坏或格式版本不符时静默弃用并直接扫描，命令退出码 0。
  Verify: case:test_corrupt_cache_falls_back
- AC-004: 默认活跃窗口（不带全历史参数）的 `session list` 不走缓存；cli_taxonomy 与 cli_contract 测试全过。
  Verify: case:test_default_scan_uncached_and_contract_tests
