---
schema_version: 1
protocol: 1.3.0
artifact: delivery
artifact_id: ar_01M5CA2C0C0007J1ZZ1HX0793J
work_item_id: wi_01M5C61SH5000FME6B0YSBBTDE
created_at: "2026-10-20T10:45:00Z"
producer: aes-finish
outcome: delivered
supersedes: [ar_01M5C9R0A70003NYMZ9EGCR20J]
landing_revision: "141fb7659875fa25e8fd4bc6aa8e6b437ff8c005"
landing_branch: "dev"
dependencies:
  work_item_contract_digest: sha256:66c5eb385a5e74388b6cd2c4a87f41f8d92e69603379b6bb23da5196597a30de
  artifacts: []
  subject:
    kind: change_set
    digest: sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: 98ec497f1e2e3a19f9c32cd5db927a9b0c95b764
    revision: 141fb7659875fa25e8fd4bc6aa8e6b437ff8c005
    branch_or_pr: feature/cli-搜全部归档与历史会话
    workflow_excluded: true
---

# 交付：CLI 全历史与归档会话搜索（关闭）

## 合入了什么

分支 `feature/cli-搜全部归档与历史会话` 快进合入 `dev`（基线 98ec497 即最新 dev，rebase 为空操作，无冲突重放）。代码落地版本 `141fb76`：全历史与归档搜索主体（`39962ef`）+ 列表 `updatedMs` 与技能文档时间口径（`141fb76`），加本任务全部记录。不留 merge commit。

## 证据链

- 施工回执两轮：`builds/1.md`、`builds/2.md`
- 代码评审 approved（锚 `141fb76`，覆盖两轮增量）：`evidence/code-review.md`
- 验收五条 passed（真实数据，锚 `141fb76` 复验）：`evidence/validation.md`
- 人工核对闭合（用户授权 subagent 代测）：`manual-test.md`
- merge-check 通过（in_review + 待合入记录）；合并前全量回归 `cargo test --manifest-path src-tauri/Cargo.toml` 在 `141fb76` 全绿（`awf gates regress` 因工具假设 manifest 在仓库根而误报，已用项目规范命令补跑；其间有一次后台并发跑出现 1 例不可复现的失败，随后四轮完整重跑全部通过，失败用例名因日志截断未能留存，按 flaky 记录在案）
- 部署已提前完成：`~/.local/bin/agentwatcher-cli`（备份 `agentwatcher-cli.bak-0.1.7`）、`~/.zcode/skills/agentwatcher/SKILL.md`（备份 `SKILL.md.bak-old`），部署的二进制与落地代码同源，无需重复动作

## 回滚办法

1. `git revert 141fb76 39962ef` 还原全部代码；ruzstd 依赖随之消失。
2. 部署回滚：`cp ~/.local/bin/agentwatcher-cli.bak-0.1.7 ~/.local/bin/agentwatcher-cli`，技能文档用 `SKILL.md.bak-old` 覆盖。
3. 临时目录解压副本（`%TEMP%\AgentWatcher\handoff-sources\codex\`）可整目录删除。

## 合入判断

评审、验收、人工核对、merge-check、全量回归全部闭合；dev 无并发提交，快进合入即原样落地，等价性成立。
