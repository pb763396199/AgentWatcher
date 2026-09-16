---
schema_version: "1"
protocol: "1.3.0"
id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
short_id: "6bty2dav"
title: "AgentWatcher AI 操作 CLI"
status: "done"
kind: "feature"
created_at: "2026-09-16T02:51:20.805378100+00:00"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
branch_or_pr: "feature/agent-cli"
---
# AgentWatcher AI 操作 CLI

## 原始请求

用户原话（2026-09-16）：

> 参考F:\AiProject\UnrealDevFlow的cli架构，为agentwatcher设计一套可供ai操作的cli，需要interview和头脑风暴来讨论需要哪些大类的cli

前置上下文：同日用户先问「AgentWatcher 是否提供了可供 AI 使用的 CLI 或 skill，能查询扫出来的任何 session 的内容和信息」。核实结论是没有任何对外接口——全部能力封闭在 Tauri 桌面进程的 19 个 `#[tauri::command]` 后面，无 CLI 二进制、无 headless 模式、无本地查询服务。用户随后发起本任务，指定参考 UnrealDevFlow `udf` CLI 的架构设计 AgentWatcher 自己的 AI 操作 CLI。

## 目标

AI agent 通过命令行直接获得 AgentWatcher 的会话查询能力：不启动 GUI 桌面进程，就能查询被监控的全部六个 provider（Copilot / Copilot CLI / Claude / Codex / OpenCode / ZCode）的会话列表、状态、摘要、用量，按需取会话正文，并导出接续用的源会话上下文。输出以机器可解析的 JSON 为一等公民，human 可读模式次要。

## 范围

- 新增独立 CLI 二进制 `agentwatcher-cli.exe`，随发布 zip 分发（发布合同变化：zip 内容新增一个文件）。
- 命令按两个大类组织（2026-09-16 访谈拍板）：session（会话查询：list / show / usage）与 handoff（接续导出：export）。
- skill 分发（2026-09-16 补充拍板，并入本任务）：`skill install / list / remove` 三个动词，把内嵌在二进制里的 AgentWatcher 用法 SKILL.md 装进本机可探测的 AI 宿主技能目录（`.zcode` / `.claude` / `.agents` / `.codex` / `.config/opencode`，只装目录已存在的），支持 `--dir` 显式指定；安装幂等覆盖，作为后续版本升级通道。
- 输出契约参考 UnrealDevFlow `udf` CLI：全局 `--format json|human`，JSON 模式只输出一个统一信封文档。
- 复用 `src-tauri/src/lib.rs` 既有的扫描、状态机、摘要提取逻辑，不重复实现解析。
- 访谈确认的非目标：不做 handoff 派发（拉起目标客户端，留后续版本）、不做运维工具（bridge / 性能 / doctor）、不做插件操作、不做连接运行中 GUI 的 IPC。

## 强约束

- 插件宿主硬规则全部适用：CLI 不得成为绕过 `AgentWatcherPluginStdio/1` 受控协议调用插件的通道，不得硬编码插件可执行文件路径。
- 隐私边界（访谈拍板）：list 类命令默认只输出元数据；会话正文只在显式子命令或开关下输出。信任模型：本机 AI 进程可信——AI 本来就能直读原始 JSONL / SQLite，CLI 不扩大边际暴露面。
- 不能随便改的发布合同标识符（Bridge ID、App 标识符等）不动。

## 验收条件

- AC-001: `cargo build` 产出 `agentwatcher-cli.exe`，零插件、GUI 未运行的状态下 CLI 可完整完成全部查询命令。
- AC-002: 命令按 session / handoff / skill 三个大类组织，大类与动词词表、帮助文本有一致性测试锁定。
- AC-003: `--format json` 输出统一信封（字段名、camelCase、退出码语义）有测试逐字段断言；human 模式的输出不混入 JSON 模式。
- AC-004: 列表类命令默认输出不含会话正文，正文仅在显式请求时输出，有测试断言。
- AC-005: `handoff export` 落盘位置与文件名规则和 GUI 接续面板一致（`%TEMP%\AgentWatcher\handoff-sources\`，文件名 = 会话 ID）。
- AC-006: Rust 单元测试与 CLI 集成测试全绿。
- AC-007: AGENTS.md / README 口径同步（CLI 命令面、隐私边界、发布 zip 内容变化）。
- AC-008: `skill install --dir <目录>` 创建 `<目录>\agentwatcher\SKILL.md` 且带 name/description 头；`skill list` 报告各宿主候选目录的安装状态（installed / outdated / not-installed / host-absent）；`skill remove` 幂等；重复 install 覆盖升级；自动化测试只经 `--dir` 打临时目录，不触碰真实宿主目录。
