---
schema_version: "1"
protocol: "1.3.0"
artifact: "review"
artifact_id: "ar_7VPQPSF4ZZ9YH3H8TJZAZG0R6N"
work_item_id: "wi_7FY75QZZZZ3Z0YTX8PZVK8N0BN"
created_at: "2026-11-11T07:51:52Z"
producer: "aes-review"
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
verdict: "approved"
review_type: "code"
reviewers: []
---
# 代码评审：CLI 会话命令接受 Codex 深链与裸 UUID

**本评审由写这段代码的同一个 agent 做（自审），按流程在此声明。** 变更集 `f439e31`（5 文件，+114/−11），逐行读过。

## 抓到的问题

无阻断问题。两条非阻断观察：

| 问题 | 位置 | 后果 | 判断 |
| --- | --- | --- | --- |
| 大写十六进制 UUID 归一化后保持原大小写，与扫描产出的全小写 `session.id` 精确比对不上 | commands.rs `normalize_lookup_id` → `codex_session_key`（只验形状不折叠大小写） | 大写输入仍报找不到；真实 Codex 深链全小写，suggest 链路同一行为 | 非阻断；要改应连同 lib 一起折叠，另开一件事 |
| 深链尾带 `/` 形状校验不过，原样放行后报找不到 | 同上（`rsplit('/')` 取到空段） | 拼写变体不命中 | 非阻断 |

## 已核实的事实

- `find_session` 的调用点恰好是三个命令入口，无遗漏（grep 核对）；`session_usage` 第二条解析路 `session_usage_detail` 用的是归一化后的 ID。
- 形态限定挡住了别家 URL 与路径：Windows 盘符路径（含 `:`）、POSIX 路径（含 `/`）、`vscode://` 均原样放行，单元测试有对应断言（这条是初版实现被抓到后收紧的）。
- 错误信息保留用户原始输入（`{id}` 不是 `{lookup}`）。
- 信封契约与内容分级断言未动：list 无正文键、五键信封、退出码 1 全保留并有新断言。
- 文档口径：AGENTS.md 与 SKILL.md 同步为「所有按 ID 查询的命令接受深链与裸 UUID」；SKILL.md `version:` 2→3，契约测试两处版本断言同步。
- suggest 链路未动（其 `is_supported` 前缀检查与 lib 内部归一化原样）。

## 非阻断问题

见上表两条，均与 suggest 现口径一致，不阻塞合入。

## 没覆盖的范围

GUI `open_session` 与卡片跳转不在本次变更集；lib 侧 `lookup_session_full_history` 的深链兜底未改（按设计在 CLI 入口收口）；大小写折叠与尾斜杠变体未处理。
