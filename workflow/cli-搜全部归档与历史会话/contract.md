---
schema_version: "1"
protocol: "1.3.0"
id: "wi_01M5C61SH5000FME6B0YSBBTDE"
short_id: "d53cbzmx"
title: "cli 搜全部归档与历史会话"
status: "in_review"
kind: "feature"
created_at: "2026-10-20T08:15:24Z"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
branch_or_pr: "feature/cli-搜全部归档与历史会话"
revision: "1"
updated: "2026-10-20"
---

# cli 搜全部归档与历史会话

## 原始请求

用户原话（2026-10-20）："agentwatcher技能现在到底能不能搜到归档后的所有会话，如果不能，我需要让它能，但并不需要让agentwatcher的软件里可以看到"

## 目标

agentwatcher-cli 用显式参数（`--active-days 0 --include-archived --limit 20000`）就能搜到全部归档会话与任意久远的历史会话，`session show` / `session usage` / `handoff export` 对这些会话同样可用；AgentWatcher GUI 的默认行为不变。

## Why

改动前的实证（2026-10-20 本机数据）：四个独立的阻断点叠加，归档会话与老会话对 CLI 完全不可见。

1. `hide_archived` 默认 `true`，CLI 没有任何参数能关掉它。
2. `active_window_days` 被硬夹在 1–30 天，`is_active_session` 在所有 provider 的扫描路径里都是硬过滤：ZCode 库 291 个会话只可见 57 个，Claude 磁盘 543 个文件只可见 161 个。
3. OpenCode/ZCode 的 SQL 行扫描有 240 行硬上限（`OPENCODE/ZCODE_SESSION_SCAN_LIMIT_MAX`），OpenCode 库 15,872 个会话再怎么调 `--limit` 也看不见更多。
4. `session show/usage/handoff export` 的 `find_session` 走默认参数扫描，归档或窗口外的 ID 一律 `session_not_found`。
5. Codex 归档目录 1,385 个文件里 1,213 个是 `.jsonl.zst`，扫描器只认 `jsonl` 扩展名且无解压依赖。

## 范围

做：

- `resolve_scan_options`：`active_window_days=0` 显式进入全历史模式（`active_window_ms = u64::MAX`）；全历史下 `max_sessions` 上限放宽到 20,000、SQLite 扫描上限解除到 `max_sessions×3`。
- CLI `session list` 加 `--include-archived`；`find_session` 两段式（默认参数 → `lookup_session_full_history` 定向兜底）。
- Codex `.jsonl.zst` 解压（ruzstd 纯 Rust 解码器）：列表走头部流式解压，定向查找全量解压；`show --content` / `handoff export` 对 zst 在临时目录写解压 JSONL 副本。
- 全历史列表用一条批量 SQL（`json_each` + `group by`）补齐 messageCount。
- 更新 `cli_taxonomy` / `cli_contract` 相关断言、`skill/SKILL.md`、`AGENTS.md`。

不做：

- GUI 任何可见行为变化（不加设置项、不改默认列表、不展示归档会话）。
- 会话正文关键词搜索——只保证会话可被发现、可看详情、可导出。
- Codex 归档 usage 聚合（rollout 片段链索引仍只覆盖 `~/.codex/sessions`）；老 rollout 采样段里没有 token 事件时 usage 为 null。

## 强约束

- GUI 调用路径语义不变：默认参数、非 0 值的 1–30 天夹取、GUI 场景的 240 行扫描上限照旧（机器锚点 `test_resolve_scan_options_defaults_preserved`）。
- CLI 输出契约不变：信封五键、退出码、`session list` 不含对话正文。
- 隐私分级不变：list 只有卡片级元数据。

## 自主边界

| 不用问直接定 | 必须停下来问 |
| --- | --- |
| 全历史模式的表达方式（0 = 不限）、上限取值（20,000） | 是否引入新的第三方依赖 |
| zst 两档读取深度（列表头部流式 / 定向全量） | |
| messageCount 批量补齐的实现 | |
| zst 解压副本落盘位置（临时目录 AgentWatcher 命名空间） | |

按用户全局指令（自主执行、提问协议随最终报告交付），ruzstd 依赖的取舍未停下来问，判断依据与可推翻问题见任务最终报告。

## 验收条件

