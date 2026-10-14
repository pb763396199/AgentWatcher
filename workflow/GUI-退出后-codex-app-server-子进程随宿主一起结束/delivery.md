---
schema_version: 1
protocol: 1.3.0
artifact: delivery
artifact_id: ar_01M4WQWTF8000EVW4T1NAYN4Y8
work_item_id: wi_5SYZXNKVTFYTV99EVBBBMJK0S6
created_at: 2026-10-14T16:44:00Z
producer: aes-finish
outcome: delivered
supersedes: null
landing_revision: "600df7183d583146885548753268fa38bac740f7"
landing_branch: "feature/GUI-退出后-codex-app-server-子进程随宿主一起结束"
dependencies:
  work_item_contract_digest: sha256:2bf0546a812236ff05d377823c4cea07a5a43bdabe190a850ba43a1afb2433e4
  artifacts: []
---

# 交付：常驻子服务 kill-on-close 作业（关闭阶段）

## 合入了什么

分支 `feature/GUI-退出后-codex-app-server-子进程随宿主一起结束` 快进合入 `dev`：修复提交 `600df71`（`src-tauri/src/lib.rs`、`src-tauri/Cargo.toml`）与本任务全部记录。不留 merge commit。

## 证据链

- 施工与调试回执（根因、五条假设、两条泄漏路径、修复）：`builds/1.md`
- 代码评审 approved：`evidence/code-review.md`
- 验收三条 passed：`evidence/validation.md`
- 人工核对零条目（有理由）：`manual-test.md`

## 等价性判断

`dev` 自任务切出（`12491fa`）后无新提交，快进合入即原样落地，无冲突重放，不触发补跑验收。

## 回滚办法

1. `git revert 600df71` 还原作业接管与退出钩子改动。
2. 回滚后旧泄漏行为恢复（仅资源清理层面），无数据迁移、无配置变更、无 Bridge 变更。
