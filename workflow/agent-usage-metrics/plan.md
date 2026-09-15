---
schema_version: 1
protocol: 1.3.0
artifact: plan
artifact_id: ar_01M2J1HY7000034P57XZ03CQ8W
work_item_id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
created_at: 2026-09-15T08:05:00Z
producer: aes-plan
result: ready
supersedes: null
dependencies:
  work_item_contract_digest: sha256:b1d0e0503375d88153a6711d42195454acdbf4b2117b2b4a4a6cca41a6fcc8d9
  artifacts:
    - artifact_id: ar_01M2HX0EKBWVB8P9ZMGGJMPG2T
      digest: sha256:1b2927edba2ce43f95a09df09d6aa0e4a5b1b08b46c3bc259a4a901c8252ac91
      locator: design.md
---

# 会话用量指标实现计划

工作目录：`F:\AiProject\AgentWatcher\.aes-workflow\worktrees\agent-usage-metrics`（分支 `feature/agent-usage-metrics`，基线 `bc915a2`）。以下路径都相对该目录。依赖：设计 `ar_01M2HX0EKBWVB8P9ZMGGJMPG2T`（accepted），调研 `ar_01M2HX0EEVEXZEF3NZYTJFG22H`。

| 步骤 | 做什么 | 改哪些文件 | 验证 | 完成证明 |
| --- | --- | --- | --- | --- |
| S1 | Rust 数据模型：新增 `SessionUsage` 结构体（13 字段，全部 Option 除 models），`AgentSession` 加 `usage: Option<SessionUsage>`（serde camelCase，None 跳过序列化） | `src-tauri/src/lib.rs`（结构体区，AgentSession 在 95-123 附近） | `cargo test --manifest-path src-tauri/Cargo.toml test_agent_session_usage_fields_serialize` | 新测试通过：带 usage 的 AgentSession 序列化含 `totalInputTokens` 等 camelCase 键 |
| S2 | JSONL 聚合器 + 增量缓存：纯解析函数 `parse_claude_usage` / `parse_codex_usage` / `parse_copilot_chat_usage`（按调研公式），通用缓存 `lookup_or_insert_jsonl_usage(path, provider)` 按（大小、mtime）命中，进程内 Mutex<Map>，容量上限 1024 条；每次扫描未聚合文件预算 `USAGE_AGGREGATION_BUDGET_PER_SCAN=10`，未命中预算的会话本轮 usage=None（渐进填充） | `src-tauri/src/lib.rs`（JSONL 辅助函数区，`read_file_prefix_text` 6907 附近） | 新单元测试：Claude 按 message.id 去重求和与轮次/工具计数（合成 JSONL）；Codex total_token_usage 取最后一条、上下文跳过全零事件；Copilot 补丁日志快照+单点补丁+全量重写三种行型；缓存闭包只执行一次 | 全部新测试通过，`cargo test` 无回归 |
| S3 | 各源接线：claude/codex 文件/ copilot 三个 summary 构建处调聚合器；codex app-server 模式加 rollout 文件索引（TTL 缓存目录遍历，thread_id→路径）；copilot_cli 由既有 SQL 行（turn 数、created/updated）构造 usage；zcode 在扫描列表后用 3 条 IN 批量 SQL（model_usage 聚合 / tool_usage 计数 / message JSON 用户轮数+上下文+模型名）构建 map 后附加；opencode 扩展 session 列表 SQL 取 cost/tokens_* 列构造 usage（上下文/轮次/工具为 None，走懒加载） | `src-tauri/src/lib.rs`（各 scan 函数与 AgentSession 组装处） | 新单元测试：zcode 内存库 fixture 含 model_usage/tool_usage/semantics 字段；opencode fixture 含 token 列；copilot_cli fixture 断言 usage.userTurns/durationMs | 对应测试通过；`cargo test` 既有 provider 测试不回归 |
| S4 | 新命令 `get_session_usage_detail({ id })`：返回 `{ usage, topTools }`；opencode/zcode 走 per-session SQL（角色计数、tool part 计数、最后 assistant tokens/modelID、top 工具）；JSONL 源从聚合缓存附加 topTools；注册进 `generate_handler!` | `src-tauri/src/lib.rs`（command 区，4643 附近） | `cargo test get_session_usage_detail`（zcode/opencode fixture 各一）；`cargo build` | 测试通过，命令注册后编译通过 |
| S5 | 前端卡片：`createSessionCard` 加 `.card-usage` 行（轮次+工具+token，缺段跳过，全缺隐藏）；`buildSessionSignature` 纳入 usage 关键字段；CSS 三档密度隐藏规则 + light 变体；token 格式化（k/M/B） | `ui/index.html`（15599 附近、15721 附近、样式区） | `npm run build:ui` 成功；`npm run test:tauri:smoke` 通过 | 构建产物生成，smoke 全绿 |
| S6 | 前端预览：`sessionPreviewData` payload 加 `sessionId`+`usage`；`renderSessionPreview` 加「用量」区块（hero 主数字=累计毛输入+缓存占比；网格=输出/上下文(+窗口)/轮次/工具/模型调用/模型/时长/成本；CLI 源加无数据说明）；预览窗收到数据后 invoke `get_session_usage_detail` 合并 topTools；i18n en/zh 各约 12 键 | `ui/index.html`（14953-15024、15214-15254、6344/6391 字典） | `npm run test:tauri:flow -- opencode-session`、`-- zcode-session`、`-- smoke` | 三个 flow 全绿，预览窗出现用量区块 |
| S7 | 验收门与发布卫生 | — | `cargo test` 全量、`npm run test:tauri:smoke`、flow 三个、`git diff --check`、`node --check vscode-agentwatcher-bridge/extension.js`（确认未波及） | 全部命令退出码 0 |
| S8 | 记录与提交：写 change-note.md、implementation.md、reviews/code-review.md（自审）、validation.md（acceptance 逐条）、manual-test.md（人工清单交用户）；`git add` 仅代码与测试文件提交到特性分支（workflow/ 留工作区） | 记录文件 + git | `awf gates`（commit-guard）通过；`git log -1` 只含代码 | 提交落在 feature/agent-usage-metrics，任务记录未混入 |

依赖关系：S1→S2→S3→S4 串行（同一文件 lib.rs）；S5、S6 依赖 S3 的字段（同文件 index.html，串行做）；S7 依赖全部；S8 最后。无并行步骤。

风险与退路：全部改动在特性分支，任一步失败 `git checkout -- <file>` 或 `git reset --hard bc915a2` 回退。最大风险是 Copilot 补丁日志的全量解析在大文件（22MB 单行）下的首扫耗时，预算常量兜底（渐进填充）。
