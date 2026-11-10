---
schema_version: "1"
protocol: "1.3.0"
id: "wi_2HMFVCQ5ZM2CWK3PTVSKX2R1H3"
short_id: "epqr579h"
title: "AgentWatcher 支持 WorkBuddy Provider"
status: "active"
created_at: "2026-10-29T07:24:17.764206100+00:00"
home_repository: "file://F:\\AiProject\\AgentWatcher"
kind: "feature"
branch_or_pr: "feature/AgentWatcher-支持-WorkBuddy-Provider"
---
## 目标

AgentWatcher 新增第 7 个会话 provider `workbuddy`（徽标 WB）：从 WorkBuddy 桌面端的本地数据
`%USERPROFILE%\.workbuddy\workbuddy.db`（SQLite，只读）与
`%USERPROFILE%\.workbuddy\projects\<工作区 slug>\<会话 id>.jsonl`（正文）扫描会话，
按 waiting / running / idle 状态机出卡片；悬浮预览显示工作区/标题/状态/最后用户输入/AI 正文；
用量卡片带上下文占用（session_usage.used/size）、credits 与 token 口径（正文 message.usage）。

范围与跳转能力（点击卡片、接续 handoff 作为来源或目标、状态机 waiting 的具体判定）在设计阶段
按 WorkBuddy 实际能力拍板后写进 design.md，验收标准以下面七条为准。

## 范围

- 只读扫描：SQLite 只读打开（mode=ro，兼容 WAL），JSONL 头尾采样，不拉起任何 WorkBuddy 进程。
- GUI 与 CLI 共用扫描入口（scan_sessions_blocking 复用），ScanOptions 增加 includeWorkbuddy（默认 true）。
- 会话 ID 采用 `workbuddy:<自然键>` 的确定性拼接，与现有 provider 口径一致。
- 桌面端未装的机器上零工作：目录不存在即静默返回空列表。
- 不发版、不改 Bridge、不改版本号。

## 强约束

- 遵守 AGENTS.md 插件宿主硬规则：本功能在宿主内实现通用 provider，不引入任何 WorkBuddy 专有插件。
- SQLite provider 只采样最近 N 行 message（与 OpenCode/ZCode 同口径），不读全文。
- 改 UI 卡片/筛选/设置开关时中英文、dark/light 双主题都要过 `npm run test:tauri:flow`。

## 验收条件

- AC-001: `cargo test --manifest-path src-tauri/Cargo.toml` 全绿，含新增 workbuddy 单元测试
  （include_workbuddy 默认开/可关、session 行映射、JSONL 摘要提取、状态机判定）。
- AC-002: 真实 Tauri 冒烟流程通过（新增或复用 `npm run test:tauri:flow` 流程）：主窗口出现
  WorkBuddy 会话卡片，徽标/工作区/标题/状态渲染正确。
- AC-003: WorkBuddy 会话可预览：悬浮预览窗显示工作区/标题/状态/最后用户输入/AI 正文且与数据源一致。
- AC-004: 设置面板「Data」组出现 WorkBuddy 会话开关（默认开），主面板 provider 筛选下拉含
  workbuddy；中英文、dark/light 双主题渲染正常。
- AC-005: 点击卡片行为与设计一致（可跳转则跳转，不可跳转则返回明确提示，不静默失败）。
- AC-006: 用量口径可用：卡片/预览带上下文占用与 credits/token 数据，数据源没有的项为 null。
- AC-007: AGENTS.md / README 口径同步（数据源路径、ScanOptions 字段、状态机判定、测试要求），
  CLI 契约测试（cli_taxonomy / cli_contract）如涉及则同步更新并通过。
