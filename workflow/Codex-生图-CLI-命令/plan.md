---
schema_version: 1
protocol: 1.3.0
artifact: plan
artifact_id: ar_01M71QFY8X000C5SGDAPGSYSF9
work_item_id: wi_01M71Q3R0W0004MNNQHHS0ZSZ1
created_at: 2026-11-10T11:19:42+08:00
producer: aes-plan
result: ready
supersedes: null
dependencies:
  work_item_contract_digest: sha256:08218fcaad2d9ec21a92a9335bb0cea91f075df699314c2971d7d17dfd158939
  artifacts:
    - artifact_id: ar_01M71QRR9M000EGGH4EF73F7DT
      digest: sha256:97d8260fbc9edeb063f2e0be24c43700f7cc009aa5c4c6b8b8ce8748b44e5d16
      locator: design.md
---

# 施工计划：Codex 生图 CLI 命令

## 背景事实（2026-11-10 实测，写计划时已确认）

- `codex exec --json --ephemeral --skip-git-repo-check -s workspace-write -C <scratch> -`（prompt 走 stdin）可触发内置 `image_gen` 工具，不需要 `OPENAI_API_KEY`。
- 事件流第一条是 `{"type":"thread.started","thread_id":"<uuid>"}`，产物落 `%USERPROFILE%\.codex\generated_images\<thread_id>\exec-<uuid>.png`。
- 编排模型会自己 `Get-Content %USERPROFILE%\.codex\skills\.system\imagegen\SKILL.md`：系统 skill 在，就不需要 CLI 注入任何说明。这条实测决定双来源的分工：系统 skill 在就靠 Codex 自读，不在才由 CLI 注入内嵌固化副本。
- 生图与编排模型（gpt 6/6.1 系列）无关：编排模型只负责读 skill、组 prompt、调工具，生图模型由 Codex 云端自动选择，客户端拿不到也不需要指定。

## 步骤

### 步骤 0：补三个实测点，结论写进实现回执

用到什么：本机 codex 0.167.0、`%TEMP%` 空目录。

| 验证点 | 命令（在空的 `%TEMP%\awf-imgtest2` 下） | 通过判据 |
| --- | --- | --- |
| read-only 沙箱下工具是否可用 | `printf '...' \| codex exec --json --ephemeral --skip-git-repo-check -s read-only -C <scratch> -` | `generated_images/<thread>/` 出现 PNG 则可用 read-only；否则实现固定 workspace-write |
| 未登录态的失败形态 | `CODEX_HOME=%TEMP%\empty-codex codex exec ...` | 记录退出码与 stderr 首行，决定错误码能否区分 `codex_auth_failed` |
| CODEX_HOME 是否影响产物目录 | `CODEX_HOME=%TEMP%\fake-codex codex exec ...`（登录态照常） | 看产物落 fake-codex 还是默认 `~/.codex`，实现按结果定位产物 |

改哪些文件：不改代码，结论记录在 `implementation.md` 的「哪里没按计划走」之前的实测节。

### 步骤 1：固化副本进仓库 + 双来源选择函数（先写测试）

改哪些文件：

- 新增 `src-tauri/src/bin/agentwatcher-cli/imagegen/SKILL.md`：从 `%USERPROFILE%\.codex\skills\.system\imagegen\SKILL.md`（2026-11-10 版）逐字节复制，不改写。这是"从当前电脑最新版固化一份"的正本。
- 新增 `src-tauri/src/bin/agentwatcher-cli/imagegen.rs`：
  - `const EMBEDDED_IMAGEGEN_SKILL: &str = include_str!("imagegen/SKILL.md");`
  - `enum ImagegenPromptSource { SystemSkill, Embedded }`
  - `fn system_imagegen_skill_path() -> Option<PathBuf>`：`CODEX_HOME` 环境变量优先，否则 `%USERPROFILE%\.codex`，拼 `skills\.system\imagegen\SKILL.md`，存在返回路径。
  - `fn build_imagegen_prompt(user_prompt: &str, source: ImagegenPromptSource) -> String`：SystemSkill 时输出短指令（要求按 imagegen skill 用内置 image_gen 工具生成、产物留在默认位置、完成后只回一行产物路径）；Embedded 时同样指令前附上 `EMBEDDED_IMAGEGEN_SKILL` 全文。纯函数。
