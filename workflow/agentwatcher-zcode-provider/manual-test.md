---
schema_version: 1
protocol: 1.3.0
artifact: manual-test
artifact_id: ar_01M2FDDE1QT5Q12F8D8C1AH3FK
work_item_id: wi_3QQPFJCPXD24P1CCXMXAZ730TQ
created_at: 2026-09-14T07:52:00Z
producer: aes-validate
result: passed
supersedes: null
dependencies:
  work_item_contract_digest: sha256:efa5a5233be0f6e70b70442ff1fb5cb63dad810669e2356c04643038eb83e195
  artifacts: []
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
---

# 人工核对清单：ZCode Provider（v0.1.5）

以下各项在 2026-09-14 于发布版与 dev 运行时上逐条点过/脚本核过：

- [x] 主窗口出现 ZC 紫色徽标卡片，waiting/running/idle 分道正确（真实 flow 断言 dataset）。
- [x] 悬停 zcode 卡片右下角展开按钮：悬浮预览显示工作区、完整路径、标题、状态、
  最后用户输入与 AI 正文（[预览核验脚本](F:/AiProject/AgentWatcher/.tmp/zcode-preview-check.mjs)，
  2/2 段非空且与 db 一致）。
- [x] 点击 zcode 卡片：toast 明确「跳转暂不支持」，无终端窗口弹出。
- [x] 右键 zcode 卡片发起接续：prompt 的会话文件与被点卡片一致（换卡不串号），
  导出文件可读、含完整转录（[审计脚本](F:/AiProject/AgentWatcher/.tmp/zcode-prompt-audit.mjs)）。
- [x] 设置 Data 组「ZCode 会话」开关默认开，关闭后卡片消失、重开恢复（flow 勾选逻辑覆盖）。
- [x] provider 筛选下拉含 ZCode 选项，四组合（dark/light × zh/en）徽标与文案正常。
- [x] 从发布目录 `artifacts/AgentWatcher/AgentWatcher.exe` 启动（工作目录 C:\）正常、可退出。
- [x] 性能面板扫描行出现 ZC 计数。

范围外（未测，非本清单阻断项，已记录在 TODO）：125%/150% DPI、8 小时长跑、
PATH 独立 zcode CLI 环境的会话恢复分支。
