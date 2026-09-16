---
schema_version: "1"
protocol: "1.3.0"
artifact: "review"
artifact_id: "ar_69Y6NNZY913M9KY5TSBJDZNB46"
work_item_id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
created_at: "2026-09-16T07:17:17Z"
producer: "aes-review"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:32abb9b77345e1c4412d7442f9ae353d1bca154ec66037eeaf403b276872b2ea
  artifacts:
    - artifact_id: "ar_17QYMV7XZVJT2YMZ97TDYEPB8T"
      digest: sha256:eaf59cad40d573697ce8a9b1016ce324caca359c8f482ca58cce821b1de91ab5
      locator: "implementation.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "de5a9813378a676dde2ffd2ad67c8ad5d995eda4"
    revision: "de5a9813378a676dde2ffd2ad67c8ad5d995eda4"
    tree: "598fe1d1a4da0bb3667ef0b1a50ad98502bb91bb"
    content_digest: "sha256:71cae4bb63af7db87e355b64ce3d0fa1f62d65a7781abd21f5d487949f26a920"
    branch_or_pr: "feature/agent-cli"
    workflow_excluded: true
verdict: "approved"
review_type: "code"
reviewers: []
---
# 代码评审

审查范围：`feature/agent-cli` 上 3e5ed2c..de5a981 的六个提交（S2-S8）。本版覆盖原评审（范围并入 skill 分发后按协议重审），逐文件读了 diff 与关键实现，核对设计与 AC-001..008。

## 审查过的重点

- 隐私边界：LIST_KEYS 白名单只放行卡片级字段；契约测试断言列表项无 `lastUserMessage` / `lastAiMessage`。`--content` 分级与设计一致。skill.rs 不接触任何会话数据。
- 发布合同：Bridge ID、App 标识符、VSIX 命名未动；skill 分发文档内嵌二进制，发布 zip 不新增文件，打包脚本无新改动点。
- 插件硬规则：CLI 路径无插件调用、无 shell 拼接、无宿主路径硬编码新增；skill 候选目录是用户主目录下的公开约定，不属插件范畴。
- 写盘边界：skill install/remove 只写候选根下的 `agentwatcher/SKILL.md` 或 `--dir` 指定目录；remove 删空目录但不递归删别的；契约测试全程经临时目录，不触碰真实宿主。
- GUI 兼容：lib.rs 改动全是可见性、一个新 pub 函数、默认关闭的原子开关；GUI 测试全绿。
- 无副作用：CLI 禁用 codex app-server（文件兜底）加清 stdio 继承标志；skill 大类是声明过的显式写盘例外，README/AGENTS 已写明。

## 非阻断问题（记录在案）

1. `find_session` 用默认扫描配额（每 provider 80 条）。`--limit 200` 看到的第 150 条会话再 show 会报 `session_not_found`。与 GUI 配额语义一致，非回归；后续版本可做按 ID 直查数据源。
2. skill 候选宿主表硬编码五个目录；新宿主要改 `skill.rs`。`--dir` 可兜底。
3. `session usage` 的 human 输出在 usage 为 null 时输出空行；human 表格排版无快照测试。
4. SKILL.md 教 AI「不在 PATH 时向用户要安装目录」，没做 exe 自动发现（注册表/常见位置探测）。v1 可接受，装 PATH 属用户环境问题。

## 结论

approved。四条非阻断记录如上，无阻断问题。
