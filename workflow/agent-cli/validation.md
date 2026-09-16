---
schema_version: "1"
protocol: "1.3.0"
artifact: "validation"
artifact_id: "ar_29WBZKFYZZPJXMYSQ8VF3JQ70E"
work_item_id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
created_at: "2026-09-16T07:17:17Z"
producer: "aes-validate"
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
outcome: "passed"
acceptance: []
---
# 验收

逐条对照 work-item 的 AC-001 到 AC-008（AC-008 为 2026-09-16 补充拍板新增，本版为合同变更后的重跑）。

- AC-001: 通过。方法：cargo build 产出 debug 版 agentwatcher-cli.exe；命令只读数据源、不连 GUI 进程。证据：构建输出与契约测试（真实二进制、无 GUI 依赖路径）。
- AC-002: 通过。方法：分类学测试锁 session / handoff / skill 三个大类、动词、必填参数、全局参数、帮助文本。证据：cargo test --test cli_taxonomy 8 项通过。
- AC-003: 通过。方法：契约测试断言信封五键、command 字段、ok、错误码、退出码 0/1、human 不吐信封。证据：cargo test --test cli_contract 8 项通过。
- AC-004: 通过。方法：契约测试断言列表项不含 lastUserMessage / lastAiMessage；实现侧 LIST_KEYS 白名单兜底。证据：json_session_list_emits_single_envelope。
- AC-005: 通过。方法：真实会话执行 handoff export，退出码 0，primarySourceFile 落在系统临时目录的 AgentWatcher\handoff-sources\zcode\ 下、文件名自然键加时间戳（344KB、256 条消息），与 GUI 同一函数。证据：S7 轮命令输出；复测方法在人工清单。
- AC-006: 通过。方法：cargo test 全量。证据：lib 91 过 1 忽略（同基线）加 分类学 8 加 契约 8，0 失败。
- AC-007: 通过。方法：通读 README/AGENTS diff，命令面（含 skill）、信封、退出码、内容分级与实现逐一对照一致。证据：提交 ce57bf2 与 de5a981。
- AC-008: 通过。方法：契约测试经 --dir 打临时目录锁完整生命周期（list 报 host-absent、install 落盘带 name 头、installed、篡改后报 outdated、重装恢复、remove 幂等带 missing 报告）；裸 skill list 断言状态词表。自动化测试未触碰真实宿主目录。证据：skill_lifecycle_in_explicit_dir_is_idempotent、skill_list_without_dir_reports_host_candidates。

Verify: case:json_session_list_emits_single_envelope
Verify: case:json_session_show_unknown_id_fails_in_envelope
Verify: case:skill_lifecycle_in_explicit_dir_is_idempotent
Verify: case:skill_list_without_dir_reports_host_candidates
Verify: manual:跑 cargo build --manifest-path src-tauri/Cargo.toml --bin agentwatcher-cli 确认产物存在
Verify: manual:用真实会话 ID 跑一次 handoff export 核对落盘路径
Verify: manual:打开 README 的「AI 命令行」一节对照终端实际命令输出
