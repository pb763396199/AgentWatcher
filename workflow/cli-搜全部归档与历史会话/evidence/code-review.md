---
schema_version: 1
protocol: 1.3.0
artifact: review
artifact_id: ar_01M5C8HPPC0003DFQ2NPFSM165
work_item_id: wi_01M5C61SH5000FME6B0YSBBTDE
created_at: 2026-10-20T09:10:00Z
producer: aes-review
verdict: approved
supersedes: null
contract_revision: 1
verified_at_commit: 39962ef838af0abb0f1e502d36490995ccca908c
review_type: code
reviewers: []
dependencies:
  work_item_contract_digest: sha256:66c5eb385a5e74388b6cd2c4a87f41f8d92e69603379b6bb23da5196597a30de
  artifacts: []
---

# 代码评审：CLI 全历史与归档会话搜索

**本评审由写这段代码的同一个 agent 做（自审），按流程在此声明。** 变更集中在 `src-tauri/src/lib.rs`（扫描选项、SQLite 扫描、Codex zst、定向查找）、`src-tauri/src/bin/agentwatcher-cli/`（参数与编排）、`skill/SKILL.md`、`AGENTS.md`、`src-tauri/tests/cli_taxonomy.rs`，共 9 个文件、+674/−112，逐 hunk 读过。

## 结论

通过。GUI 默认路径不变有双重保障（机器锚点 + 新旧二进制对照）；CLI 契约（信封、退出码、列表无正文）未被触碰；隐私分级未变。

## 审查中抓到的问题

| 位置 | 问题 | 严重度 | 处理 |
| --- | --- | --- | --- |
| lib.rs `read_zst_suffix_bytes` | 初版窗口扩容策略可能让内存涨到 2×max_bytes 以上 | 低 | 已改为每次 drain 到 max_bytes，内存有界（≤ max_bytes + 64KB chunk） |
| lib.rs `cached_or_read_codex_file_session` | HeadOnly 薄摘要若落缓存，后续定向 show 会拿到无尾部预览 | 高 | 已修：仅 `CodexZstDepth::Full` 的摘要落缓存 |
| lib.rs `sqlite_session_scan_limit` | 第一版全历史上限 `max_sessions` 会吃掉 ×3 裕量 | 中 | 已修：全历史返回未夹取值，单测锁定 60,000 |
| commands.rs `find_session` | 每次未命中都触发全历史宽扫，claude 路径要重读全部文件 | 低 | 接受：CLI 一次性进程，定向路径前缀分流后 claude 才走宽扫，release 实测 5.9s 内完成 |

## 已核实的事实

- `resolve_scan_options` 对 `None`/非 0 入参的输出与改动前逐字段一致（`test_resolve_scan_options_defaults_preserved`）；`clamp(1, 30)` 与 `clamp(1, 5000)` 语义保留为命名常量。
- GUI 的 Tauri 命令路径不经 `lookup_session_full_history`（该函数只有 CLI 调用）；`session_watch_roots` 的 archived 目录条件（`!hide_archived`）未改，GUI 默认 `hide_archived=true` 不监视归档目录。
- 隐私边界：`LIST_KEYS` 未变，`session list` 仍无 `lastUserMessage`/`lastAiMessage`（cli_contract 断言未动且通过）。
- zst 解压副本写入位置沿用 handoff-sources 的 AgentWatcher 临时目录命名空间，文件名来自源文件（已剥离 `.zst`），无路径注入面（`File::create` 不解释路径，基名不含分隔符——`file_name()` 已保证）。
- ruzstd 0.9.1 为纯 Rust、decode-only，无 build.rs C 编译；传递依赖仅 twox-hash。
- Codex app-server 路径（GUI 用）对 `hide_archived` 的处理未改；CLI 仍 `set_codex_app_server_disabled(true)`。

## 非阻断问题

- 全历史列表里 zst 会话与预算外 SQLite 会话的预览文本为空（设计取舍，list 契约不含正文）；依赖 `session show` 定向兜底补全。
- `LIST_KEYS` 里的 `updatedAtMs` 键与序列化字段名不匹配（列表里从未出现过该键）——改动前即如此，未在本任务修，避免混入无关行为变化。

## 没覆盖的范围

- 未跑 `npm run test:tauri:*`：本任务无 UI/窗口改动，GUI 扫描入口只动了 lib 内部分支（有单测锚点）。发布该版本时应按发布规范补跑。
- 未验证非 Windows 路径下 zst 的行为（本产品本来就只在 Windows 下支持会话扫描）。
