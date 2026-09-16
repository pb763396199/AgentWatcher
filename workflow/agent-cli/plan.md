---
schema_version: "1"
protocol: "1.3.0"
artifact: "plan"
artifact_id: "ar_27YPYVZFZXMGQYQDTCJTED297J"
work_item_id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
created_at: "2026-09-16T03:25:38Z"
producer: "aes-plan"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:32abb9b77345e1c4412d7442f9ae353d1bca154ec66037eeaf403b276872b2ea
  artifacts:
    - artifact_id: "ar_35YPMQD4YZVPHVKV26G13Q0D5T"
      digest: sha256:ed2c17d64e20015f62e1adcc2e907ed33a8484b24599f5d1c8ae387848c09534
      locator: "design.md"
result: "ready"
---
# 计划：AgentWatcher AI 操作 CLI

设计依据：`design.md`（ar_35YPMQD4YZVPHVKV26G13Q0D5T，2026-09-16 accepted）。全部改动在 `feature/agent-cli` 分支，回退就是丢弃分支，没有共用文件风险，步骤串行执行。

## 第 1 步：记录基线

- 做什么：跑 `cargo test --manifest-path src-tauri/Cargo.toml`，记录通过数和当前 HEAD。
- 验证：全绿才继续。
- 证明：实现记录里贴测试摘要和 HEAD 哈希。

## 第 2 步：lib.rs 开放最小 API（纯可见性，无行为变化）

- 改哪些：`src-tauri/src/lib.rs` 加 `pub`——类型：`AgentSession`（97 行）、`ScanOptions`（349 行，字段也加 `pub`，CLI 要构造）、`HandoffSourceContextRequest`（264 行，字段同）、`HandoffSourceContext`（275 行）、`SessionUsageDetail`（173 行）；函数：`scan_sessions_blocking`（942 行）、`get_session_usage_detail`（1798 行）、`prepare_handoff_source_context_blocking`（2335 行）。
- 不动：这五个类型以外的结构、全部函数体、`generate_handler` 注册表。
- 行为：无变化，GUI 与既有测试不受影响。
- 验证：`cargo build --manifest-path src-tauri/Cargo.toml` 不新增警告（重点看 `private_interfaces`）；`cargo test` 仍全绿。
- 证明：警告数前后对比。

## 第 3 步：bin crate 骨架 + 分类学测试

- 改哪些：
  - `src-tauri/Cargo.toml`：`[dependencies]` 加 `clap = { version = "4.5", features = ["derive"] }`；加 `[[bin]] name = "agentwatcher-cli" path = "src/bin/agentwatcher-cli/main.rs"`。
  - 新文件 `src-tauri/src/bin/agentwatcher-cli/cli.rs`：纯 clap 定义，不依赖其他模块。顶层 `Cli { format: OutputFormat（global，默认 json）, version, command: Commands }`；`Commands::Session(SessionAction)`、`Commands::Handoff(HandoffAction)`；`SessionAction::List { provider: Vec<String>, status: Option<String>, workspace: Option<String>, limit: Option<usize>, active_days: Option<u64> }`、`SessionAction::Show { id: String, content: bool }`、`SessionAction::Usage { id: String }`；`HandoffAction::Export { id: String }`。每个子命令的 about 用中文一句话。
  - 新文件 `src-tauri/src/bin/agentwatcher-cli/main.rs`：本步只做解析和空分发（编译通过即可，实现在第 4 步）。
