---
schema_version: 1
protocol: 1.3.0
artifact: delivery
artifact_id: ar_01M5C9R0A70003NYMZ9EGCR20J
work_item_id: wi_01M5C61SH5000FME6B0YSBBTDE
created_at: "2026-10-20T10:25:00Z"
producer: aes-finish
outcome: ready_to_land
supersedes: [ar_01M5C8QHP500090HGYYPXFQRJQ]
landing_branch: "feature/cli-搜全部归档与历史会话"
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

# 交付：CLI 全历史与归档会话搜索（准备阶段，第二轮后）

## 要合入什么

分支 `feature/cli-搜全部归档与历史会话` 两个代码提交：`39962ef`（全历史与归档搜索主体，9 文件 +674/−112）与 `141fb76`（列表补 `updatedMs` + SKILL.md 时间口径），加本任务全部记录。

## 证据链

- 施工回执两轮：`builds/1.md`、`builds/2.md`
- 代码评审 approved（锚 `141fb76`，覆盖两轮）：`evidence/code-review.md`
- 验收五条 passed（真实数据，锚 `141fb76` 复验）：`evidence/validation.md`
- 人工核对已闭合：`manual-test.md`（用户授权 subagent 代测，结果与耗时记录在案；本人可随时按同一路径复现）

## 部署状态（相对第一版的变化）

用户授权代测后部署已提前完成：`~/.local/bin/agentwatcher-cli` 为第二轮 release 构建（旧版备份 `agentwatcher-cli.bak-0.1.7`）；`~/.zcode/skills/agentwatcher/SKILL.md` 已刷新（旧版备份 `SKILL.md.bak-old`）。合入 dev 后正常发版即可，无需重复部署动作；回滚时用上述备份。

## 回滚办法

1. `git revert 141fb76 39962ef` 还原全部代码；`Cargo.toml` 的 ruzstd 依赖随之消失。
2. 部署回滚：`cp ~/.local/bin/agentwatcher-cli.bak-0.1.7 ~/.local/bin/agentwatcher-cli`，技能文档用 `SKILL.md.bak-old` 覆盖回去。
3. 临时目录解压副本（`%TEMP%\AgentWatcher\handoff-sources\codex\`）可整目录删除，无其他残留状态。

## 能不能合入的判断

可以：评审、验收、人工核对全部闭合，GUI 默认路径有机器锚点与新旧二进制对照。任务保持 `in_review`，等用户明确发话后 rebase 到 `dev` 快进合入并走关闭阶段。