- 测试（`src-tauri/src/lib.rs` 末尾或 imagegen.rs 内 `#[cfg(test)]`，放 CLI 侧则加在 commands 测试附近）：
  - `build_imagegen_prompt` 两种来源的输出分别包含用户提示词；Embedded 版包含 `imagegen` 字样与固化正文片段。
  - `system_imagegen_skill_path` 在 `CODEX_HOME` 指向含该文件的临时目录时返回 Some，指向不存在路径时返回 None（用 `std::env::set_var` + 串行测试，参照仓库既有环境变量测试写法；跑完恢复）。

怎么验证：`cargo test --manifest-path src-tauri/Cargo.toml imagegen`，预期新增 3 条测试全绿。

### 步骤 2：lib.rs 生图入口（跑 codex exec、定位产物、复制）

改哪些文件：`src-tauri/src/lib.rs`。

新增 pub 入口（对齐 `generate_suggestion_for_session` 的复用模式，CLI 经 lib crate 调用）：

```rust
pub struct CodexImagegenOutcome {
    pub output_path: PathBuf,       // 最终复制到的路径
    pub source_path: PathBuf,       // generated_images 下的原始产物
    pub prompt_source: String,      // "system-skill" | "embedded"
    pub thread_id: String,
    pub elapsed_ms: u64,
}
pub enum CodexImagegenError {
    CodexUnavailable,        // codex CLI 或 node 找不到
    Timeout { secs: u64 },
    GenerationFailed { detail: String },  // 退出码非 0 / 无产物 / 未登录（detail 带分类线索）
}
pub fn generate_image_via_codex(
    user_prompt: &str,
    out_path: Option<&Path>,
    timeout_secs: Option<u64>,
    scratch: &Path,
) -> Result<CodexImagegenOutcome, CodexImagegenError>
```

行为：

- 参数组装参照 `codex_exec_args`（lib.rs:16478）另写 `imagegen_exec_args`：保留 `--json --ephemeral --skip-git-repo-check -C <scratch>`，沙箱档按步骤 0 结论选 `read-only` 或 `workspace-write`，`-o` 不需要（产物是图片不走文本输出文件），末尾 `-` stdin 读 prompt。不传 `-m`、不传 effort——生图模型由云端定，实现里不出现任何模型名（AC-003）。
- prompt 用步骤 1 的 `build_imagegen_prompt`（source 由 `system_imagegen_skill_path()` 决定）。
- 进程执行复用 `run_child_with_timeout_stdin`、`codex_cli_path`、`which_node`、`suggestion_scratch_dir` 同层的私有辅助（同文件内直接调）。
- 事件流解析：新增 `fn parse_codex_thread_id(stdout: &str) -> Option<String>`，取第一条 `"type":"thread.started"` 的 `thread_id`。产物目录 = `<codex_home>\generated_images\<thread_id>`，等进程退出后取目录内 mtime 最新的 `exec-*.png`（实测产物名形态 `exec-<uuid>.png`；匹配放宽为 `*.png` 兜底）。找不到产物时报 `GenerationFailed { detail: "no image artifact" }`。
- 产物复制（非移动，保留 `generated_images` 原件）到 `out_path`；`None` 时默认 `%TEMP%\AgentWatcher\imagegen\image-<yyyyMMdd-HHmmss>.png`，父目录不存在则创建。
- 默认超时 240 秒（生图比建议慢，实测 20 秒内出图，留余量），`--timeout` 覆盖。

测试（lib.rs 末尾 tests 模块）：

- `imagegen_exec_args` 锁必需 flag 且**断言不含 `-m`/`model_reasoning_effort`**（AC-003 的机器判定）。
- `parse_codex_thread_id` 用实测事件流样例（含 `thread.started` 行）返回正确 id；空流返回 None。
- 产物选择函数 `pick_latest_image`（抽纯函数）：目录里多个 png 时选 mtime 最新；空目录返回 None。

怎么验证：`cargo test --manifest-path src-tauri/Cargo.toml`，预期全绿且新增 3 条。

### 步骤 3：CLI 接线（cli.rs / main.rs / commands.rs，先补 taxonomy 测试）

改哪些文件：