- 新测试 `src-tauri/tests/cli_taxonomy.rs`：用 `#[path = "../src/bin/agentwatcher-cli/cli.rs"] mod cli;` 引入命令定义。断言：`session list --status waiting` 解析成功且 status 为 waiting；`session show`（缺 id）解析失败；`session list --format json` 全局参数生效且默认值是 json；`handoff export x` 解析成功；`session list --provider a --provider b` 多值收集成功；未知动词 `session fly` 解析失败；全部子命令 about 非空。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml --test cli_taxonomy` 全绿。
- 证明：测试输出。

## 第 4 步：命令实现 + 输出契约测试

- 新文件 `src-tauri/src/bin/agentwatcher-cli/output.rs`：`Envelope { command, ok, data, error, messages }`（serde camelCase 序列化，`error` 形如 `{ code, message }`）；全局格式状态（`OnceLock<OutputFormat>`）；`emit_success(command, data)`、`emit_failure(command, code, message)`——json 模式 stdout 只打一个文档，失败后 `exit(1)`；human 模式交给命令自渲染。用法错误由 clap 自己处理（stderr + 退出码 2），不进信封，与设计一致。
- 新文件 `src-tauri/src/bin/agentwatcher-cli/commands.rs`，四个命令：
  - `session list`：把筛选参数拼成 `ScanOptions`（`--provider` 给了就把六个 `include_*` 设成「命中才 true」；`--limit` → `max_sessions`；`--active-days` → `active_window_days`）调 `scan_sessions_blocking(Some(opts))`；status、workspace 在结果上后置过滤（workspace 与 `workspace_path` 忽略大小写整串相等）。每个 `AgentSession` 序列化成 JSON 值后**只保留**这些键进列表：`id, provider, providerLabel, title, workspaceLabel, workspacePath, status, timeLabel, updatedAtMs, messageCount, branch, usage`——`lastUserMessage, lastAiMessage, lastUserMessageTruncated, lastAiMessageTruncated, lastAiMessageExcerptKind, sessionPath, sessionResource, todoIds, pluginWorkflow, workspaceKey, workspaceName, workspaceGroup, workspaceDiscriminator, workspace` 一律不出现在 list（AC-004）。信封 `data = { sessions: [...], total: n }`。
  - `session show <id>`：扫描后按 id 找；找到则 `data = { session: <完整 AgentSession 序列化> }`，带 `--content` 时追加 `data.content`：provider 为 `opencode`/`zcode` 时调 `prepare_handoff_source_context_blocking`，取 `primarySourceFile`（完整 transcript Markdown 的落盘路径）和 `inlineSummary`；其余 provider 给 `data.content.rawSessionPath = session.sessionPath`。找不到时 `emit_failure("session", "session_not_found", ...)` 退出码 1。
  - `session usage <id>`：调 `get_session_usage_detail(id)`，`data = <SessionUsageDetail 序列化>`（`usage`、`topTools`）。
  - `handoff export <id>`：扫描按 id 找（拿 provider、sessionPath、sessionResource、title）→ 构造 `HandoffSourceContextRequest` → 调 `prepare_handoff_source_context_blocking` → `data = <HandoffSourceContext 序列化>`（`primarySourceFile` 对 OpenCode/ZCode 是新落的导出文件，对 JSONL provider 是原始会话文件）。找不到会话同上退出码 1。
  - human 模式：list 打固定列表格，show/usage 打键值行，export 打落盘路径。
- `main.rs` 补全分发：解析 → 设格式 → `match` 到命令 → 统一错误边界（命令返回 `Result`，`Err` 走 `emit_failure`）。信封 `command` 字段用稳定字符串 `"session list"` / `"session show"` / `"session usage"` / `"handoff export"`。
- 新测试 `src-tauri/tests/cli_contract.rs`：用 `env!("CARGO_BIN_EXE_agentwatcher-cli")` 起真实二进制。断言（结构断言为主，不依赖本机有没有真实会话）：
  1. `--format json session list`：stdout 恰好一个 JSON 文档；五键齐；`command == "session list"`；`ok == true`；`data.sessions` 是数组；sessions 非空时每个元素没有 `lastUserMessage`/`lastAiMessage` 键；退出码 0。
  2. `--format json session show nosuch:000000`：`ok == false`；`error.code == "session_not_found"`；退出码 1；stdout 仍恰一个 JSON 文档。
  3. `--format json session usage nosuch:000000`：同上。
  4. `--format human session list`：stdout 里不含 `"command"` 字样（human 模式不吐信封）。
  5. `--version`：退出码 0，输出非空。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml --test cli_contract` 全绿；`cargo test` 全量全绿。
