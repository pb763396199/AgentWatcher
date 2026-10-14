---
schema_version: 1
protocol: 1.3.0
artifact: delivery
artifact_id: ar_01M4W5SPXN000F5MC9025H2ZSM
work_item_id: wi_19X7WBYXTZK9DH6X1KT55XHK7N
created_at: 2026-10-14T11:20:00Z
producer: aes-finish
outcome: delivered
supersedes: [ar_01M4W5DCG10001VTKSS3BNA5PM]
landing_revision: "5387854c43d6ef7fa6d99aed8f41f9c1bdf1cf7b"
landing_branch: "feature/主窗口搜索栏支持用会话-ID-过滤卡片"
dependencies:
  work_item_contract_digest: sha256:98aa641eac6856359c6ac518be14749ff547a9c78327586b62d542ed67347bc4
  artifacts: []
---

# 交付：搜索栏按会话 ID 过滤（关闭阶段）

## 合入了什么

分支 `feature/主窗口搜索栏支持用会话-ID-过滤卡片` 快进合入 `dev`：施工提交 `5387854`（`ui/index.html` 搜索过滤增加会话 ID 匹配、`tools/tauri-realtest/cli.mjs` 筛选流程增加 ID 片段搜索断言）与待办提交 `1130029`（既有测试环境问题登记）。不留 merge commit。

## 证据链

- 代码评审 approved：`evidence/code-review.md`
- 验收两条 passed：`evidence/validation-v2.md`
- 人工核对三条全勾（2026-10-14，用户本人在 `manual-test.md` 打勾）：`manual-test.md`
- 全量回归：`cargo test --manifest-path src-tauri/Cargo.toml` 通过（结果见施工环境日志）

## 等价性判断

`dev` 自任务切出后没有新提交（基线 `2e11226` 未动），rebase 为空操作，快进合入即原样落地，无冲突重放，不触发补跑验收。

## 回滚办法

1. `git revert 5387854` 还原两处改动（搜索过滤与测试断言一起退）。
2. 无数据迁移、无配置变更、无 Bridge 变更，无残留状态需要清理。
