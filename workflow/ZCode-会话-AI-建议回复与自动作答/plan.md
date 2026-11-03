---
schema_version: "1"
protocol: "1.3.0"
artifact: "plan"
artifact_id: "ar_03QZ5EWXZXV89C2WW64GEZ7501"
work_item_id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
created_at: "2026-10-30T03:50:38Z"
producer: "aes-plan"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:2b1c17e9e8d949086387fef35bc752d834b0026f368ec9f17c071b583ecf4d0a
  artifacts:
    - artifact_id: "ar_0KMYDEZFWFEYQ4A58SPWM0MAV4"
      digest: sha256:3a79da40b6ee1d8bbf75bb7ef9debd0837a232082c42871ab8fd7bcd3b95c7ba
      locator: "design.md"
result: "superseded"
---
# 施工计划

对着设计 `ar_0KMYDEZFWFEYQ4A58SPWM0MAV4`（accepted）拆。步骤 1 到 10 顺序执行，每步一次提交；全部代码提交完跑步骤 10 的全量回归。工具目录：`awf gates commit-guard` 每次提交前跑。

## S1 依赖与插件注册

- 改 `src-tauri/Cargo.toml`：`[dependencies]` 加 `ureq = { version = "2", features = ["json"] }` 和 `tauri-plugin-global-shortcut = "2"`。
- 改 `src-tauri/src/lib.rs` 的 `run()`：builder 链上 `.plugin(tauri_plugin_global_shortcut::init())`（跟在 `tauri_plugin_opener` 后面）。
- 改 `src-tauri/capabilities/default.json`：`permissions` 数组加 `"global-shortcut:allow-register"` 和 `"global-shortcut:allow-unregister"`。
- 验证：`cargo build --manifest-path src-tauri/Cargo.toml` 退出码 0。证明：终端输出无 error。

## S2 管线契约单元测试（先写会失败的）

- 改 `src-tauri/src/lib.rs` 文件末尾 `mod tests`：新增两个测试函数。
  - `zcode_suggestion_pipeline_contract`：状态对比（上轮 idle 本轮 waiting 触发；上轮 running 本轮 idle 不触发；同一会话在途不重复触发）、指纹校验（最后一条消息 ID 变了即作废）、提示词组装（包含"120 个字"、"具体动作和具体对象"、待选项时包含"原样使用其中一个选项"、结尾 JSON 格式句）、响应解析（`{"suggestion":"继续跑测试"}` 通过、`{"suggestion":null}` 与坏 JSON 与 121 字超长都判为不出建议）。
  - `zcode_auto_answer_guardrails`：每会话每小时第 7 次被拒；连续两次同答案后熔断，手动发送一次后解除；等待未满 20 秒不动；生成器返回选项集之外的文本时放弃不发送；审计行为 `时间	会话ID	问题	选项	所选	结果` 的 JSONL 行且字段齐全。
- 这些测试引用的类型与函数此步只写签名（`todo!()` 或空实现），保证编译过、测试红。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml zcode_suggestion zcode_auto`，两个测试失败。证明：失败输出里能看到断言位置。

## S3 管线核心实现（让步骤 2 转绿）

- `src-tauri/src/lib.rs` 新增：
  - `struct SuggestionSettings { enabled, api_base, api_key, model, hotkey, hotkey_action, auto_enabled, auto_hourly_limit, auto_wait_secs }` 与 `Default`。
  - `struct SessionSuggestion { session_id, provider, text, options: Option<Vec<String>>, fingerprint, created_at_ms }`。
  - `fn suggestion_should_trigger(prev_status, cur_status) -> bool`；`fn build_suggestion_prompt(context: &SuggestionContext) -> String`（正文用设计里那段全文，上下文追加在后面）；`fn parse_suggestion_response(body: &str, max_chars) -> Option<String>`；`struct AutoAnswerBudget` 与 `fn auto_answer_decision(...)`（纯函数，预算、熔断、等待时长、选项匹配都在这里）。
  - `fn zcode_suggestion_context(session_id) -> Option<SuggestionContext>`：复用扫描的 SQLite 只读打开方式，取该会话最后 8 条 message（user/assistant 交替，各截 600 字）加 `session.directory`，`part` 表里 `tool = "AskUserQuestion"` 且 pending 的行解析出问题与选项（沿用扫描状态机已有的解析代码路径）。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml zcode_suggestion zcode_auto` 全绿。证明：`test result: ok`。

## S4 触发挂接与建议事件

- `src-tauri/src/lib.rs`：`static SUGGESTION_STATE: Mutex<...>`（上次各会话状态快照、每会话在途标记、最近一条建议——热键用）。
- `scan_sessions` 命令返回前 `std::thread::spawn`：对比快照，对新进 waiting 的 ZCode 会话置在途标记后调 `zcode_suggestion_context` + `call_suggestion_model`，成功则 `app.emit("agentwatcher-suggestions-updated", payload)` 并更新最近建议；失败静默（记日志）。线程退出时清在途标记。
- `fn call_suggestion_model(settings, context) -> Option<String>`：`ureq` POST `{api_base}/chat/completions`，体 `{"model", "messages":[{system 提示词},{user 上下文}]}`，60 秒超时，网络层不写单测（薄封装），请求体组装在步骤 3 测过。
- 验证：`cargo build` 通过；`cargo test` 全绿。证明：编译与测试输出。

## S5 send_zcode_message 命令

