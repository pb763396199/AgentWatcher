---
schema_version: 1
protocol: 1.3.0
artifact: summary
artifact_id: ar_01M2J4TZXS000EDM8FRTSXSZB2
work_item_id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
created_at: 2026-09-15T08:55:00Z
producer: aes-finish
result: complete
scope: work_item
supersedes: null
dependencies:
  work_item_contract_digest: sha256:b1d0e0503375d88153a6711d42195454acdbf4b2117b2b4a4a6cca41a6fcc8d9
  artifacts:
    - artifact_id: ar_01M2J40XQH0008PPQ139448DKH
      digest: sha256:64eb30e0c41ff47d39e9eb7fd5dc9d4a6da4d90a454a2f649c4c43687ed8f3bb
      locator: summary.md
  subject:
    kind: change_set
    digest: sha256:15ddace05bae1e680465fa93adb7d941ce6d9db06fa276ac27b6725893c3a280
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: bc915a208539f73abc4964919eabcfb69e66bbf6
    revision: cd03bed8df4ab0cad1509f0a9ba7b97cb95a0b59
    tree: 71f902930d26313687b228b90f1e390e5573740d
    content_digest: sha256:15ddace05bae1e680465fa93adb7d941ce6d9db06fa276ac27b6725893c3a280
    branch_or_pr: feature/agent-usage-metrics
    workflow_excluded: true
---

# 任务总结

## 结论

这个任务给 AgentWatcher 加上了会话用量展示：六个扫描源各自采集 token、轮次、工具、模型、时长、成本，
卡片显示紧凑用量行，悬浮预览显示完整用量区块。代码已提交在特性分支，等你人工核对后合入。

## 你现在能做什么

- 不打开各 AI 助手，卡片直接看到每个会话的轮数、工具调用数、累计 token。
- 悬停卡片看完整用量：累计输入（带缓存占比）、当前上下文、工具排行、模型、时长，OpenCode 还有美元成本。
- 数据源没有的指标显示「—」，不会把订阅制的 ZCode 误读成免费。

## 改了什么

| 变更 | 落在哪 | 为什么 | 锚点 |
| --- | --- | --- | --- |
| 会话用量数据模型与六源采集（含增量缓存、懒加载命令） | [src-tauri/src/lib.rs](F:\AiProject\AgentWatcher\.aes-workflow\worktrees\agent-usage-metrics\src-tauri\src\lib.rs) | 卡片与预览共用的数据底座 | S1–S4 |
| 卡片用量行、预览用量区块、中英文案 | [ui/index.html](F:\AiProject\AgentWatcher\.aes-workflow\worktrees\agent-usage-metrics\ui\index.html) | 不悬停扫一眼消耗，悬停看明细 | S5–S6 |

## 每一轮的去向

| 轮 | 去向 |
| --- | --- |
| S1–S8（研究→设计→实现→验证，唯一一轮） | 沿用，提交 cd03bed |

## 还欠什么

主动不做：成本价目表估算、趋势图、工作区汇总面板（设计非目标）。

没做完：人工核对 M1–M5（[manual-test.md](F:\AiProject\AgentWatcher\.aes-workflow\worktrees\agent-usage-metrics\workflow\agent-usage-metrics\manual-test.md)）未勾；
`opencode-session` flow 在本机因活跃窗口内无 OpenCode 会话无法运行（环境性）；DPI 125%/150% 与 8 小时长跑未测。
