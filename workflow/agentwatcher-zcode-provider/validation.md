---
schema_version: 1
protocol: 1.3.0
artifact: validation
artifact_id: ar_01M2F4REPJV65VNY2WGC3W62BF
work_item_id: wi_3QQPFJCPXD24P1CCXMXAZ730TQ
created_at: 2026-09-14T05:08:00Z
producer: aes-validate
outcome: passed
supersedes: null
dependencies:
  work_item_contract_digest: sha256:459f4ee8b901939c53f38c70a8943698719fe1808efb82d10ed86ca691149e99
  artifacts:
    - artifact_id: ar_01M2F4REGYSZC2KA486XPBBE20
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

# 验证：AgentWatcher 支持 ZCode Provider

按 work item AC 逐条（本机为「仅桌面发行版、无独立 zcode CLI」环境）：

- AC-001 通过：`cargo test` 82 passed / 0 failed（含 9 个 zcode 新测试）。
- AC-002 通过：`npm run test:tauri:flow -- zcode-session` 真实 WebView2 CDP 附着 dev 运行时；
  卡片（zcode:sess_493c0cf7…，running，标题/工作区正确）点击后走「TUI 运行时不可用」
  分支，toast 明确报「ZCode TUI runtime not available: 桌面发行版不含 @zcode/tui…」，
  未弹注定失败的终端。PATH 独立 CLI 分支由单测 `zcode_bundle_tui_runtime_requires_sibling_tui_package`
  与 `find_zcode_cli_on_path` 逻辑覆盖，真机分支待有独立 CLI 的环境复验（未测，明示）。
- AC-003 通过：运行时缺失返回明确错误；running 会话警示 toast 已实现
  （警示展示依赖启动成功，本机 TUI 不可用故未在真实点击中呈现——实现路径已过单测与代码审查，标注未真机验证）。
- AC-004 通过：`#includeZcode` 默认开；dark/light × zh/en 四组合徽标（#c586c0 / #71337a）
  与筛选项存在（计算样式 + 截图核验，截图在 .tmp/zcode-visual/）。
- AC-005 通过：`zcode-handoff-context` flow 真实导出
  `%TEMP%\AgentWatcher\handoff-sources\zcode\sess_493c0cf7…-1789362035447.md`（311 KB、
  246 条消息），6 种目标模式（含 zcode → 目标代理: ZCode TUI）全部切换验证。
- AC-006 通过：性能面板 `formatScanMetric`/`scanExplainText` 输出含 ZC 计数（构建验证 + smoke）。
- AC-007 通过：AGENTS.md / README 已同步（数据源、ScanOptions、状态机、TUI 边界、测试要求）。

其他：`node --check` bridge/cli.mjs 通过；`npm run build:ui` 通过；
`node .tmp/bridge-handoff-routing-test.cjs` 通过；`npm run test:tauri:smoke` 通过；
`git diff --check` 通过。未测项：DPI 125%/150%、8 小时长跑、发版流水线（不在本任务范围）。

## 用户二次拍板后（2026-09-14 傍晚）：跳转下线 + 预览/上下文可用性复验

- 跳转下线：`zcode-session` flow 断言点击返回「ZCode session jump is not supported yet」
  提示（无终端 spawn、无 TUI 逻辑残留）；`zcode-handoff-context` flow 的目标模式列表
  不再含 zcode；cargo test 81 通过（TUI 预检测试随实现删除）。
- 悬浮预览真机验证（.tmp/zcode-preview-check.mjs）：悬停 zcode 卡片展开按钮 →
  session-preview 窗口显示工作区 aes-workflow · AiProject + 完整路径 + 标题 + 状态 +
  最后用户输入/最后 AI 正文两段均非空，内容与 db 一致。
- 上下文检索复验：审计脚本逐项对照 db（会话存在、文件归属、工作区一致、快照一致、
  消息数一致）全部通过。
- AGENTS.md / README / 合同 AC 口径同步为「跳转暂不实现」；wayfinder 决策票
  wt_01M2FCG3WW0MQV4ZAG6N2GF96V 已 resolve，map 三条决定，validate 通过。

## 用户实测追加（2026-09-14 下午）

- 用户发现接续 prompt 串号（来源 35d3ab34、导出文件却是 493c0cf7）：已定位为
  sourceContext 无归属校验的通用缺陷并修复（见 implementation.md 偏差 5）。
- 修复后逐项审计（.tmp/zcode-prompt-audit.mjs，prompt 对照 db 事实）：
  会话 ID 存在 ✓；来源文件存在且文件名/文件内 Session ID 与卡片一致 ✓；
  主要来源文件行与会话文件行一致 ✓；工作区存在且等于 db.directory ✓；
  快照最后用户输入/AI 正文与 db 最后一条一致 ✓；导出消息数 96 = db 96 ✓。
- 修复后 `zcode-handoff-context` flow（点最后一张 zcode 卡 + 文件归属断言）连续通过；
  `zcode-session` flow 通过（TUI 不可用分支，明确报错）。
- TUI 二轮调查（本轮新增证据）：桌面最新版 3.12.1 解包验证无 @zcode/tui；
  SEA 缓存目录不存在；npm/npmmirror 均无 @zcode/tui；社区 zcode-cli-stream 为
  unofficial（自研 pi-tui）。结论维持：官方渠道当前无法获得可用的官方 TUI 运行时。
