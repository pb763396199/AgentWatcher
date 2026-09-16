---
schema_version: "1"
protocol: "1.3.0"
artifact: "change-note"
artifact_id: "ar_6QZQXNZXZKAB0CDMCJKYF5N9EY"
work_item_id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
created_at: "2026-09-16T07:17:16Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:32abb9b77345e1c4412d7442f9ae353d1bca154ec66037eeaf403b276872b2ea
  artifacts:
    - artifact_id: "ar_27YPYVZFZXMGQYQDTCJTED297J"
      digest: sha256:9af41749bdab6e2a224286695252b0cd82884d033962f6a62a56c077cadd6620
      locator: "plan.md"
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
result: "complete"
---
# 修改说明

## 覆盖账本

| 改动 | 提交 | 依据 |
| --- | --- | --- |
| lib.rs 开放五个类型与三个入口（含命令壳拆分） | 979d377 | 计划第 2 步 |
| .gitignore 的 bin/ 规则改根锚定；[[bin]] 与 clap；cli.rs；分类学测试 | 5da2dcd | 计划第 3 步（gitignore 为施工中发现） |
| 信封层、四命令、契约测试；codex app-server 禁用开关与 stdio 继承标志清除 | cc9aa64 | 计划第 4 步（后半为执行中发现的缺陷修复） |
| package-exe.mjs 增量构建并拷贝 CLI exe | e3682c8 | 计划第 5 步 |
| README / AGENTS 口径同步 | ce57bf2 | 计划第 6 步 |
| skill install/list/remove、内嵌 SKILL.md、测试与文档 | de5a981 | 计划第 8 步（2026-09-16 补充拍板并入） |

## 修改理由

- 开放 API：CLI 是独立 bin，必须经 pub 入口复用扫描逻辑；`get_session_usage_detail` 带 `#[tauri::command]` 宏，宏生成的包装与 `pub` 冲突（E0255），拆成命令壳加 `session_usage_detail` 实现函数，行为零变化。
- codex app-server 开关：扫描 Codex 会话会拉起常驻 app-server，其 cmd→node→codex 进程链继承 stdout 管道写端，管道读者永远等不到 EOF，且每次调用泄漏一条孤儿进程链。CLI 启动即禁用，Codex 走 `~/.codex/sessions` 文件兜底（GUI 在 server 启动失败时的既有路径）。GUI 行为不变。
- stdio 继承标志清除：防御性修复，CLI 将来再 spawn 任何子进程都不会攥住输出管道。
- skill 分发：AI 助手是 CLI 首要消费者（访谈拍板），没有 skill 就要用户手动喂用法。文档内嵌二进制（include_str!）避免发布包多文件；重复 install 覆盖升级解决跨版本同步；候选宿主只装目录已存在的，避免给没装的宿主造垃圾目录。
- .gitignore 根锚定：仓库根 `bin/` 通配规则是 .NET 模板遗留，误伤 `src-tauri/src/bin/`。

## 代码范围

- `src-tauri/src/lib.rs`：可见性（5 类型 + 8 字段 + 2 函数）、命令壳拆分、app-server 禁用开关。无逻辑改动。
- `src-tauri/src/bin/agentwatcher-cli/`：main / cli / commands / output / skill 五个文件加 skill/SKILL.md。
- `src-tauri/tests/cli_taxonomy.rs`（8 测试）、`src-tauri/tests/cli_contract.rs`（8 测试）。
- `src-tauri/Cargo.toml`、`.gitignore`、`scripts/package-exe.mjs`、`README.md`、`AGENTS.md`。

## 未解释或例外改动

无。测试期间清理过 29 条本机孤儿 codex app-server 进程链（含 GUI dev 热重载历史遗留），不涉及仓库文件。
