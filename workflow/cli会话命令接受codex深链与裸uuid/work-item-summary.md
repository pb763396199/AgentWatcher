---
schema_version: "1"
protocol: "1.3.0"
artifact: "summary"
artifact_id: "ar_01M74SQ1DS0006ZJJ5EC3NY56J"
work_item_id: "wi_7FY75QZZZZ3Z0YTX8PZVK8N0BN"
created_at: "2026-11-11T08:15:00Z"
producer: "aes-finish"
result: "complete"
scope: "work_item"
dependencies:
  work_item_contract_digest: sha256:ac9ef900a601c9ec2c7772f8a1df16791d2d07a04de86d850b7c7a89ea08446b
  artifacts: []
---
# 任务成果总结

## 结论

命令行的三个按 ID 查询命令（show / usage / handoff export）现在认得 Codex 深链和裸 UUID，粘过来直接命中，不再误报找不到。等人核对的只有一件事：这份改动还在特性分支上，确认后合入 dev、装新版 CLI、跑一次 `agentwatcher-cli skill install` 升级宿主技能副本。

## 你现在能做什么

- 拿到 `codex://threads/<uuid>` 深链或裸 UUID，直接查会话、看用量、导接续上下文，与 `codex:<uuid>` 命中同一会话。
- 不存在的 UUID 照样明确报 `session_not_found`、退出码 1，不会被形态可解析而误判成功。
- 其他 agent（ZCode、Claude 等）的会话 ID 行为不变。

## 改了什么

| 变更 | 落在哪 | 为什么 | 锚点 |
| --- | --- | --- | --- |
| 查询入口 ID 归一化（三形态限定 + 形状校验） | `src-tauri/src/bin/agentwatcher-cli/commands.rs` | 深链/裸 UUID 与 suggest 同口径 | S2 |
| `codex_session_key` 改 pub | `src-tauri/src/lib.rs` | CLI 复用同一套校验 | S1 |
| 单元测试 3 条 + 契约测试 1 条 + 版本断言同步 | `src-tauri/tests/cli_contract.rs`、commands.rs | 锁行为与退出码 | S2/S3 |
| 技能文档升 v3、AGENTS.md 口径同步 | `src-tauri/src/bin/agentwatcher-cli/skill/SKILL.md`、`AGENTS.md` | 文档跟行为走 | S4 |

## 每一轮的去向

| 轮次 | 去向 |
| --- | --- |
| 第 1 轮（唯一一轮） | 沿用：一次施工一次通过，评审 approved、五条验收全 passed |

## 还欠什么

主动不做：大写 UUID 与尾斜杠深链变体不折叠（与 suggest 现口径一致，评审记录登记为非阻断）；GUI 跳转、其他 provider 深链不在范围。没做完的：无。人工核对清单零条目（全部现象可由命令输出判定，理由写在 [manual-test.md](F:\AiProject\AgentWatcher\.aes-workflow\worktrees\cli会话命令接受codex深链与裸uuid\workflow\cli会话命令接受codex深链与裸uuid\manual-test.md)），任务停在 in_review 等你确认后合入。
