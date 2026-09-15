---
schema_version: 1
protocol: 1.3.0
artifact: delivery
artifact_id: ar_01M2J4V005000BS0H7KT8XZR74
work_item_id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
created_at: 2026-09-15T08:55:00Z
producer: aes-finish
outcome: ready_to_land
supersedes: null
dependencies:
  work_item_contract_digest: sha256:b1d0e0503375d88153a6711d42195454acdbf4b2117b2b4a4a6cca41a6fcc8d9
  artifacts:
    - artifact_id: ar_01M2J40XMD0000GC2DDBG67VQ0
      digest: sha256:534f79f43e09f1c41f80a8eb05c71157e0e8680823f69b4df0fbb7b89a5f165a
      locator: implementation.md
    - artifact_id: ar_01M2J4TZXS000EDM8FRTSXSZB2
      digest: sha256:57f56bd4964bf8c99ab2a409bf750103c1e202d62649bff745c40244ac3aef3d
      locator: work-item-summary.md
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
landing_revision: null
landing_branch: null
---

# 交付记录（准备阶段）

## 要落地什么

提交 `cd03bed`（feature/agent-usage-metrics，基线 bc915a2）：src-tauri/src/lib.rs 与 ui/index.html
两个文件的会话用量功能，+1896/-14 行。功能范围、采集公式与验证证据见任务目录各记录。

## 证据链接

- 实现：[implementation.md](F:\AiProject\AgentWatcher\.aes-workflow\worktrees\agent-usage-metrics\workflow\agent-usage-metrics\implementation.md)
- 评审：[reviews/code-review.md](F:\AiProject\AgentWatcher\.aes-workflow\worktrees\agent-usage-metrics\workflow\agent-usage-metrics\reviews\code-review.md)（approved，自审声明）
- 验收：[validation.md](F:\AiProject\AgentWatcher\.aes-workflow\worktrees\agent-usage-metrics\workflow\agent-usage-metrics\validation.md)（AC-001/002/003/005 passed，AC-004 留人工）
- 人工核对：[manual-test.md](F:\AiProject\AgentWatcher\.aes-workflow\worktrees\agent-usage-metrics\workflow\agent-usage-metrics\manual-test.md)（M1–M5 未勾）
- 任务总结：[work-item-summary.md](F:\AiProject\AgentWatcher\.aes-workflow\worktrees\agent-usage-metrics\workflow\agent-usage-metrics\work-item-summary.md)

## 怎么回滚

1. `git revert cd03bed`（或 `git reset --hard bc915a2` 回到基线，分支未合入 dev 前均可直接删）。
2. 无数据迁移、无持久化新增：用量缓存全在进程内，回滚后进程重启即清。
3. 无发布合同影响：Bridge 与插件接口零改动，回滚不需要动 VSIX。

## 落地判断

变更只加不改：AgentSession.usage 为 Option 且序列化跳过 None，旧前端拿到新字段无感；
六个采集路径全部带失败降级（读不到就 None，界面显示 —）。风险集中在首扫渐进填充的
等待体验与 opencode flow 的环境性失败，均已在实现记录写明。等 M1–M5 勾完即可合并。
