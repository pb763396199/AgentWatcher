---
schema_version: "1"
protocol: "1.3.0"
artifact: "plan"
artifact_id: "ar_4BNZYPTFCZ2947619P78B2X5ZY"
work_item_id: "wi_3KXFZQMQ9AH2XQ8PGQBW299E2P"
created_at: "2026-10-14T06:44:59Z"
producer: "aes-plan"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:288b282963a92773fb16e68c5fd502e3994c05fcdb071bb4aee77785c21e33b2"
  artifacts:
    - artifact_id: "ar_01M4W7K8S60002YW4JZC6E9A5E"
      digest: "sha256:ff6352642554acccf159ede71a3efaa1251f88838a118c1200c2484c550c301c"
      locator: "design/session-lifecycle-design.md"
result: "ready"
---
# 单会话全生命周期分析视图（阶段 A）施工计划

设计：`design/session-lifecycle-design.md`（ar_01M4W7K8S60002YW4JZC6E9A5E，accepted）。
基线：feature/会话分析统计面板 = dev `12491fa`。共用文件：`src-tauri/src/lib.rs` 与
`ui/index.html` 各只有本任务一个写者，步骤串行做。

与设计的一处实现偏差先说清：设计写「JSONL 聚合器在增量缓存值里保留逐调用数组」，
实现改为**独立的懒加载时序缓存**（新静态 Map，键=文件路径，值含逐调用点），理由：
扫描热路径的 `CachedJsonlUsage` 每次扫描逐会话 clone，塞进上千个点会放大内存与拷贝；
懒加载只在用户点开分析时读一次文件（实测 37–65ms/文件），与 `get_session_usage_detail`
同一模式。预算降级（5000 点）按设计执行。

| 步 | 内容 | 改动文件 | 验证 | 证据 |
| --- | --- | --- | --- | --- |
| S1 | Rust 数据结构与降采样：`LifecycleCallPoint`/`SessionLifecycle` 结构体（serde camelCase）；`downsample_lifecycle_calls(calls, budget=5000)` 连续同模型合并、保总和、末点上下文；`TIMELINE_POINT_BUDGET` 常量 | lib.rs（SessionUsageDetail 定义之后） | 新增单测：`downsample` 输入 12000 个双模型交替点 → 输出 ≤5000 且 input/output 总和与输入一致、context 取每桶末值 | cargo test 该用例绿 |
| S2 | Rust ZCode/OpenCode 时序查询：`zcode_session_lifecycle`（model_usage 按 started_at 逐行 + part type='tool' 时间点 + message real_user 时间点）、`opencode_session_lifecycle`（assistant 消息 data.tokens 逐条 + part/user 时间点），复用既有连接与 json_extract 写法 | lib.rs（zcode_usage_detail / opencode_usage_detail 旁边） | 扩展既有 SQLite fixture 测试：插入 3 行 model_usage + 2 行 tool part → 断言点数、顺序、总和 | cargo test 新用例绿 |
| S3 | Rust JSONL 三源 timeline 解析：`parse_claude_timeline`、`parse_codex_timeline`（沿 rollout 链拼接）、`parse_copilot_chat_timeline`（先抽取 `copilot_chat_requests` 共用函数供聚合器与 timeline 复用）；静态 `JSONL_TIMELINE_CACHE`（路径→size+modified+点集） | lib.rs | 新增 fixture 单测：每源最小 JSONL 文本 → 点数、token、时间戳、轮次/工具时间点断言；既有聚合测试不回归 | cargo test 全量绿 |
| S4 | Rust 命令与注册：`get_session_lifecycle(id)` → scan 解析会话 → 按 provider 分发（zcode:/opencode: 前缀走 SQL；codex 走链；claude/copilot 读 session_path；copilot-cli 降级空时序）；`generate_handler` 注册 | lib.rs（get_session_usage_detail 旁 + 注册表） | 单测：伪造 scan 结果的分发函数；CLI 契约测试确认信封不回归 | cargo test + cli 两套绿 |
| S5 | 前端窗口：`capabilities/analytics-panel.json`（照 performance-panel.json）；boot 判定 `?analytics=1`；`showNativeAnalyticsWindow(sessionId)` 窗口样板（label analytics-panel、几何键 `agentwatcher.analyticsPanelGeometry`、默认 880×640、min 560×480）；目标会话经 `agentwatcher-analytics-target` 事件传入并触发拉取 | capabilities/analytics-panel.json、ui/index.html | `npm run build:ui` 过；dev 下 CDP 打开窗口确认 `window.__TAURI_INTERNALS__===true` 且标题正确 | CDP 截图 |
| S6 | 前端视图：身份区、7 指标卡、按模型堆叠面积图（h/6h/d 三档）、上下文曲线（自适应纵轴、骤降>40% 标「压缩」）、活动节奏条、按类型/按模型/按工具分解、模型徽章过滤、三图联动悬停、降级横幅与 Copilot CLI 降级布局；i18n 两字典各加键 | ui/index.html | dev 下 CDP：六源各开一个会话视图核对（本机至少有 zcode/claude/codex 样本）；切粒度/过滤/悬停/双语/双主题 | CDP 截图 + result.json |
| S7 | 前端入口：预览用量区块尾部「会话分析」按钮 + handoff 右键菜单加同名项，都调 `openAnalyticsForSession(sessionId)` | ui/index.html | dev 下 CDP：从预览按钮与右键菜单各打开一次，窗口内容与目标会话一致 | CDP 截图 |
| S8 | 全量回归：`cargo test --manifest-path src-tauri/Cargo.toml`、`cargo test … --test cli_taxonomy --test cli_contract`、`npm run test:tauri:smoke` | — | 全部退出码 0；smoke 的 result.json 无 console/pageerror | 命令输出 + .tmp/tauri-realtest/latest-result.json |

