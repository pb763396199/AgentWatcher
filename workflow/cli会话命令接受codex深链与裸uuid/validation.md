---
schema_version: "1"
protocol: "1.3.0"
artifact: "validation"
artifact_id: "ar_43WFMXFCPFGV53TS26HFDEQ9P1"
work_item_id: "wi_7FY75QZZZZ3Z0YTX8PZVK8N0BN"
created_at: "2026-11-11T07:51:52Z"
producer: "aes-validate"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:ac9ef900a601c9ec2c7772f8a1df16791d2d07a04de86d850b7c7a89ea08446b
  artifacts:
    - artifact_id: "ar_4DY7MSFTTEHHVAVJYAKSSJKAA5"
      digest: sha256:d489aac3989e51482d3ad5b45787d1131addd94684a064ecf684c886f38013e0
      locator: "implementation.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "f439e318bfd80077dfeeedda34a07f117b543b1f"
    revision: "f439e318bfd80077dfeeedda34a07f117b543b1f"
    tree: "cfdbc1ba4089b5a03c8161310fed3bf9b77e5a0c"
    content_digest: "sha256:a1cc9c749a48aa4803a82b0a7f1a7536a7bf39ecd29ab053fce681fa786dff9d"
    branch_or_pr: "feature/cli会话命令接受codex深链与裸uuid"
    workflow_excluded: true
outcome: "passed"
acceptance:
  - acceptance_id: "AC-001"
    outcome: "passed"
    method: "cargo test --manifest-path src-tauri/Cargo.toml --bin agentwatcher-cli test_normalize_lookup_id_accepts_codex_forms；真机以深链形态分别执行 session show / session usage / handoff export"
    evidence: "单元测试 ok（3 passed）；真机三命令均 ok:true，session show 返回 data.session.id == codex:01a1c945-78f0-7882-a2cb-6ae5539162eb，handoff export 返回该会话 rollout JSONL 路径"
  - acceptance_id: "AC-002"
    outcome: "passed"
    method: "cargo test --manifest-path src-tauri/Cargo.toml --bin agentwatcher-cli test_normalize_lookup_id_accepts_bare_uuid；真机以裸 UUID 形态分别执行 session show / session usage / handoff export"
    evidence: "单元测试 ok；真机三命令均 ok:true，show 命中与 codex:<uuid> 相同会话"
  - acceptance_id: "AC-003"
    outcome: "passed"
    method: "cargo test --manifest-path src-tauri/Cargo.toml --bin agentwatcher-cli test_normalize_lookup_id_passthrough_non_codex"
    evidence: "单元测试 ok：zcode/claude/opencode/workbuddy/非 UUID codex 键/空串/别家 URL 全部原样通过断言"
  - acceptance_id: "AC-004"
    outcome: "passed"
    method: "cargo test --manifest-path src-tauri/Cargo.toml --test cli_contract not_found_deep_link_and_bare_uuid_still_fail_cleanly（真实二进制）；真机直接执行两形态看退出码"
    evidence: "契约测试 ok：信封五键、error.code == session_not_found、data 为 null、退出码 1；真机深链/裸全零 UUID 退出码均为 1"
  - acceptance_id: "AC-005"
    outcome: "passed"
    method: "在提交 f439e31 之后的同一工作区跑全量 cargo test --manifest-path src-tauri/Cargo.toml"
    evidence: "lib 177 passed / 0 failed、agentwatcher-cli bin 8 passed / 0 failed、cli_contract 13 passed / 0 failed、cli_taxonomy 19 passed / 0 failed（2 ignored 为既有忽略项）"
executed_at: "2026-11-11T07:55:00Z至08:05:00Z"
environment: "Windows 11（10.0.26200），worktree 构建 src-tauri/target/debug/agentwatcher-cli.exe；本机存在真实 Codex 会话 01a1c945-78f0-7882-a2cb-6ae5539162eb（即触发本次调查的那条深链）"
---
# 验收：CLI 会话命令接受 Codex 深链与裸 UUID

五条验收全部在提交 `f439e31` 之后重跑通过。真机复验用的正是触发本次调查的那条深链——修复前它报 `session_not_found`，现在三命令直接命中。人工核对清单零条目，理由见 `manual-test.md`。