- `src-tauri/src/bin/agentwatcher-cli/cli.rs`：`Commands` 加 `Imagegen { action: ImagegenAction }`；`ImagegenAction::Generate { prompt: String, #[arg(long="out")] out: Option<String>, #[arg(long="timeout")] timeout: Option<u64> }`。doc 注释写明"会真实调用 Codex 生图、产生用量"。命令组注释（cli.rs 头部副作用分组）加一行：imagegen 调外部模型、产生用量，与 suggest 同级计费类别。
- `src-tauri/src/bin/agentwatcher-cli/main.rs`：`Commands::Imagegen { action } => match action { ImagegenAction::Generate { prompt, out, timeout } => commands::imagegen_generate(&prompt, out.as_deref(), timeout) }`。
- `src-tauri/src/bin/agentwatcher-cli/commands.rs`：`pub fn imagegen_generate(...)` 调 lib 的 `generate_image_via_codex`，成功输出信封（data：`outputPath`、`sourcePath`、`engine: "codex/imagegen"`、`promptSource`、`threadId`、`elapsedMs`、`prompt`），失败映射错误码：`CodexUnavailable → codex_unavailable`、`Timeout → codex_timeout`、`GenerationFailed → generation_failed`（detail 进 stderr 分类日志，格式对齐 suggest 的 `suggestion generator:` 前缀风格，用 `imagegen:` 前缀）。退出码 1。
- 测试 `src-tauri/tests/cli_taxonomy.rs`：加 imagegen generate 的解析用例（必填 prompt、--out/--timeout 可选、帮助文本含 imagegen）。
- 测试 `src-tauri/tests/cli_contract.rs`：加一条失败路径信封断言（例如用 `--out` 指向不可写路径或 CODEX_HOME 空目录触发失败，断言五键齐全、ok=false、退出码 1；具体触发方式按步骤 0 未登录实测结论选稳定的那个）。

怎么验证：`cargo test --manifest-path src-tauri/Cargo.toml --test cli_taxonomy --test cli_contract`，预期全绿。

### 步骤 4：真实验收（AC-001 / AC-002）

命令（在本机真实环境跑，构建 release 或 debug 均可）：

1. `cargo build --manifest-path src-tauri/Cargo.toml --bin agentwatcher-cli`
2. AC-001：`src-tauri/target/debug/agentwatcher-cli.exe imagegen generate "a minimal hero image of a ceramic coffee mug, clean product photography" --out %TEMP%\awf-img-accept\hero.png` → 信封 ok=true、hero.png 存在且为 PNG（前 8 字节 `89 50 4E 47`）、data.engine == "codex/imagegen"、promptSource == "system-skill"。
3. AC-002：`ren "%USERPROFILE%\.codex\skills\.system\imagegen" imagegen-bak` → 重跑同命令 → 信封 promptSource == "embedded" 且仍生图成功 → `ren` 改回。核对两次的 data 与落盘文件。
4. 失败路径抽查（AC-004 的人工复核部分）：`set CODEX_HOME=%TEMP%\empty-codex` 后重跑，确认退出码 1、信封 error.code 可区分。

证据：命令行、退出码、信封 JSON、产物截图或文件属性，落 `validation.md`；人工核对清单落 `manual-test.md`（两条：真实生图目视是咖啡杯马 克杯 hero 图；改名 skill 后仍出图）。

### 步骤 5：文档口径同步

改哪些文件：

- `AGENTS.md`「AI 操作 CLI」一节：命令面清单加 `imagegen generate`，标注计费类别与"生图模型由 Codex 云端自动选择，不随版本绑定"。
- `README.md`：CLI 命令表同步（如有 CLI 章节则加一行，没有则只改 AGENTS.md，README 不硬造章节）。

怎么验证：人工核对两份文档与 `--help` 输出一致。

## 风险与回退

| 风险 | 应对 |
| --- | --- |
| read-only 沙箱禁工具（步骤 0 发现） | 固定 workspace-write，scratch 限定在 `%TEMP%\AgentWatcher\imagegen-scratch\`，风险面等同既有建议管线 |
| Codex 云端行为变化（事件流字段、产物目录） | 解析全走"存在才用"的宽容路径；失败分类明确报错不静默 |
| `--ephemeral` 未来不再落 generated_images | 事件流兜底：产物路径若出现在 `command_execution` 事件输出里，优先从事件流提取路径 |

回退：整条命令是新增子命令，不改既有命令面，`git revert` 单次提交即可整体退回。

## 并行性

步骤 1、2 有依赖（prompt 构造被入口调用），需顺序做；步骤 3 依赖 2 的 pub 入口；步骤 4、5 依赖 3。无并行步骤。