| S9 | Rust 子代理发现：`codex_subagents_of(parent_thread_id, segments)`——扫父 rollout 同目录文件首行 `source.subagent`，thread_spawn 按 parent_thread_id 精确、guardian 按 cwd+时间窗；每个子代理用既有 `parse_codex_usage` 取用量与模型、取首末时间戳；预算 40 条按毛输入截断 | lib.rs | 临时目录 fixture（父+2 worker+1 异会话 worker+1 guardian）：断言归属与计数 | cargo test 新用例绿 |
| S10 | Rust 结构与命令扩展：`TrajectoryAgent`/`OrchestrationEvent` 进 `SessionLifecycle`（serde camelCase）；`parse_codex_timeline` 增收 spawn_agent/send_message/wait_agent 的 (tMs, kind)；等待区间=每个 wait 回溯最近 spawn 的近似 | lib.rs | 扩展既有 codex timeline fixture：断言 orchestration 事件与等待区间 | cargo test 绿 |
| S11 | 前端轨迹泳道区：默认只主代理；子代理组/排行展开收起（明细挂泳道节点内防残留）；泳道点击懒加载该子会话 lifecycle 并渲染合并明细（面积=输入、折线=上下文、non-scaling-stroke 细线、≤4 倍行高、像素对齐、共享轴+悬停读数）；模型切换参考线、编排标记、等待阴影（估算标注）、并行度曲线、效率列、KPI 扩展、i18n | ui/index.html | dev 下 CDP：用真实多智能体会话核对泳道数/合计/明细/收展 | CDP 截图 + result |
| S12 | 回归：cargo test 全量、npm run build:ui、真实会话 CDP 走查（含无子代理会话不渲染泳道区） | — | 全绿 | 命令输出 |

验收对照：步骤 1–4 对应 AC-004/005/006 的数据面，5–7 对应 AC-001/002/003/007 的界面面，
AC-002 的六源核对由步骤 6 的 CDP 走查覆盖（本机没有的源以 mock 数据在 dev:ui 不可用时标注
「未实测」）。风险最大处是步骤 3 的三源解析与聚合口径漂移：每个解析器的总和断言必须与
对应 `parse_*_usage` 的累计值一致，对不上先修解析器再继续。

