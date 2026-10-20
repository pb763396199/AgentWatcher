---
schema_version: 1
protocol: 1.3.0
artifact: review
artifact_id: ar_01M5C9MDB70003MMYATTCQP84Z
work_item_id: wi_01M5C61SH5000FME6B0YSBBTDE
created_at: 2026-10-20T10:10:00Z
producer: aes-review
verdict: approved
supersedes: [ar_01M5C8HPPC0003DFQ2NPFSM165]
contract_revision: 1
verified_at_commit: 141fb7659875fa25e8fd4bc6aa8e6b437ff8c005
review_type: code
reviewers: []
dependencies:
  work_item_contract_digest: sha256:66c5eb385a5e74388b6cd2c4a87f41f8d92e69603379b6bb23da5196597a30de
  artifacts: []
---

# 代码评审：CLI 全历史与归档会话搜索（覆盖第二轮增量）

**本评审由写这段代码的同一个 agent 做（自审），按流程在此声明。** 第一轮（提交 `39962ef`，9 文件 +674/−112）的评审结论沿用：通过，抓到并修掉了 zst 薄摘要缓存污染、扫描上限裕量被 clamp 吃掉两个实现问题。本版把锚点刷到第二轮提交 `141fb76`，并对增量做补充评审。

## 第二轮增量评审（`39962ef..141fb76`）

| 位置 | 问题 | 严重度 | 处理 |
| --- | --- | --- | --- |
| commands.rs `LIST_KEYS` `updatedAtMs`→`updatedMs` | 补上的是卡片级元数据（绝对时间戳），无正文泄漏；`cli_contract` 的"列表无正文键"断言重跑通过 | 无阻断 | 通过 |
| skill/SKILL.md 时间口径与 data 结构说明 | 纯文档；`include_str!` 内嵌，重建后经 `skill install` 落到宿主目录，实测刷新成功 | 无阻断 | 通过 |

增量无阻断问题。

## 第一轮已核实的事实（结论不变）

- `resolve_scan_options` 对 `None`/非 0 入参的输出与改动前逐字段一致；`clamp(1,30)` 与 `clamp(1,5000)` 语义保留为命名常量。
- GUI 不经 `lookup_session_full_history`；`session_watch_roots` 未改，GUI 默认不监视归档目录。
- 隐私边界：`session list` 仍无 `lastUserMessage`/`lastAiMessage`；新增的 `updatedMs` 是元数据。
- zst 解压副本沿用 handoff-sources 临时目录命名空间，基名经 `file_name()` 约束，无路径注入面。
- ruzstd 0.9.1 纯 Rust、decode-only，无 build.rs C 编译。
- 第一轮评审发现的 `LIST_KEYS` 键名 quirk（`updatedAtMs` 从未匹配）在第二轮顺带修复——当时列为非阻断未动，委托人测证明它实际卡住真实用法，修复理由充分。

## 非阻断问题

- 全历史列表里 zst 会话与预算外 SQLite 会话的预览文本为空（设计取舍，定向 show 补全）。
- 未跑 `npm run test:tauri:*`（无 UI/窗口改动）；发布时按发布规范补跑。

## 没覆盖的范围

- 非 Windows 路径的 zst 行为（产品只在 Windows 下支持会话扫描）。
