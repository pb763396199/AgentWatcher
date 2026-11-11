---
schema_version: "1"
protocol: "1.3.0"
artifact: "review"
artifact_id: "ar_2BNZFV5ZFQH5W3039QB8WCEE8A"
work_item_id: "wi_01M71Q3R0W0004MNNQHHS0ZSZ1"
created_at: "2026-11-10T04:06:01Z"
producer: "aes-review"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:08218fcaad2d9ec21a92a9335bb0cea91f075df699314c2971d7d17dfd158939
  artifacts:
    - artifact_id: "ar_15QZ5SZWH6YBJ6AKRSHHY9CTRN"
      digest: sha256:f6a4d9663c90e90bf03a3055d0fc934a84ea8f577876787b0c45dacf7001779a
      locator: "implementation.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "34600a6a84017e628673841d74ef71c744af8dbb"
    revision: "69e674cdbb3ae7e9fbc1f8235285da8cf5ad2cdf"
    tree: "d296ab2e610c0b1f557befa5a08c9651feba3e9f"
    content_digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
    branch_or_pr: "feature/Codex-生图-CLI-命令"
    workflow_excluded: true
verdict: "superseded"
review_type: "code"
reviewers: []
---
# 评审

审查范围：feature/Codex-生图-CLI-命令 上 c1d1b4a（代码）与 c0efe38（文档）两个提交，7 个文件 891 行新增。从五条验收标准倒推关键行为逐条核对实现与测试。

## 阻断问题

### R1（中）：401 特征串过宽会误杀正常生图

`src-tauri/src/lib.rs` 的 `run_codex_exec_with_auth_watch` 里 `saw_unauthorized` 用 `buffer.contains("401")` 判定登录失败。事件流里任何 "401" 字样都会触发：用户提示词是"画一张 HTTP 401 错误页"时，模型回复（agent_message 事件）大概率包含 "401"，命令会被误判成 `codex_auth_failed` 并 kill 正在进行的生图。实测的未登录输出形态是 `unexpected status 401 Unauthorized`，特征可以收紧到 `401 Unauthorized`（大小写不敏感已在 contains("unauthorized") 分支覆盖，两处判定同步收紧）。最小改法：把 stdout/stderr 监视与退出分类里的裸 `"401"` 全部换成 `"401 Unauthorized"`。

## 不阻断的提示

- `--out` 指向已存在文件时 `std::fs::copy` 会直接覆盖。CLI 是显式工具，用户点名路径即意图写入，可接受；AGENTS.md 口径未提，后续有人问再补。
- 产物定位对事件流字段（`thread.started`）和目录名（`generated_images`）有依赖，Codex 云端变更时失败有分类不静默，风险已知并写入实现记录。

## 核对过的关键行为

| 验收标准 | 实现位置 | 结论 |
| --- | --- | --- |
| AC-001 真实生图 | lib `generate_image_via_codex` + 实测 47.5 s 出图 | 过 |
| AC-002 双来源 | `system_imagegen_skill_path` 分流 + 实测两种来源出图 | 过 |
| AC-003 无模型绑定 | `imagegen_exec_args` 无 -m；单测 + 帮助文本断言 | 过 |
| AC-004 失败分类 | 四类错误码实测三种（R1 修后复核第四种） | R1 修后过 |
| AC-005 测试全过 | 166+5+12+19 全绿 | 过 |

结论：changes_requested，阻断项 R1 一条，修法一处字符串收紧。

<!-- 2026-11-11 指针维护：rebase（amend）洗掉原锚定提交，代码净补丁经 git patch-id 比对与审查版一致，锚点行按协议 rebase 重锚教义机械改写到重放后的最后代码提交。证据正文未改动。 -->
