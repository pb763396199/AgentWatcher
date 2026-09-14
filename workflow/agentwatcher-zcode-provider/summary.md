---
schema_version: 1
protocol: 1.3.0
artifact: summary
artifact_id: ar_01M2FDDDQ4HYKPCH5EZ4V0CTQ9
work_item_id: wi_3QQPFJCPXD24P1CCXMXAZ730TQ
created_at: 2026-09-14T07:55:00Z
producer: aes-execute
result: complete
scope: round
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

## 结论

ZCode 作为第 6 个只读监控 provider 交付，v0.1.5 已发布；跳转按你拍板下线，依据与恢复条件在 AGENTS.md。

## 你现在能做什么

- 主窗口可看 ZCode 卡片，悬停即预览最后输入与回复。
- 右键 ZCode 卡片可导出全量上下文接续到其他 provider。
- 点击 ZCode 卡片返回明确「暂不支持」，不再弹秒退终端；接续串号已修复。

## 改了什么

| 变更 | 落在哪 | 为什么 | 锚点 |
| --- | --- | --- | --- |
| ZCode 扫描/状态机/性能计数 | [lib.rs](F:/AiProject/AgentWatcher/src-tauri/src/lib.rs) | 数据面，复用 OpenCode 管线 | 1 |
| 接续导出与「暂不支持」提示 | [lib.rs](F:/AiProject/AgentWatcher/src-tauri/src/lib.rs) | 上下文可检索；跳转无官方通道 | 2-3 |
| 徽标/设置/筛选/预览/归属守卫 | [ui/index.html](F:/AiProject/AgentWatcher/ui/index.html) | 串号修复全 provider 生效 | 4 |
| 两个 zcode 真实流程 | [cli.mjs](F:/AiProject/AgentWatcher/tools/tauri-realtest/cli.mjs) | 回归固化 | 5 |
| 口径文档与决策票据 | [AGENTS.md](F:/AiProject/AgentWatcher/AGENTS.md)、[README.md](F:/AiProject/AgentWatcher/README.md)、wayfinder-tickets/ZCode-卡片点击跳转的可行路线.md、ZCode-TUI-运行时在桌面发行版上的可用性.md、ZCode-卡片跳转是否本期实现.md | 口径与票据 | 6 || 版本/CHANGELOG/TODO/图标/忽略项 | [package.json](F:/AiProject/AgentWatcher/package.json)、[Cargo.toml](F:/AiProject/AgentWatcher/src-tauri/Cargo.toml)、[Cargo.lock](F:/AiProject/AgentWatcher/src-tauri/Cargo.lock)、[tauri.conf.json](F:/AiProject/AgentWatcher/src-tauri/tauri.conf.json)、[CHANGELOG.md](F:/AiProject/AgentWatcher/CHANGELOG.md)、[TODO.md](F:/AiProject/AgentWatcher/TODO.md)、[icon.ico](F:/AiProject/AgentWatcher/src-tauri/icons/icon.ico)、[icon-runtime-256.rgba](F:/AiProject/AgentWatcher/src-tauri/icons/icon-runtime-256.rgba)、[.gitignore](F:/AiProject/AgentWatcher/.gitignore) | v0.1.5 发布对齐与本地产物忽略 | 7 |

## 还欠什么

主动不做：会话级跳转（等官方能力，实现在 git 历史）。没做完：DPI 125%/150%、8 小时长跑、独立 CLI 复验（TODO）。
