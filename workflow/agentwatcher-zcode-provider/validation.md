---
schema_version: 1
protocol: 1.3.0
artifact: validation
artifact_id: ar_01M2FDDDHW446PZ1GSAEA7VCJY
work_item_id: wi_3QQPFJCPXD24P1CCXMXAZ730TQ
created_at: 2026-09-14T07:50:00Z
producer: aes-validate
outcome: passed
executed_at: 2026-09-14T15:55:00+08:00
environment: Windows 11 10.0.26200 / ZCode 桌面 3.11.2（无独立 CLI）/ VS Code + Bridge 0.1.12
supersedes: null
dependencies:
  work_item_contract_digest: sha256:efa5a5233be0f6e70b70442ff1fb5cb63dad810669e2356c04643038eb83e195
  artifacts:
    - artifact_id: ar_01M2F4REGYSZC2KA486XPBBE20
      digest: sha256:4e3b4bf048f5f730e1accd05699dd51f44edb4963d1a706602107f6cf2a82d93
      locator: implementation.md
  subject:
    kind: change_set
    digest: sha256:b60025e1431c0ee311f478d7afc97e421507a7622c71084518b4a11adc575a21
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: cd721467e82a7199ca0da299071e2b99f33312b7
    revision: 10649708e7168a397c850b38fccf1b31cecebfbd
    tree: 1f344685d607a9bccaacef077f1e1ec3d07fc9be
    content_digest: sha256:b60025e1431c0ee311f478d7afc97e421507a7622c71084518b4a11adc575a21
    branch_or_pr: dev
    excluded_prefixes: []
    workflow_excluded: true
acceptance:
  - acceptance_id: AC-001
    outcome: passed
    method: cargo test --manifest-path src-tauri/Cargo.toml
    evidence: 81 passed / 0 failed（含 8 个 zcode 新测试：行映射、摘要、waiting/running hint、心跳免疫、handoff markdown）
  - acceptance_id: AC-002
    outcome: passed
    method: npm run test:tauri:flow -- zcode-session（真实 WebView2 CDP）
    evidence: .tmp/tauri-realtest/20260914-151918/result.json 通过；Toast=「ZCode session jump is not supported yet…」，无终端 spawn
  - acceptance_id: AC-003
    outcome: passed
    method: zcode-handoff-context flow + .tmp/zcode-prompt-audit.mjs + .tmp/zcode-preview-check.mjs 逐项对照 db
    evidence: 导出文件名=卡片会话 ID、文件内 Session ID 一致、工作区=db.directory、快照与 db 最后一条一致、消息数 96=96；悬浮预览窗口工作区/标题/状态/两段正文全非空且与 db 一致
  - acceptance_id: AC-004
    outcome: passed
    method: CDP 计算样式四组合 + 截图核验
    evidence: .tmp/zcode-visual/（dark/light × zh/en 徽标 #c586c0/#71337a，#includeZcode 默认开，筛选项含 zcode）
  - acceptance_id: AC-005
    outcome: passed
    method: npm run test:tauri:flow -- zcode-handoff-context（点最后一张 zcode 卡）
    evidence: 导出落盘 %TEMP%\AgentWatcher\handoff-sources\zcode\sess_<卡片ID>-<ts>.md；5 种目标模式 prompt 均携带来源文件路径；ZCode 无目标模式（与 AC-002 一致）
  - acceptance_id: AC-006
    outcome: passed
    method: npm run build:ui + npm run test:tauri:smoke + 源码走查
    evidence: build:ui 退出码 0（vite 343ms）；smoke result.json 通过（.tmp/tauri-realtest/20260914-150651）；formatScanMetric/scanExplainText 输出行均拼入「ZC <n>」计数
  - acceptance_id: AC-007
    outcome: passed
    method: 文档口径复查
    evidence: AGENTS.md/README/TODO/CHANGELOG 均为「跳转暂不实现、扫描/预览/接续导出可用」口径；发布包与 SHA256 已写入 CHANGELOG
---

# 验证：AgentWatcher 支持 ZCode Provider（对齐最终版合同）

按合同最终版（跳转下线后的 AC-001~AC-007）重新核对，合并此前各轮验证结论，
acceptance 明细见头部。发布产物验证：zip SHA256
`EC356AF8FABAECE38FA8E1C75DB1B1D19C43BDC902C17C76E324AF22EF653DE7`、
VSIX 重复安装两次后仅 `agentwatcher.agentwatcher-vscode-session-bridge@0.1.12`、
发布目录 EXE 自 `C:\` 工作目录独立启动成功并可正常退出。
未测项（非阻断，TODO 已列）：125%/150% DPI、8 小时长跑、PATH 独立 CLI 环境的恢复分支。
