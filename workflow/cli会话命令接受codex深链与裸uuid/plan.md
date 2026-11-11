---
schema_version: "1"
protocol: "1.3.0"
artifact: "plan"
artifact_id: "ar_6QYZQVF0FAQG1TCGWGGYAB6GPD"
work_item_id: "wi_7FY75QZZZZ3Z0YTX8PZVK8N0BN"
created_at: "2026-11-11T07:51:51Z"
producer: "aes-plan"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:ac9ef900a601c9ec2c7772f8a1df16791d2d07a04de86d850b7c7a89ea08446b
  artifacts:
    - artifact_id: "ar_3ZXZXSFVEY1GC0Q2EYPAT8ZJ19"
      digest: "sha256:931b05469071b2de0c81fc87142e862f5f18f2aad8c66d1166ba1d4084065147"
      locator: "design.md"
result: "ready"
---
# 计划：CLI 会话命令接受 Codex 深链与裸 UUID

改动面小、方向已拍板，计划就是施工顺序，一步一验。

| 步 | 做什么 | 怎么验 |
| --- | --- | --- |
| S1 | `codex_session_key` 改 `pub`（lib.rs，逻辑不动） | lib 单元测试全绿（既有 `codex_session_key_accepts_cli_id_and_deep_link` 覆盖形状） |
| S2 | commands.rs 新增 `normalize_lookup_id`（三形态限定 + 形状校验），接入 `session show` / `session usage` / `handoff export` 三入口 | 新增单元测试三条：深链/幂等/空白、裸 UUID、其他形态原样通过 |
| S3 | cli_contract.rs 补 `not_found_deep_link_and_bare_uuid_still_fail_cleanly`；SKILL.md 升 v3 后同步两处版本断言 | `cargo test --test cli_contract --test cli_taxonomy` 全绿 |
| S4 | 口径文档同步：AGENTS.md CLI 一节、SKILL.md 第 32 行 + `version: 3` | `skills_doc_commands_resolve_in_clap`（taxonomy 内）通过；复读两处口径一致 |
| S5 | 全量回归 + 真机复验（真实会话深链三命令 + not-found 退出码） | `cargo test --manifest-path src-tauri/Cargo.toml` 全绿；真机命令输出 ok:true / exit 1 |

## 风险

| 风险 | 缓解 |
| --- | --- |
| `codex_session_key` 对「任何以 UUID 结尾的串」放行，把别家 URL 吸进 codex 命名空间 | helper 先限定三种明确形态再交形状校验；单元测试含 `vscode://threads/<uuid>` 反例 |
| SKILL.md 版本升级牵动契约测试硬编码 | S3 同步改断言，跑契约测试确认 |
| 裸 UUID 与其他 provider 的 UUID 撞形 | 各 provider UUID 空间独立，误命中概率可忽略；不命中照常 session_not_found，验收条件已写明口径 |
