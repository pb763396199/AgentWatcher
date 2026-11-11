---
schema_version: "1"
protocol: "1.3.0"
artifact: "implementation"
artifact_id: "ar_4DY7MSFTTEHHVAVJYAKSSJKAA5"
work_item_id: "wi_7FY75QZZZZ3Z0YTX8PZVK8N0BN"
created_at: "2026-11-11T07:51:51Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:ac9ef900a601c9ec2c7772f8a1df16791d2d07a04de86d850b7c7a89ea08446b
  artifacts:
    - artifact_id: "ar_0SZ7WSZSNXHXS9TJCQEG17RBM2"
      digest: sha256:34726801246335337a2e0f8c8124a7cb18742fd87f25dd9d3a768e771015c718
      locator: "change-note.md"
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
result: "complete"
---
# 实现回执：CLI 会话命令接受 Codex 深链与裸 UUID

## 做了什么

按计划 S1–S5 全部完成，一次提交 `f439e31`：`codex_session_key` 改 pub；`normalize_lookup_id`（三形态限定 + 形状校验）接入三个命令入口；cli_contract 补深链/裸形态 not-found 断言并同步 SKILL.md v3 版本断言；commands.rs 内嵌三条单元测试；AGENTS.md 与内嵌 SKILL.md 口径同步。

## 逐步结果

| 步 | 结果 |
| --- | --- |
| S1 lib.rs 改 pub | 完成，lib 单元测试 177 passed（含既有 `codex_session_key_accepts_cli_id_and_deep_link`） |
| S2 helper + 三入口接线 | 完成；首轮单元测试抓到 `vscode://threads/<uuid>` 被吸入，收紧形态限定后全过 |
| S3 契约测试 | 完成，`cli_contract` 13 passed / `cli_taxonomy` 19 passed |
| S4 文档同步 | 完成，AGENTS.md 一处、SKILL.md 两处（口径行 + version） |
| S5 全量回归 + 真机复验 | 完成：全量 cargo test 绿；真机深链/裸 UUID 三命令 ok:true，全零 UUID 退出码 1 |

## 改了哪些代码

`src-tauri/src/lib.rs`（1 行 + 注释）、`src-tauri/src/bin/agentwatcher-cli/commands.rs`（helper 22 行 + 三入口各 2 行 + 测试 53 行）、`src-tauri/tests/cli_contract.rs`（+28 行）、`src-tauri/src/bin/agentwatcher-cli/skill/SKILL.md`（2 行）、`AGENTS.md`（1 行）。

## 自审发现的问题

| 问题 | 怎么发现的 | 修了没有 |
| --- | --- | --- |
| 初版 helper 直接复用 `codex_session_key`，`vscode://threads/<uuid>` 会被吸进 codex 命名空间 | 单元测试 `test_normalize_lookup_id_passthrough_non_codex` 首跑失败 | 修了：先限定三种明确形态（`codex://threads/` 前缀 / `codex:` 前缀 / 不含 `:` 与 `/` 的裸串）再交形状校验，反例进测试 |
| SKILL.md 升 v3 后 `skill_print_and_status_expose_doc_surface` 两处硬编码断言失败 | 跑契约测试当场红 | 修了：断言同步为 3 |

## 哪里没按计划走

| 偏差 | 不做它验收标准能达成吗 |
| --- | --- |
| helper 比计划多了一层「三形态限定」（计划只说复用 `codex_session_key`） | 不能。没有它，AC-003（其他形态原样精确匹配）证不干净——别家 URL 会被改写 |
| 任务记录中途因我误删主检出副本重建过一次（内容从 `.tmp` 契约草稿恢复，ID 不变） | 能。记录是流程工件不是代码，恢复后校验通过；如实记录在此 |

## 跑过的检查

- `cargo test --manifest-path src-tauri/Cargo.toml`（全量）：lib 177 / bin 8 / cli_contract 13 / cli_taxonomy 19，0 failed。
- 真机复验（提交后重跑）：深链与裸 UUID 形态下 `session show` / `session usage` / `handoff export` 全部 ok:true；全零 UUID 两形态退出码 1。
- `awf gates commit-guard`：通过。

## 剩余风险

大写 UUID 与深链尾斜杠等拼写变体不折叠（与 suggest 现口径一致，评审记录登记为非阻断）；无其他已知风险。
