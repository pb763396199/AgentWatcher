---
branch_or_pr: "feature/agentwatcher-zcode-provider"
created_at: "2026-09-14T04:17:26.441638100+00:00"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
id: "wi_3QQPFJCPXD24P1CCXMXAZ730TQ"
protocol: "1.3.0"
schema_version: "1"
short_id: "xf9ag8cp"
status: "active"
title: "AgentWatcher 支持 ZCode Provider"
---
## 目标

AgentWatcher 新增第 6 个会话 provider `zcode`（徽标 ZC）：从 ZCode 的本地会话库
`%USERPROFILE%\.zcode\cli\db\db.sqlite` 只读扫描会话，按 waiting / running / idle 三档
状态机展示卡片；点击卡片通过官方 CLI（node + zcode.cjs --resume）在新终端窗口恢复
该具体会话；接续（handoff）面板支持以 ZCode TUI 为目标派发。

范围限定：

- 只读扫描 db.sqlite，不 spawn 常驻进程，不接入 ZCode Protocol / 远控 relay。
- 点击跳转 = TUI 会话级恢复；桌面工作区级跳转不做（ZCode 当前无会话深链，
  官方反馈 zai-org/feedback#465 待响应，用户已拍板等官方能力）。
- 不发版、不改 Bridge、不改版本号。

## 验收条件

- AC-001: `cargo test --manifest-path src-tauri/Cargo.toml` 全绿，含新增 zcode 单元测试
  （include_zcode 默认开/可关、session 行映射、摘要提取、AskUserQuestion pending →
  waiting、tool running → running、handoff transcript markdown）。
- AC-002: 真实 Tauri 冒烟流程 `npm run test:tauri:flow -- zcode-session` 通过：主窗口出现
  ZCode 会话卡片，点击返回「ZCode session jump is not supported yet」明确提示
  （跳转暂不实现，用户拍板：官方无会话深链、桌面发行版不含 @zcode/tui、独立 CLI 未分发）。
- AC-003: ZCode 会话上下文必须可检索、可预览：接续面板导出源会话 Markdown 落盘
  `handoff-sources\zcode\` 且归属正确（文件名 = 卡片会话 ID）；悬浮预览窗显示
  工作区/标题/状态/最后用户输入/AI 正文且与 db 一致。
- AC-004: 设置面板「Data」组出现 ZCode 会话开关（默认开），主面板 provider 筛选下拉
  含 zcode；中英文、dark/light 双主题下徽标与筛选项渲染正常。
- AC-005: 接续面板可读取 ZCode 源会话导出（`%TEMP%\AgentWatcher\handoff-sources\zcode\`），
  ZCode 仅作为接续来源、不作为接续目标（与 AC-002 的跳转下线一致）。
- AC-006: 性能诊断面板 provider 计数包含 zcode。
- AC-007: AGENTS.md / README 口径同步（数据源路径、ScanOptions 字段、状态机判定、
  测试要求）。