- AC-001: `session list --include-archived` 能列出归档会话。Verify: manual:在本机跑 agentwatcher-cli session list --provider opencode --include-archived --active-days 0 --limit 20000，对照 SQLite 里 time_archived 非空的会话数
- AC-002: `session list --active-days 0` 能列出 30 天窗口之外的会话，`--limit` 足够大时各数据源全量可见。Verify: manual:在本机跑 session list --active-days 0 --include-archived --limit 20000，对照各数据源总量（扣除 subagent 与临时目录工作区）
- AC-003: `session show` / `session usage` / `handoff export` 对归档或窗口外会话 ID 不再报 `session_not_found`。Verify: manual:取一个 30 天前的会话 ID 依次跑三条命令，全部成功
- AC-004: Codex `archived_sessions` 下 `.jsonl.zst` 与纯 `.jsonl` 会话都能被列出，`show --content` 给出可读路径。Verify: manual:对归档目录里的 zst 会话跑 session show --content，确认 rawSessionPath 指向可读 JSONL
- AC-005: 不带新参数的 CLI 调用与 GUI 扫描的默认语义、扫描上限与改动前一致。Verify: case:test_resolve_scan_options_defaults_preserved

## 访谈记录

| 时间 | 谁说的 | 内容 | 结论 |
| --- | --- | --- | --- |
| 2026-10-20 | 用户 | "agentwatcher技能现在到底能不能搜到归档后的所有会话，如果不能，我需要让它能，但并不需要让agentwatcher的软件里可以看到" | 能力做进 CLI，GUI 不动 |

## 设计取舍

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 0 = 不限时间（采用） | `active_window_days` 显式传 0 时 `active_window_ms = u64::MAX`，非 0 值维持 1–30 夹取 | 语义上多一个特殊值，要在文档写清 | 采用：ScanOptions 不加字段，GUI 滑条最低 1 天永远传不了 0，默认路径逐字段不变 |
| 放宽 1–30 夹取上限到 3650 | 改 clamp 常量 | 改了 GUI 文档化的合同口径；3650 天仍不是"全部" | 不采用 |
| ScanOptions 加 `includeAllHistory: bool` | 新字段新语义 | 合同面变大，GUI 前端 serde 也要跟着动 | 不采用 |
| SQLite 扫描上限只在全历史解除（采用） | `is_full_history` 时上限变 `max_sessions×3`，否则维持 240 | 多一个模式分支 | 采用：240 上限是 GUI 响应速度调的，全历史是 CLI 显式参数，按模式分开 |
| 一律解除 240 | 上限直接跟 `max_sessions` | GUI 传大 `maxSessions` 时扫描变慢，违反 GUI 不变约束 | 不采用 |
| messageCount 批量补齐（采用） | 一条 `json_each` + `group by` 聚合（实测 15,872 会话 66ms） | 多一条 SQL | 采用：逐会话摘要实测约 8.8ms/个，全库 140s 不可行 |
| zst 列表只解头部（采用） | 流式解压解够 128KB 即停，尾部留给定向查找 | 全历史列表里 zst 会话的 lastAi/预览文本与精确 messageCount 缺失 | 采用：4.6GB 归档目录全解压要分钟级；list 契约本来就不含正文 |
| zst 列表也全解压 | 每个文件全流解压 | 实测推算 40–75s 起步，列表不可用 | 不采用 |
| ruzstd 解码器（采用） | 纯 Rust、无 C 工具链要求，decode-only 符合只读场景 | 新增依赖（含 twox-hash 传递依赖） | 采用：zstd 官方 crate 是 C 绑定，Windows 构建链更重 |
| zstd 官方 crate（C 绑定） | libzstd 绑定 | 需要 C 编译工具链参与构建，产物更大 | 不采用 |
| show --content 对 zst 给原始路径 | 不做解压副本 | 调用方拿到压缩档路径读不了，"可用路径"不成立 | 不采用 |
| zst 解压副本在 CLI 层写（采用） | `export_codex_zst_transcript` 写 `<tmp>\AgentWatcher\handoff-sources\codex\`，lib 的 handoff 上下文不动 | show --content 从纯只读变为可能写盘（SKILL.md 已同步声明） | 采用：在 lib 层做会改变 GUI 接续面板引用的路径，违反 GUI 不变约束 |
| 在 lib 层解压替换 handoff 路径 | prepare_handoff 直接返回解压路径 | GUI 行为随之改变 | 不采用 |

## revision 历史

| 版本 | 日期 | 为什么改 |
| --- | --- | --- |
| 1 | 2026-10-20 | 首版 |
