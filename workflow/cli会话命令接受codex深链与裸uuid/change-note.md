---
schema_version: "1"
protocol: "1.3.0"
artifact: "change-note"
artifact_id: "ar_0SZ7WSZSNXHXS9TJCQEG17RBM2"
work_item_id: "wi_7FY75QZZZZ3Z0YTX8PZVK8N0BN"
created_at: "2026-11-11T07:51:51Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:ac9ef900a601c9ec2c7772f8a1df16791d2d07a04de86d850b7c7a89ea08446b
  artifacts:
    - artifact_id: "ar_6QYZQVF0FAQG1TCGWGGYAB6GPD"
      digest: sha256:5c9f5615df963886150e364a1f62cd3719c7872693574d4929e50f6e2d53b2dc
      locator: "plan.md"
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
# 修改说明

全部改动在施工提交 `f439e31`（5 文件，+114/−11），理由与来源一行一条：

| 改哪里 | 为什么改 | 依据 |
| --- | --- | --- |
| `src-tauri/src/bin/agentwatcher-cli/commands.rs`：新增 `normalize_lookup_id` 并接入 `session show` / `session usage` / `handoff export` 三入口 | 深链 `codex://threads/<uuid>` 与裸 UUID 此前被误报 `session_not_found`（`find_session` 拿原始串精确匹配、全历史兜底把深链剥成 `//threads/…`），与 suggest 链路口径不一致 | 用户拍板的调查结论（ZCode 会话 `sess_44b48522-…` 里 agent 被拒实录 + 源码链路核对） |
| `src-tauri/src/lib.rs`：`codex_session_key` 改 `pub` | CLI 复用同一套三形态形状校验，不另写一份 | 同上；函数逻辑一字未动，lib 既有测试仍覆盖 |
| `src-tauri/tests/cli_contract.rs`：新增 `not_found_deep_link_and_bare_uuid_still_fail_cleanly`，`version: 2`/`docVersion=="2"` 两处断言改 3 | 锁住「形态可解析但会话不存在仍走信封报错、退出码 1」；SKILL.md 升版后断言必须跟着走 | 验收条件 AC-004；AGENTS.md「改文档必须同步契约测试」口径 |
| `src-tauri/src/bin/agentwatcher-cli/skill/SKILL.md`：第 32 行口径改写、`version: 2`→`3` | 行为变了，宿主技能副本要能感知版本差异 | AGENTS.md skill 分发规则（版本号取 frontmatter，`skill list` 才报 outdated） |
| `AGENTS.md`：CLI 一节补「所有按 ID 查询的命令接受深链与裸 UUID」 | 指令文件与实现口径一致 | AGENTS.md 自身「对应 README/TODO/AGENTS 与实现注释保持口径一致」 |

## 例外和还没解释的改动

无。diff 里每一行都对应上表某一行；没有夹带清理、没有格式化无关代码。