- `src-tauri/src/lib.rs` 新增 `#[tauri::command] fn send_zcode_message(app, id, text) -> Result<(), String>`，注册进 `generate_handler`：
  - `fn resolve_zcode_exe() -> Result<PathBuf, String>`：读注册表 `HKCR\zcode\shell\open\command`（`winreg` 已在依赖里则复用，不在则加 `winreg = "0.52"`），从 `"C:\...\ZCode.exe" -- "%1"` 里剥出 exe 路径并确认存在。
  - 参数组装 `fn zcode_send_args(zcode_cjs, id, cwd, text)`：`[zcode.cjs 路径, "--resume", id, "--cwd", cwd, "-p", text, "--json"]`；环境变量插 `ELECTRON_RUN_AS_NODE=1`；`CREATE_NO_WINDOW`；`wait_timeout(120s)`。
  - 失败分类：注册表缺失 → `"未找到 ZCode 桌面版"`；非零退出 → stderr 前 200 字进错误串；超时 → 杀进程报超时。
  - 成功后 `app.emit("agentwatcher-sessions-changed", ())` 让前端立即重扫。
  - 发送前校验指纹：目标会话当前状态与建议指纹不符返回 `"会话已前进，建议已失效"`。
- 单元测试：注册表命令串解析（三种带引号形态）、参数组装、用 `cmd /c exit 2` 模拟非零退出的错误分类。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml send_zcode` 全绿。证明：测试输出。

## S6 全自动护栏挂接与审计

- 步骤 4 的扫描后线程里加一段：设置开、会话 waiting 且带选项、首见 waiting 时间距今超过 `auto_wait_secs`、`auto_answer_decision` 放行 → 生成（选项约束）→ `send_zcode_message` 同一内部函数 → 追加 `%TEMP%\AgentWatcheruto-answersudit.jsonl` 一行（`serde_json::json!` 组装，追加打开）。
- 单元测试：审计行落盘与内容（`std::env::temp_dir()` 下唯一文件名，测完删）。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml zcode_auto` 全绿。证明：测试输出。

## S7 set_suggestion_settings 命令与热键

- `#[tauri::command] fn set_suggestion_settings(app, settings: SuggestionSettings)`：存进 `SUGGESTION_STATE`；热键有变化时 `app.global_shortcut().unregister_all()` 后 `register(settings.hotkey)`，回调里按 `hotkey_action` 执行：`copy` → 建议文本写剪贴板（复用现有剪贴板代码）+ toast 事件；`send` → 调发送函数。注册失败（热键被占）toast 报错不 panic。
- 验证：`cargo build` 与 `cargo test` 全绿。证明：编译输出；热键真按属于 AC-004 人工项。

## S8 前端卡片与设置

- 改 `ui/index.html`：
  - 监听 `agentwatcher-suggestions-updated`，按 `sessionId` 把建议挂到对应 ZCode 卡片；waiting 卡片渲染置灰建议行（60 字截断）与 复制/发送/换一个 三个动作；`options` 存在时渲染选项芯片。换一个 = `invoke('regenerate_suggestion', { id })`（后端加这个薄命令，带避开上条重生成，同一条最多两次）。
  - 设置面板新增"AI 建议"区块：启用开关、API 地址、密钥、模型名、热键、热键动作（复制/发送）、全自动开关、每小时上限、等待秒数，九项读写 `agentwatcher.settings` 现有键的 `suggestion` 子对象，保存时 `invoke('set_suggestion_settings', ...)`，启动时也推一次。
  - 中英文词条进现有 i18n 字典（两边各约 14 条）；dark/light 走现有 CSS 变量。
- 验证：`npm run build:ui` 退出码 0。证明：Vite 构建输出。

## S9 流程测试扩展

- 找到 `npm run test:tauri:flow -- zcode-session` 的步骤定义（`scripts/` 下），追加两步：设置面板出现"AI 建议"区块并可保存；扫描后若存在 waiting 的 ZCode 卡片，轮询 30 秒断言出现建议行（本机真实环境有 ZCode 在跑；测试对模型 API 指向本地假服务，脚本内起一个只回 `{"suggestion":"建议冒烟"}` 的 HTTP 服务，测完关掉；没有 waiting 会话时该步跳过并记录原因）。
- 验证：`npm run test:tauri:flow -- zcode-session` 通过，结果 JSON 在 `.tmp/tauri-realtest/latest-result.json`。证明：该文件里新增步骤 `passed`。

## S10 AGENTS.md、全量回归与人工核对清单

- 改 `AGENTS.md`：ZCode 相关段落补一句"会话卡片可出 AI 建议并经 zcode.cjs --resume 发送；全自动仅限 AskUserQuestion 选项作答，默认关；点击跳转仍不实现"。
- 跑全量：`cargo test --manifest-path src-tauri/Cargo.toml`、`node --check vscode-agentwatcher-bridge/extension.js`、`git diff --check`、`npm run test:tauri:flow -- zcode-session`。
- 写 `manual-test.md`：AC-002/003/004/007 的人工步骤（真实 ZCode 会话一键发送往返、热键在别的窗口聚焦时按一次、设置四象限、重启生效），交给用户勾。
- 验证：上述四条命令全过。证明：各命令退出码与 latest-result.json。

## 回退

每步一个提交，回退就是 `git revert` 单个提交。步骤 1 的依赖删除会让后续代码编译失败，回退时按倒序 revert。`workflow/` 记录不进这些提交。
