---
schema_version: 1
protocol: 1.3.0
artifact: plan
artifact_id: ar_01M2F4REB0VAPKGZ4EE052QRTW
work_item_id: wi_3QQPFJCPXD24P1CCXMXAZ730TQ
created_at: 2026-09-14T05:06:00Z
producer: aes-plan
result: ready
supersedes: null
dependencies:
  work_item_contract_digest: sha256:459f4ee8b901939c53f38c70a8943698719fe1808efb82d10ed86ca691149e99
  artifacts:
    - artifact_id: ar_01M2F4RE4Q0RM4E1KHYJW8HJQ5
---

# 计划：AgentWatcher 支持 ZCode Provider

1. Rust M1：`ScanOptions.include_zcode`（默认 true）→ `scan_sessions_blocking` 分派 →
   `scan_zcode_sessions`（db 路径、只读连接、扫描上限、三层缓存、摘要预算 8）→
   `session_watch_roots` 注册 db.sqlite(+wal) → `PerformanceProviderCounts.zcode`。
   顺带把 opencode 专有的 part/message 状态 hint、handoff transcript 结构重命名为共享 helper。
2. Rust 跳转：`open_session` 顶部 zcode 分支 → PATH zcode CLI 优先 → bundle+node 且预检
   TUI 运行时 → `spawn_visible_terminal` 新终端 `--resume <id> --cwd <ws>`；不可用明确报错。
3. Rust M2：`prepare_zcode_handoff_source_context`（导出 Markdown，共享写盘/渲染 helper）+
   `launch_handoff` 新模式 `zcode`（剪贴板 prompt + TUI 新会话，无 ack）。
4. 前端：ZC 徽标（三处 CSS 深浅两套）、`#includeZcode` 设置链路、provider 筛选、
   i18n en/zh、性能面板 ZC 计数、handoff 目标选项与派发映射、running 并行恢复警示。
5. 测试：Rust 单测（映射/摘要/waiting/running/TUI 预检/handoff markdown）；
   cli.mjs `zcode-session` + `zcode-handoff-context` 两个真实 flow。
6. 文档：AGENTS.md（数据源/ScanOptions/状态机/测试要求/跳转边界）+ README 口径。
7. 验收：cargo test、node --check、build:ui、test:tauri:flow（两个 flow）、smoke、
   双主题双语言视觉检查。
