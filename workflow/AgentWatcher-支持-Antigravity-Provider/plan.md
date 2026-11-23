---
schema_version: "1"
protocol: "1.3.0"
artifact: "plan"
artifact_id: "ar_3SNE7ZQBXNDEWVYD7394M01FCA"
work_item_id: "wi_0ZQ8FFAX6K2B4D8F66T96TFDEM"
created_at: "2026-11-23T03:37:04Z"
producer: "aes-plan"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:377edb0dac11131577aa3523826b0b7accc7a012a8b0fef5fe9226ef0433ed54
  artifacts:
    - artifact_id: "ar_53PD3ENZXZYMZ9TY0TFB1DB3BF"
      digest: sha256:14a9d7a61d8a0e915aadc45f77d6883dfd2a5eba34d2595b1bdb8f810b99c635
      locator: "design.md"
result: "ready"
---
# 计划：AgentWatcher 支持 Antigravity Provider

对着设计 ar_53PD3ENZXZYMZ9TY0TFB1DB3BF（accepted）拆步。所有路径都在任务 worktree
`F:\AiProject\AgentWatcher\.aes-workflow\worktrees\AgentWatcher-支持-Antigravity-Provider`
下，命令一律在该目录跑。九步全序：1 到 4 步都在 `src-tauri/src/lib.rs` 一个文件里，
按层依赖排；5 到 8 步各自独立文件；9 步收口。

## S1：protobuf 裸解析器与数据模型（测试先行）

