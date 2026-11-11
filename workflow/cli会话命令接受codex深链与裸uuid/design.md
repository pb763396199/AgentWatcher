---
schema_version: "1"
protocol: "1.3.0"
artifact: "design"
artifact_id: "ar_3ZXZXSFVEY1GC0Q2EYPAT8ZJ19"
work_item_id: "wi_7FY75QZZZZ3Z0YTX8PZVK8N0BN"
created_at: "2026-11-11T07:51:10Z"
producer: "aes-brainstorm"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:ac9ef900a601c9ec2c7772f8a1df16791d2d07a04de86d850b7c7a89ea08446b
  artifacts: []
result: "accepted"
---
# 设计：CLI 会话命令接受 Codex 深链与裸 UUID

## 要解决什么

`session show` / `session usage` / `handoff export` 只认 `session list` 给出的 `codex:<uuid>` 形态。调用方（人或 agent）拿 Codex 桌面端的深链 `codex://threads/<uuid>` 或裸 UUID 查询被误报 `session_not_found`，要自己再跑一轮 list 换形态；suggest 链路经 `codex_session_key` 已接受三种形态，同一 CLI 两套口径。本设计经用户拍板（调查结论当面给出后回复「修」）。

## 定了什么

- 归一化放 CLI 命令入口：三个命令共用的 `normalize_lookup_id`，先限定三种明确形态（`codex://threads/` 前缀、`codex:` 前缀、不含 `:` 与 `/` 的裸串），再交 `codex_session_key` 做 UUID 形状校验，收敛成 `codex:<uuid>` 后进 `find_session` 与 `session_usage_detail`。
- `codex_session_key` 从私有改 `pub`，逻辑不动。
- 裸 UUID 一律按 Codex 解释，不做多 provider 猜测。
- 文档同步：AGENTS.md CLI 一节、内嵌技能文档 SKILL.md（version 2→3）。

## 为什么这么定（方案对照）

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 归一化放 CLI 命令入口 | 三入口共用 helper，两条解析路都拿到规范形态 | lib 需把已有函数改 pub | 采用 |
| 归一化塞进 lib 的 `lookup_session_full_history` | 改兜底一处生效 | GUI `open_session` 是另一套请求结构，为 CLI 改 lib 兜底影响面大于收益 | 不采用 |
| 只改报错文案提示正确形态 | 不改行为 | 调用方仍要多跑一轮换形态，深链明明可无损换算 | 不采用（用户拍板直接修行为） |
| 只收深链、拒绝裸 UUID | 规避跨 provider 歧义 | suggest 已收裸 UUID，两套口径只解决一半；各 provider UUID 空间独立，误命中可忽略 | 不采用 |

## 放弃了什么

- 大写 UUID 折叠小写、深链尾斜杠等拼写变体不在本次范围（与 suggest 现口径一致，评审记录里登记为非阻断）。
- 其他 provider 的深链/裸键形态不做（无统一深链标准）。
- GUI `open_session` 不动（ZCode 跳转不实现是另一个拍板）。

## 影响面

`find_session` 的调用点就三个命令入口（grep 核对无遗漏）；`session list` 无 ID 参数不涉及；suggest 链路独立归一化不涉及。信封契约、内容分级、退出码语义全部保持。