- 证明：测试输出计数。

## 第 5 步：打包脚本带上新 exe

- 改哪些：`scripts/package-exe.mjs`，在拷贝 `AgentWatcher.exe` 进 `artifacts/AgentWatcher/` 的同一段逻辑里加拷贝 `src-tauri/target/release/agentwatcher-cli.exe` 到同目录（不做 rcedit 图标，v1 保持默认）。zip 打包逻辑按目录打，自动带上。
- 验证：`node --check scripts/package-exe.mjs`；diff 确认拷贝段与 AgentWatcher.exe 对称。
- 证明：`node --check` 退出码 0 + diff。完整 `npm run package:exe` 属于发版流程，本任务不跑（AC-001 只要求 `cargo build` 产物）。

## 第 6 步：文档口径同步

- 改哪些：
  - `AGENTS.md`：「目录结构」加 `src-tauri/src/bin/agentwatcher-cli/`；「怎么跑起来」加 CLI 构建/测试命令；「怎么测」加两类 CLI 测试；新增「AI 操作 CLI」小节（命令面、信封契约、正文按需输出边界、退出码）；修正 lib.rs 行数口径（写「4678 行左右」处改为不写死行数或写当前实数 13438）。
  - `README.md`：加 CLI 用法节：命令表、信封示例、`--content` 的内容分级说明、退出码表。
- 验证：通读 diff，命令名、字段名、退出码与实现逐一对照。
- 证明：diff。

## 第 7 步：全量验证

- 跑：`cargo test --manifest-path src-tauri/Cargo.toml`（全量）、`cargo build --manifest-path src-tauri/Cargo.toml --bin agentwatcher-cli`（AC-001 产物）、`node --check scripts/package-exe.mjs`、`git diff --check`。
- 证明：四条命令的退出码和关键输出，记进实现记录。

## 第 8 步：skill 分发命令（2026-09-16 补充拍板并入）

- 新文件 `src-tauri/src/bin/agentwatcher-cli/skill/SKILL.md`：教 AI 用 CLI 的技能文档，YAML 头带 name/description，正文含命令表、信封契约、内容分级、exe 定位兜底（不在 PATH 时向用户要安装目录）。以 `include_str!` 内嵌。
- 新文件 `src-tauri/src/bin/agentwatcher-cli/skill.rs`：候选宿主目录表（`~/.zcode/skills`、`~/.claude/skills`、`~/.agents/skills`、`~/.codex/skills`、`~/.config/opencode/skill`）、状态判定（内容逐字节比对）、install（幂等覆盖，只装存在的宿主；`--dir` 显式给则只装它，目录不存在就建）、list、remove（删 SKILL.md，目录空则一并删）。
- `cli.rs` 加 `Skill(SkillAction)` 组与三个动词的 `--dir` 可选参数；`main.rs` 接分发；信封命令名 `skill install` / `skill list` / `skill remove`。
- 测试：分类学补 skill 解析断言；契约测试经 `--dir` 打临时目录锁 install 幂等、文件头、list 状态、remove 幂等，不触碰真实宿主目录。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml` 全量绿。
- 证明：测试输出；真实宿主安装留给人工清单。

## 风险与回退

- lib.rs 只动可见性（第 2 步），出问题 `git revert` 单提交即可。
- 其余全是新文件和脚本小改，回退 = 丢弃对应提交。
- `agent-usage-metrics` worktree 遗留的未跟踪 `TODO.md`（`.aes-workflow/worktrees/agent-usage-metrics/TODO.md`）不属于本任务，不动。
