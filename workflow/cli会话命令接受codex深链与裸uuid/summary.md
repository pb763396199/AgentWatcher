---
schema_version: "1"
protocol: "1.3.0"
artifact: summary
artifact_id: "ar_01M74RT4N80006J2G3Z8QW1R8X"
work_item_id: "wi_7FY75QZZZZ3Z0YTX8PZVK8N0BN"
created_at: "2026-11-11T07:45:00Z"
producer: "aes-execute"
result: "complete"
scope: round
dependencies:
  work_item_contract_digest: sha256:ac9ef900a601c9ec2c7772f8a1df16791d2d07a04de86d850b7c7a89ea08446b
  artifacts: []
---
# 第 1 轮成果总结

## 结论

命令行的三个按 ID 查询命令现在认得 Codex 桌面端的深链和裸 UUID 了，粘过来直接命中，不用再先跑 session list 换形态。等人的只有一件事：装上新版 CLI 后把各宿主的技能副本升级一下（`agentwatcher-cli skill install`）。

## 你现在能做什么

- 拿到 `codex://threads/<uuid>` 深链可以直接 `session show` / `session usage` / `handoff export`，和 `codex:<uuid>` 命中同一会话。
- 粘一个裸 UUID 也行，一律按 Codex 会话解释。
- 其他 agent（ZCode、Claude 等）的会话 ID 行为不变，仍是精确匹配。

## 改了什么

| 变更 | 落在哪 | 为什么 | 锚点 |
| --- | --- | --- | --- |
| 查询入口 ID 归一化 helper | `src-tauri/src/bin/agentwatcher-cli/commands.rs` | 深链/裸 UUID 与 suggest 同口径，不再误报找不到 | f439e31 |
| `codex_session_key` 改 pub | `src-tauri/src/lib.rs` | CLI 复用同一套形状校验，不另写一份 | f439e31 |
| 单元测试 3 条 + 契约测试 1 条 | `src-tauri/tests/cli_contract.rs`、commands.rs 内嵌 | 锁住三形态命中与 not-found 退出码 | f439e31 |
| 技能文档升 v3、AGENTS.md 口径同步 | `src-tauri/src/bin/agentwatcher-cli/skill/SKILL.md`、`AGENTS.md` | 文档跟着行为走，宿主可感知版本差异 | f439e31 |

## 还欠什么

主动不做：GUI 卡片点击的 ZCode 跳转仍不实现（另一个拍板）；其他 provider 不引入深链/裸键形态。没做完的：无——五条验收全部有测试或真机证据，人工核对清单为零条目（理由见 `manual-test.md`）。
