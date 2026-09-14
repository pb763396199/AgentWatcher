---
schema_version: 1
protocol: 1.3.0
artifact: implementation
artifact_id: ar_01M2F4REGYSZC2KA486XPBBE20
work_item_id: wi_3QQPFJCPXD24P1CCXMXAZ730TQ
created_at: 2026-09-14T05:07:00Z
producer: aes-execute
result: complete
supersedes: null
dependencies:
  work_item_contract_digest: sha256:459f4ee8b901939c53f38c70a8943698719fe1808efb82d10ed86ca691149e99
  artifacts:
    - artifact_id: ar_01M2F4RE4Q0RM4E1KHYJW8HJQ5
    - artifact_id: ar_01M2F4REB0VAPKGZ4EE052QRTW
  subject:
    kind: change_set
    digest: sha256:9a5af7d9d42069b5bd89c585260d6c378946d3a68bb959be49d71f161e8cfcf9
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: cd721467e82a7199ca0da299071e2b99f33312b7
    revision: cd721467e82a7199ca0da299071e2b99f33312b7
    tree: 66e8b88e1c5ca2bd9a33da06e17e3634074b76ee
    content_digest: sha256:9a5af7d9d42069b5bd89c585260d6c378946d3a68bb959be49d71f161e8cfcf9
    branch_or_pr: dev
    workflow_excluded: true
---

# 实现：AgentWatcher 支持 ZCode Provider（变更未提交，驻留工作区；subject 摘要为工作区 change-digest，revision=HEAD）

## 哪里没按计划走

1. 计划原定「node + zcode.cjs --resume 直接恢复」；实施中实测桌面发行版不含 `@zcode/tui`
   （`zcode tui`/`--resume` 均 Cannot find package），改为「PATH 独立 CLI 优先 + bundle TUI
   预检 + 明确报错」，并补 wayfinder 票 wt_01M2F47PRW03KE6W9NGYHZ63A3 记录证据。
   二轮复核：最新桌面版 3.12.1（2026-09-13 发布）解包验证 glm 仍无 node_modules/@zcode/tui；
   SEA 运行时解压缓存 %LOCALAPPDATA%\zcode\Cache\sea-assets 本机不存在；npm 官方源与
   npmmirror 镜像均无 @zcode/tui（从未公开发布）；CDN 无 CLI 产物。
2. 并行恢复警示从「点击前提示」改为「启动成功后提示」（原实现会被 opening/opened toast
   立即覆盖，用户看不到）。
3. `zcode-session` flow 从「必须无 failed toast」改为双分支断言（恢复成功 / TUI 运行时
   不可用明确报错），并在结果里记录走了哪个分支。
4. 计划外新增 `zcode-handoff-context` flow（镜像 opencode 版），把 AC-005 的真实导出
   验证固化成可回归资产。
5. 用户实测发现接续 prompt 串号（会话 ID 是 A、导出文件是 B）：根因是接续面板把上一次
   导出的 sourceContext 原样带给新来源会话（payload 发布、早退复用、渲染全链路无归属
   校验，opencode 同样存在）。修复：新增 handoffContextMatchesSource 不变量，
   applyHandoffPayload / prepareHandoffSourceContext / ensureHandoffSourceContextReady /
   handoffPortableSourceFile/Summary 四处守卫；两个 handoff flow 增加文件归属断言
   （文件名必须以被点卡片原始会话 ID 开头），zcode flow 改点最后一张卡复现换源场景。
6. 用户二次拍板：跳转整体下线（wayfinder 票 wt_01M2FCG3WW0MQV4ZAG6N2GF96V）。删除
   launch_zcode_tui / ZcodeTuiRuntime / find_zcode_* 全套与 handoff 的 zcode 目标模式、
   并行恢复警示及死键；open_zcode_session 返回「not supported yet」；zcode-session flow
   改为断言该提示。ZCode 本期定位为只读监控 provider：扫描 + 悬浮预览 + 接续上下文导出。

按 plan 全量落地。改动文件：`src-tauri/src/lib.rs`、`ui/index.html`、
`tools/tauri-realtest/cli.mjs`、`AGENTS.md`、`README.md`。

要点与偏差：

- OpenCode 专有 helper 重命名为共享：`agent_part_status_hint`、`assistant_message_status_hint`、
  `HandoffTranscriptMessage/Part`、`handoff_transcript_part_from_data`、
  `write_handoff_source_markdown`、`handoff_inline_summary`、`sqlite_session_scan_limit`。
- 实施中发现并落地运行时边界（wayfinder 票 wt_01M2F47PRW03KE6W9NGYHZ63A3）：
  桌面发行版无 `@zcode/tui`；`find_zcode_tui_runtime` = PATH 独立 CLI → bundle 预检 →
  报「ZCode TUI runtime not available」。
- 并行恢复警示改为启动成功后展示（避免被 opening/opened toast 覆盖）。
- `zcode-session` flow 接受双分支：TUI 恢复成功 / TUI 运行时不可用明确报错。
- 新增 `zcode-handoff-context` flow（镜像 opencode 版，验证 6 种目标模式含 zcode）。
