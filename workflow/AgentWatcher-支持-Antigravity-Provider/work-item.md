---
schema_version: "1"
protocol: "1.3.0"
id: "wi_0ZQ8FFAX6K2B4D8F66T96TFDEM"
short_id: "v3s0ybtf"
title: "AgentWatcher 支持 Antigravity Provider"
status: "done"
created_at: "2026-11-23T03:09:31.730571100+00:00"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
kind: "feature"
branch_or_pr: "feature/AgentWatcher-支持-Antigravity-Provider"
---
# AgentWatcher 支持 Antigravity Provider

## 目标

AgentWatcher 新增第 8 个会话 provider `antigravity`：从 Google Antigravity 桌面端的本地数据
扫描会话，按 waiting / running / idle 状态机出卡片；悬浮预览显示工作区/标题/状态/最后用户输入/AI 正文。

本机已确认的数据布局（Windows）：

- 数据根 `%USERPROFILE%\.gemini\antigravity\`（Gemini 系数据目录，不是 `%APPDATA%\Antigravity`——那里只是 Electron 壳的浏览器 profile）。
- 会话清单：`conversation_summaries.db`（SQLite）的 `conversation_summaries` 表，字段含
  `conversation_id`、`title`、`preview`、`step_count`、`last_modified_time`、`workspace_uris`（JSON 数组，URI 编码的 file:// 路径）、
  `status`（观察到 `CASCADE_RUN_STATUS_RUNNING`）、`not_fully_idle`、`killed`、`last_user_input_time`、`project_id`、`app_data_dir`；
  另有 FTS5 表 `conversations_fts` 带按块索引的 `user_body`/`agent_body`。
- 会话正文：`conversations\<conversation_id>.db`（SQLite，一会话一库），`steps` 表的
  `step_payload`/`metadata` 是 protobuf 编码 BLOB（工具调用参数以 JSON 字符串内嵌），含 `step_type`/`status`/`permissions` 列；
  `gen_metadata` 表疑似模型与 token 用量。
- 活跃信号：`daemon\hub_*.json`（守护进程 pid/端口）、`presence\<conversation_id>.lock`。
- `antigravity://` URL 协议已注册（HKCR 指向 Antigravity.exe），会话级深链格式待查。

点击跳转、handoff 接续、waiting 的具体判定（steps 的 protobuf 结构、status 枚举全集）在
设计阶段按 Antigravity 实际能力拍板后写进 design.md，验收标准以下面八条为准。

## 范围

- 只读扫描：SQLite 只读打开（mode=ro，兼容 WAL），不拉起任何 Antigravity 进程（daemon/presence 只读文件不连端口）。
- GUI 与 CLI 共用扫描入口（scan_sessions_blocking 复用），ScanOptions 增加 includeAntigravity（默认 true）。
- 会话 ID 采用 `antigravity:<自然键>` 的确定性拼接，与现有 provider 口径一致。
- 接续来源照 ZCode 强口径：导出可读 Markdown 到 `handoff-sources\antigravity\`（正文库是
  protobuf 二进制，不能像 WorkBuddy 那样只给原始文件路径）。
- CLI `session show --content` 按「其余 provider」分级：最后输入输出节选加原始 db 路径。
- Antigravity 作为用法技能宿主进 `skill` 分发候选表（`~/.gemini/config/skills`，本机已有
  用户手动装的 agentwatcher 技能副本为证）。
- 桌面端未装的机器上零工作：数据根目录不存在即静默返回空列表。
- 不发版、不改 Bridge、不改版本号。

## 强约束

- 遵守 AGENTS.md 插件宿主硬规则：本功能在宿主内实现通用 provider，不引入任何 Antigravity 专有插件。
- protobuf 解析不得引入带 schema 生成代码的重依赖；用轻量 wire-format 解析或只用清单/FTS 层满足卡片与预览口径，取舍在设计阶段定。
- SQLite provider 只采样最近 N 行（与 OpenCode/ZCode/WorkBuddy 同口径），不读全文。
- 改 UI 卡片/筛选/设置开关时中英文、dark/light 双主题都要过 `npm run test:tauri:flow`。

## 验收条件

- AC-001: `cargo test --manifest-path src-tauri/Cargo.toml` 全绿，含新增 antigravity 单元测试
  Verify: manual:在本机跑 cargo test --manifest-path src-tauri/Cargo.toml，全部通过（含 15 个 antigravity 前缀单元测试）
  （include_antigravity 默认开/可关、conversation 行映射、workspace_uris 解码、状态机判定）。
- AC-002: 真实 Tauri 冒烟流程通过（新增 `npm run test:tauri:flow -- antigravity-session` 流程）：主窗口出现
  Verify: manual:跑 npm run test:tauri:flow -- antigravity-session，流程通过且 result.json 记录卡片/徽标/Toast
  Antigravity 会话卡片，徽标/工作区/标题/状态渲染正确。
- AC-003: Antigravity 会话可预览：悬浮预览窗显示工作区/标题/状态/最后用户输入/AI 正文且与数据源一致。
  Verify: manual:真机悬浮 Antigravity 会话卡片，预览窗显示工作区/标题/状态/最后用户输入/AI 正文且与桌面端一致
- AC-004: 设置面板「Data」组出现 Antigravity 会话开关（默认开），主面板 provider 筛选下拉含
  Verify: manual:中英文与 dark/light 四种组合下，设置面板 Data 组出现 Antigravity 开关（默认开）且筛选下拉含 Antigravity
  antigravity；中英文、dark/light 双主题渲染正常。
- AC-005: 点击卡片行为与设计一致（可跳转则跳转，不可跳转则返回明确提示，不静默失败）。
  Verify: manual:点击 Antigravity 卡片，toast 显示「Antigravity session jump is not supported yet…」，不静默失败
- AC-006: 用量口径可用：卡片/预览带 token/上下文占用等数据源有的项，没有的项为 null。
  Verify: case:antigravity_usage_counts_and_tokens
- AC-007: AGENTS.md / README 口径同步（数据源路径、ScanOptions 字段、状态机判定、测试要求），
  CLI 契约测试（cli_taxonomy / cli_contract）如涉及则同步更新并通过，含 skill 宿主表新增
  Verify: case:skill_list_without_dir_reports_host_candidates
  antigravity 后 `cli_contract` 宿主名单断言同步。
- AC-008: 接续面板可读取 Antigravity 源会话导出（`%TEMP%\AgentWatcher\handoff-sources\antigravity\`，
  文件名 = 卡片会话 ID，内容为解码后的用户/AI 正文 Markdown）；Antigravity 仅作为接续
  来源、不作为接续目标（与 AC-005 的不跳转一致）。

  Verify: case:antigravity_handoff_collect_and_render