---
schema_version: 1
protocol: 1.3.0
artifact: summary
artifact_id: ar_01M2J40XQH0008PPQ139448DKH
work_item_id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
created_at: 2026-09-15T08:48:34Z
producer: aes-execute
result: complete
scope: round
supersedes: null
dependencies:
  work_item_contract_digest: sha256:b1d0e0503375d88153a6711d42195454acdbf4b2117b2b4a4a6cca41a6fcc8d9
  artifacts:
    - artifact_id: ar_01M2J40XMD0000GC2DDBG67VQ0
      digest: sha256:534f79f43e09f1c41f80a8eb05c71157e0e8680823f69b4df0fbb7b89a5f165a
      locator: implementation.md
  subject:
    kind: change_set
    digest: sha256:15ddace05bae1e680465fa93adb7d941ce6d9db06fa276ac27b6725893c3a280
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: bc915a208539f73abc4964919eabcfb69e66bbf6
    revision: bc915a208539f73abc4964919eabcfb69e66bbf6
    tree: d752bf95fbe0cca563743eb717c1563c9c38acc2
    content_digest: sha256:15ddace05bae1e680465fa93adb7d941ce6d9db06fa276ac27b6725893c3a280
    branch_or_pr: feature/agent-usage-metrics
    workflow_excluded: true
---

# 成果总结

## 结论

这一轮交付了会话用量展示：卡片显示紧凑用量行，悬浮预览显示完整「用量」区块，数据来自六个扫描源的
真实采集。等你做人工核对（manual-test.md 的 M1–M5）。

## 你现在能做什么

- 不打开各 AI 助手，卡片上直接看到每个会话的对话轮数、工具调用次数和累计 token 消耗。
- 悬停卡片，在预览窗看完整用量：累计输入（带缓存占比）、累计输出、当前上下文、轮次、工具明细排行、
  模型调用次数、模型名、时长，OpenCode 会话还显示美元成本。
- 数据源没有的指标显示「—」，不会把「没有」误读成 0。

## 改了什么

| 变更 | 落在哪 | 为什么 | 锚点 |
| --- | --- | --- | --- |
| 会话数据新增用量字段（token/轮次/工具/模型/时长/成本） | src-tauri/src/lib.rs | 六个扫描源采集，卡片与预览共用 | S1 |
| Claude/Codex/Copilot 会话文件的全量聚合与增量缓存 | src-tauri/src/lib.rs | 累计值必须读全文件，缓存保证文件没变不重读 | S2 |
| ZCode 观测表批量聚合、OpenCode 汇总列、Copilot CLI 轮次 | src-tauri/src/lib.rs | 数据库源走毫秒级 SQL，不碰全表扫描 | S3 |
| 新增用量明细查询命令（工具排行、OpenCode 明细） | src-tauri/src/lib.rs | 大库明细懒加载，悬停时才查 | S4 |
| 卡片紧凑用量行与密度隐藏 | ui/index.html | 不悬停也能扫一眼消耗 | S5 |
| 预览「用量」区块与中英文案 | ui/index.html | 完整指标只在悬停时展开，缺数据显示 — | S6 |

## 每一轮的去向

| 轮 | 去向 |
| --- | --- |
| S1–S8（唯一一轮） | 沿用，构成当前交付 |

## 还欠什么

主动不做：成本价目表估算、趋势图、按工作区汇总面板（设计的非目标）。

没做完：人工核对 M1–M5 等你勾；`opencode-session` flow 在本机因活跃窗口内无 OpenCode 会话无法运行
（环境性，非缺陷）；DPI 125%/150% 与 8 小时长跑未测。
