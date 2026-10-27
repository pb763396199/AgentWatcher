---
schema_version: 1
protocol: 1.3.0
artifact: validation
artifact_id: ar_01M5YDKRA70004TZBG43Z5ATBE
work_item_id: wi_25PNYKYZZZEZF9VBX61N79GPEG
created_at: 2026-10-27T10:25:00Z
producer: aes-validate
outcome: passed
supersedes: null
contract_revision: 4
verified_at_commit: c948a518afddfb789250e52485094a5ac85dd09c
dependencies:
  work_item_contract_digest: sha256:e03deeaffb83f1d3d5bcc288625d29642afd773ef027a39c45750a1581c454813
  artifacts: []
---
# 逐条验收（契约 revision 4，提交 c948a51）

- AC-001（菜单 + zip + 剪贴板 + 清理）：方法=真实 Tauri 壳交互流程；证据=`npm run test:tauri:flow -- 打包` 通过（真实打包 Codex 会话，toast「已打包并复制到剪贴板」，剪贴板写入 true，zip 476KB），结果在 worktree `.tmp/tauri-realtest/20261027-181033/result.json`；清理行为由 `case:session_package_cleans_stale_temp_artifacts` 锁定。
- AC-002（六 provider raw 完整）：方法=Rust 单元测试 + 真实数据实证；证据=`case:session_package_covers_all_providers` 通过（Codex 片段链 + 归档、Claude/Copilot 原样文件、Copilot CLI/OpenCode/ZCode NDJSON 全量）；真实归档 `rollout-…-019c413a….jsonl.zst` 一次性打包实证解压 29,309 字节进包（过程记于 builds/1.md）。
- AC-003（manifest/HANDOFF/README 自描述瘦身）：方法=Rust 单元测试 + 真实包人工核对；证据=`case:session_package_manifest_matches_files` 通过（逐文件 SHA256 与包内容一致、HANDOFF 无正文、README 有格式说明）；真实包 `codex-01a17c43-…-1793088092063.zip` 人工核对结构与 manifest 字段；git 锚点由 `case:session_package_records_git_anchor` 用临时真实 git 仓库锁定。
- AC-004（失败路径）：方法=Rust 单元测试；证据=`case:session_package_reports_source_errors` 通过（源缺失报带路径错误、无半截 zip、无 .tmp 残留）。
- AC-005（既有行为不回退）：方法=真实冒烟；证据=`npm run test:tauri:smoke` 通过（`.tmp/tauri-realtest/20261027-181100/result.json`），全量 `cargo test` 117 lib + CLI 契约 8 + 分类 9 全绿。

执行环境：Windows 11，本机真实会话数据，worktree `feature/会话卡片右键打包整个会话跨电脑交接` @ c948a51，Vite 1429 + WebView2 CDP（`window.__TAURI_INTERNALS__ === true`）。
