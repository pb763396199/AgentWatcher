---
schema_version: "1"
protocol: "1.3.0"
artifact: "validation"
artifact_id: "ar_2SMYZVPYBEMM6XCB8893WF3WYM"
work_item_id: "wi_5VYQZTXYZD0H5M6CN9TKTRFJC3"
created_at: "2026-10-30T02:54:52Z"
producer: "aes-validate"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:6848ab9fb6f89246f58decd1503a13d728d7aa614109393c23af3b6f2ffae203"
  artifacts:
    - artifact_id: "ar_35QQ7HZD7QA793KM6KP7NWRYTB"
      digest: "sha256:b7fb0886c5c81d4db8f1c3cc6cb0748b638a46516d11c292e61a4ab62792b8b1"
      locator: "implementation.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "e8dbd949b77d217d5e7bdf7773d5d12e4016a437"
    revision: "e8dbd949b77d217d5e7bdf7773d5d12e4016a437"
    tree: "a2335216ba100fdaf0bd0c46b47716c986e13c10"
    content_digest: "sha256:d17e421222a8a15f1aca54b1872097d7907aa31031f0a21dd0e26af6321ddec4"
    branch_or_pr: "feature/逐轮降智审计器"
    workflow_excluded: true
outcome: "passed"
acceptance:
  - acceptance_id: AC-001
    outcome: passed
    method: 对照设计文档与调查记录逐源核对字段名与本机证据位置
    evidence: design/design-v2.md 的「数据」映射表与 research/provider-audit-signals-research.md 的信号矩阵，六源每条带文件/表/字段名
  - acceptance_id: AC-002
    outcome: passed
    method: cargo test --manifest-path src-tauri/Cargo.toml turn_audit
    evidence: 9 个 turn_audit_* 单元测试通过（分组、四类信号、空值、边界）
  - acceptance_id: AC-003
    outcome: passed
    method: 单元测试夹具（ZCode/OpenCode/Codex/Claude/Copilot Chat 解析器）加真实本机会话 CDP 核对
    evidence: zcode_session_timeline_reads_model_usage_rows_and_stamps 等解析器测试通过；真实壳上 ZCode 会话 4 行审计、交接包内 Codex 会话 ndjson 首行为真实数据（gpt-6-astra/low）；Copilot CLI 不支持原因记录于调查记录「查到的：关键细节」
  - acceptance_id: AC-004
    outcome: passed
    method: 真实 Tauri 壳（worktree 前端 + 隔离 CDP）逐项核对，用户本人试玩打包版本
    evidence: 预览摘要行与分析面板审计表在真实壳可见（.tmp/ui-audit-check.mjs 输出）；文案中英文成对、颜色走主题变量；用户试玩后指示合入
  - acceptance_id: AC-005
    outcome: passed
    method: 信号判定单元测试与 UI 徽标列核对
    evidence: turn_audit_flags_reliability_signals、turn_audit_flags_model_change_between_turns 等通过；审计表信号列为独立徽标，无合成分数
  - acceptance_id: AC-006
    outcome: passed
    method: cargo test 全量与真实交互测试
    evidence: cargo test 126+8+9 全过；npm run test:tauri:smoke 通过；session-package 流程通过且解包核对 audit/turns.ndjson 与 manifest 条目
---
# 验收

六条验收全部通过。执行环境：Windows 11 26200、Node 24.18、Rust 工作区 target dev/release 双档、真实本机数据源（ZCode 754MB 库、Codex rollout、Claude 557 会话文件、OpenCode 10.6GB 库）。补跑说明：验收基线为 e8dbd94（含三处预览修正的最终提交），与评审同一变更集。
