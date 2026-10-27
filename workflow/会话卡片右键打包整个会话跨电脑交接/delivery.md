---
schema_version: 1
protocol: 1.3.0
artifact: delivery
artifact_id: ar_01M5YDKREF000EVA9MCRK286JW
work_item_id: wi_25PNYKYZZZEZF9VBX61N79GPEG
created_at: 2026-10-27T10:35:00Z
producer: aes-finish
outcome: ready_to_land
supersedes: null
contract_revision: 4
verified_at_commit: c948a518afddfb789250e52485094a5ac85dd09c
dependencies:
  work_item_contract_digest: sha256:e03deeaffb83f1d3d5bcc288625d29642afd773ef027a39c45750a1581c454813
  artifacts: []
---
# 交付：会话卡片右键打包整个会话跨电脑交接

待合入提交（`feature/会话卡片右键打包整个会话跨电脑交接`，基于 dev@2fd4ab0）：

- `1aef8bf` feat(handoff)：打包核心（六 provider 收集、zip + SHA256 manifest、HANDOFF 定位块、CF_HDROP 剪贴板、session-package 流程）
- `ea160da` feat(handoff)：git 锚点（repo/branch/head/未提交数/未推送数）与 README 格式说明
- `5cb7b19` fix(handoff)：git 子进程补 CREATE_NO_WINDOW（用户实测闪窗缺陷）
- `c948a51` feat(handoff)：temp 产物 7 天清理（session-packages 与 handoff-sources）

配套：AGENTS.md 命令与流程文档同步；`package-lock.json` 版本号 0.1.7→0.1.8 对齐（过期未同步的存量修正）。

评审与验收：evidence/code-review.md（approved）、evidence/validation.md（AC-001..005 全过）、manual-test.md（用户实测通过，英文/浅色样式一项标注未目视）。

用户已于 2026-10-27 指示发布新版本并更新桌面 dev，即授权合入与发布。发布流程（版本号、CHANGELOG、package:exe、GitHub Release）按 AGENTS.md 发布规范在合入后执行，记录于仓库发布提交。
