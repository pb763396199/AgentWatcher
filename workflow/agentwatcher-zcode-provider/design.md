---
schema_version: 1
protocol: 1.3.0
artifact: design
artifact_id: ar_01M2F4RE4Q0RM4E1KHYJW8HJQ5
work_item_id: wi_3QQPFJCPXD24P1CCXMXAZ730TQ
created_at: 2026-09-14T05:05:00Z
producer: aes-plan
result: accepted
supersedes: null
dependencies:
  work_item_contract_digest: sha256:efa5a5233be0f6e70b70442ff1fb5cb63dad810669e2356c04643038eb83e195
  artifacts: []
---

# 设计：AgentWatcher 支持 ZCode Provider

## 数据源

只读 SQLite：`%USERPROFILE%\.zcode\cli\db\db.sqlite`（桌面版与 CLI 共用，WAL）。
`session.directory` 为明文 workspace；`message`/`part` 的 `data` JSON 列与 OpenCode db 同构，
完全复用 `scan_opencode_sessions` 管线模式（三层缓存、摘要预算、per-provider 配额）。

## 状态机

- waiting：tool part `AskUserQuestion` 且 `state.status='pending'`（经共享 user-control 文本命中）。
- running：assistant message 无 `time.completed/finish/error`，或 tool part pending/running。
- 新鲜度必须用 message/part 内容时间戳；`session.time_updated` 是亚秒级心跳，空闲可能空刷。
- `turn_usage` 实测只在轮次结束落盘，对 running 判定无效，弃用。
- 过滤 `task_type='subagent_child'` 子会话。

## 跳转（wayfinder 结论 + 用户拍板）

- 卡片点击 = 官方 CLI TUI 会话级恢复（`--resume <id> --cwd <ws>`），桌面工作区级跳转不做。
- 运行时边界（实施中发现，见 wayfinder 票 wt_01M2F47PRW…）：官方桌面发行版不含 `@zcode/tui`
  （SEA 资产只在独立 CLI；npm 无官方包）。实现为：PATH 独立 `zcode` 优先 → bundle 预检
  `node_modules/@zcode/tui/dist/index.js` → 不可用即明确报错，不弹注定失败的终端。
- running 会话点击后弹并行恢复警示（不阻断）。

## Handoff

源上下文从 db 导出 Markdown 到 `%TEMP%\AgentWatcher\handoff-sources\zcode\`；
目标模式 `zcode` = prompt 进剪贴板 + 新终端 TUI 新会话，无 ack。

## 放弃的选项

桌面深链（实测无 session 路由，feedback#465）、远控 relay（无文档）、app-server 协议宿主
（需自建聊天 UI + 反向请求处理，Future）。