- 改 `src-tauri/src/lib.rs`：在 WorkBuddy 实现块（约 15630 行处）之后新增 antigravity 区块。
  先写测试夹具与测试函数，再写实现：
  - `antigravity_wire_field` 遍历器：varint 读取、按字段号取子消息/字符串/整数的纯函数族
    （`wire_first_bytes`、`wire_all_fields`、`wire_sub_messages`、`wire_string_at`、
    `wire_u64_at`）。
  - `AntigravityStepTail` 结构：步骤类型（14 用户轮 / 15 模型轮 / 132 工具轮 / 17 报错 /
    101 通知 / 其他）、正文文本、工具名、内容时间戳毫秒、token 四桶（字段 9 的
    f2/f3/f5/f6）、模型编号。
  - `antigravity_parse_step_payload(&[u8]) -> AntigravityStepTail`：按设计记录的字段号
    映射解析，取不到的字段返回 None，解析异常不 panic。
  - 测试：`antigravity_wire_roundtrip_varint`（varint 编码解码往返）、
    `antigravity_parse_step_user_text`（构造类型 14 字节串，断言字段 19 子字段 2 文本）、
    `antigravity_parse_step_model_text_and_tools`（构造类型 15，断言字段 20 子字段 3 文本
    与子字段 7 工具名）、`antigravity_parse_step_timestamp`（断言字段 5 秒加纳秒合成毫秒）。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml antigravity_` 全绿。
- 证明：测试输出里上述四个测试名通过。

## S2：清单扫描、卡片构造与状态机

- 改 `src-tauri/src/lib.rs`：
  - 常量 `ANTIGRAVITY_SESSION_SCAN_LIMIT_MAX`、SQLite busy 超时对齐 WorkBuddy。
  - `antigravity_home_dir()` → `~/.gemini/antigravity`（走现有平台助手）；
    `antigravity_summaries_db_path()`、`antigravity_conversation_db_path(id)`。
  - `AntigravitySummaryRow`（清单表行）与 `scan_antigravity_sessions(...)`：只读打开
    清单库，跳过 `killed=1` 与 `parent_conversation_id` 非空的行，`workspace_uris[0]`
    percent 解码成工作区路径。
  - `read_antigravity_session_row(...)` → `AgentSession`：ID `antigravity:<conversation_id>`，
    状态机按设计——waiting（尾部 ask_question 未闭合）> running（RUNNING/BUSY/CANCELING
    且内容时间戳 10 分钟内）> idle（IDLE/UNSPECIFIED 或超窗 RUNNING）；预览与用量接第 3 步。
  - `lookup_antigravity_session_by_key(...)` 定向查找。
  - 接线 8 处：`ScanOptions`/`ResolvedScanOptions`/`resolve_scan_options` 默认 true、
    `session_watch_roots` 挂 `conversations` 目录与清单库（含 -wal，不挂 daemon/presence）、
    `scan_sessions_blocking` 分发、`lookup_session_full_history` 名单与分发、
    `full_history_scan_options`/`lookup_session_via_wide_scan` 逐 provider 置位、
    `PerformanceProviderCounts` 加字段。
  - 既有 `ScanOptions {` 字面量全部补 `include_antigravity: None`。
  - 测试：临时目录拼清单库与 conversations 库（复用现有临时目录测试夹具写法）——
    `antigravity_scan_skips_killed_and_child`、`antigravity_scan_workspace_uri_decoded`、
    `antigravity_status_running_within_window`、`antigravity_status_stale_running_is_idle`
    （429 滞留坑）、`antigravity_status_ask_question_pending_is_waiting`、
    `antigravity_default_include_flag`、`antigravity_scan_missing_home_is_empty`。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml` 全绿。
- 证明：测试计数里新增 7 个 antigravity 测试通过、既有测试无回归。

## S3：预览、用量聚合与 topTools

- 改 `src-tauri/src/lib.rs`：
  - `summarize_antigravity_tail(...)`：conversations 库 `steps` 表尾采 24 行（与 OpenCode/
    ZCode 同口径），逐行过第 1 步解析器，出最后用户输入、AI 正文（过滤工具参数噪声与
    空段）、轮次/模型调用/工具调用计数、token 桶累加、topTools 计数、活跃时长（时间戳
    聚类，5 分钟间隔口径）、首末时间戳。
  - `AgentSession.usage` 填充（数据源没有的项 null：无窗口上限、无成本、无 credits）。
  - `get_session_usage_detail` 加 antigravity 分支返回 `{ usage, topTools }`。
  - 正文库缺失时降级：卡片照出、预览与用量 null（`antigravity_missing_body_db_degrades`）。
  - 测试：`antigravity_tail_preview_texts`、`antigravity_usage_counts_and_tokens`、
    `antigravity_top_tools_counts`、`antigravity_missing_body_db_degrades`。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml` 全绿。
- 证明：四个新测试名通过。

## S4：打开提示、接续导出与打包

- 改 `src-tauri/src/lib.rs`：
  - `open_antigravity_session(...)`：返回明确「not supported yet」错误（ZCode 同款文案
    模板，英文正文）；`open_session` 加分支。
  - `prepare_handoff_source_context(_blocking)` 加 antigravity 分支：全量读 steps（导出
    不受 24 行采样限制，走一次性顺序读），解码成 Markdown（用户/AI 轮正文 + 工具调用
    一行摘要 + 时间戳），落 `%TEMP%\AgentWatcher\handoff-sources\antigravity\<会话ID>.md`，
    文件名 = 卡片会话 ID 的自然键部分。
  - `export_session_package` 分发到 `antigravity_package_entries`：会话 db 原件 +
    清单行 NDJSON 落 `raw/antigravity/`。
  - 测试：`antigravity_handoff_markdown_contains_turns`（断言用户与 AI 轮正文都在、
    文件名正确）、`antigravity_package_entries_layout`。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml` 全绿。
- 证明：两个新测试名通过。

## S5：UI 接线

- 改 `ui/index.html`（单文件，全部改这一处）：
  - 设置面板 Data 组加 `includeAntigravity` 复选框（默认开）+ 中英文 i18n 键。
  - 主面板 provider 筛选下拉数组加 `'antigravity'`；徽标 `AG`、class `agy`；
    light/dark 两套徽标配色 CSS（.provider.agy / .provider-option-badge.agy）。
  - 默认设置值、读档合并、持久化 key 白名单、组装 ScanOptions、勾选态回填、change
    监听（照 includeWorkbuddy 的全部 6 组位置）。
  - 性能面板 provider 计数串加 AG；显示名映射加 Antigravity；预览窗懒加载前缀门控加
    `antigravity:`。
- 验证：`npm run build:ui` 成功；浏览器 mock 模式 `npm run dev:ui` 下卡片与开关可渲染
  （mock 数据里加一个 antigravity 会话），真实验收留给第 7 步。
- 证明：build 输出无报错。

## S6：CLI 接线与技能宿主

- 改 `src-tauri/src/bin/agentwatcher-cli/cli.rs`：帮助文本加 `antigravity:<id>` 与
  `--provider antigravity`；`commands.rs`：`ScanOptions` 默认值、`--provider` picks、
  全历史定向查找置位、`KNOWN_PROVIDERS` 加前缀、`session show --content` 节选走抽取器
  （最后输入输出节选 + 原始 db 路径）。
- 改 `src-tauri/src/bin/agentwatcher-cli/skill.rs`：宿主候选表加
  `("~/.gemini/config/skills", "antigravity")`。
- 改 `src-tauri/tests/cli_contract.rs`：宿主名单断言同步加 antigravity。
- 测试先行：先在 `cli_taxonomy.rs`/`cli_contract.rs` 补 antigravity 相关断言（provider
  参数接受、KNOWN_PROVIDERS、宿主名单），跑红再实现。
- 验证：`cargo test --manifest-path src-tauri/Cargo.toml --test cli_taxonomy --test cli_contract`。
- 证明：两组测试全绿。

## S7：真实交互流程测试

- 改 `tools/tauri-realtest/cli.mjs`：别名表加 `antigravity-session`（含中文别名）、
  `flowDisplay` 显示名、runFlow 分发分支、`runAntigravitySessionFlow`（照
  `runWorkBuddySessionFlow`：等卡片出现、断言 `data-provider="antigravity"` 与徽标 AG、
  点击断言「not supported yet」toast、截图）。
- 验证：`npm run test:tauri:flow -- antigravity-session`（本机有真实 Antigravity 会话，
  数据源现成）。
- 证明：`.tmp/tauri-realtest/latest-result.json` 里该流程 ok，截图存在。

## S8：文档同步

- 改 `AGENTS.md`：开头 provider 名单加 Antigravity；`ScanOptions` 字段清单加
  `includeAntigravity`；`open_session` 一节加 antigravity 分支说明；「Session 数据源」
  加数据根/清单库/正文库/protobuf 口径；采样口径加 24 步；状态机一节加 antigravity
  判定；`test:tauri:flow` 流程名清单加 `antigravity-session`；skill 宿主表口径同步。
- 改 `README.md`：特性 provider 名单、session list 参数表、技能宿主目录清单。
- 改 `CHANGELOG.md`：未发布条目一条（不发版，不写 SHA256）。
- 验证：`grep -n -i antigravity AGENTS.md README.md CHANGELOG.md` 逐处复核口径；
  与设计记录无打架。
- 证明：grep 输出的每一处与实现一致。

## S9：全量验证收口

- `cargo test --manifest-path src-tauri/Cargo.toml`（全量含新测试）。
- `cargo test --manifest-path src-tauri/Cargo.toml --test cli_taxonomy --test cli_contract`。
- `node --check vscode-agentwatcher-bridge/extension.js`（未动 Bridge，发布规范例行）。
- `npm run test:tauri:flow -- antigravity-session`。
- 逐条对照 work-item 的 AC-001 到 AC-008 写验收记录，人工核对清单留给用户
  （等回复判定、429 超窗两条真机项）。
- 证明：四条命令输出加验收记录。

## 回退

每步一个提交（施工提交只含代码，`workflow/` 记录不进提交）。任一步出问题回退该步
提交即可；第 2 步动了一堆既有 `ScanOptions` 字面量，回退时连字面量补字段一起回。
