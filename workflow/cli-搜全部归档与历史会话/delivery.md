---
schema_version: 1
protocol: 1.3.0
artifact: delivery
artifact_id: ar_01M5C8QHP500090HGYYPXFQRJQ
work_item_id: wi_01M5C61SH5000FME6B0YSBBTDE
created_at: 2026-10-20T09:25:00Z
producer: aes-finish
outcome: ready_to_land
supersedes: null
landing_branch: "feature/cli-搜全部归档与历史会话"
dependencies:
  work_item_contract_digest: sha256:66c5eb385a5e74388b6cd2c4a87f41f8d92e69603379b6bb23da5196597a30de
  artifacts: []
  subject:
    kind: change_set
    digest: sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: 98ec497f1e2e3a19f9c32cd5db927a9b0c95b764
    revision: 39962ef838af0abb0f1e502d36490995ccca908c
    branch_or_pr: feature/cli-搜全部归档与历史会话
    workflow_excluded: true
---

# 交付：CLI 全历史与归档会话搜索（准备阶段）

## 要合入什么

分支 `feature/cli-搜全部归档与历史会话` 上的代码提交 `39962ef`（9 文件，+674/−112）与本任务全部记录：`resolve_scan_options` 全历史模式、SQLite 扫描上限解除、批量 messageCount、Codex `.jsonl.zst` 解压、`lookup_session_full_history` 定向兜底、CLI `--include-archived`、zst 解压副本导出、skill/AGENTS 文档口径、测试锚点。

## 证据链

- 施工回执：`builds/1.md`
- 代码评审 approved：`evidence/code-review.md`
- 验收五条 passed（真实数据）：`evidence/validation.md`
- 人工核对一条待测：`manual-test.md`（部署新 CLI 后端到端体验，属人才能下的判断）

## 合入后的部署动作（不在本任务代码内）

1. 新 `agentwatcher-cli.exe` 要替换 PATH 上的旧版（`npm run package:exe` 产物或手动替换 `~/.local/bin/agentwatcher-cli`）。
2. 跑 `agentwatcher-cli skill install` 刷新 `~/.zcode/skills/agentwatcher/SKILL.md` 等宿主技能目录，否则技能文档仍是旧口径（无 `--include-archived` 说明）。
3. 发版时按发布规范补跑 `npm run test:tauri:smoke`（本任务无 UI 改动，评审里说明了为何没跑）。

## 回滚办法

1. `git revert 39962ef` 还原全部代码；`Cargo.toml` 的 ruzstd 依赖随之消失。
2. 无数据迁移、无 Bridge 变更、无 GUI 行为变化；临时目录里的解压副本（`%TEMP%\AgentWatcher\handoff-sources\codex\`）可整目录删除，无其他残留状态。

## 能不能合入的判断

代码侧可以：评审通过、验收全绿、GUI 默认路径有机器锚点与新旧二进制对照。人工核对一条未闭合，任务停在 `in_review` 等人测；那条测的是部署后的体验，不阻塞合入判断本身（准备阶段放行）。
