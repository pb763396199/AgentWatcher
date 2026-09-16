---
schema_version: "1"
protocol: "1.3.0"
artifact: "research"
artifact_id: "ar_6NMBYQFZVZY8AXPPP9GA39CPMA"
work_item_id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
created_at: "2026-09-16T03:13:00Z"
producer: "aes-research"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:32abb9b77345e1c4412d7442f9ae353d1bca154ec66037eeaf403b276872b2ea
  artifacts: []
result: "complete"
topic: "interview"
---
# 访谈记录：AI 操作 CLI 的目标与边界

## 方法

两轮问答，2026-09-16，用带推荐项的选项表。第一轮问四个互不依赖的拍板点（消费者、大类范围、进程架构、隐私模型），每个选项都带代价说明。第二轮做边界复查（handoff 覆盖段、二进制分发形态），复查前先核实了一个隐藏假设的事实基础（见「证据引用」第二节）。

## 第一轮：四个拍板点

| 拍板点 | 用户选择 | 落到任务里是什么 |
| --- | --- | --- |
| 首要消费者 | AI agent 优先 | JSON 信封输出是一等公民，人类可读模式次要 |
| v1 大类 | session 查询 + handoff 接续 | 运维工具、插件操作不进 v1 |
| 进程架构 | 独立 headless 进程 | 新二进制直读数据源，不要求 GUI 在运行 |
| 隐私模型 | 按需输出正文 | 列表默认只给元数据，正文要显式请求 |

## 第二轮：边界复查

| 复查点 | 用户选择 | 与推荐的关系 |
| --- | --- | --- |
| handoff 覆盖到哪一段 | v1 仅导出 | 推翻了「导出和派发都进 v1」的推荐 |
| 二进制进发布包的形态 | 独立 exe，全名 agentwatcher-cli.exe | 采纳推荐 |

## 被否决的选择

- 连接运行中的 GUI（本地 IPC）：能读实时内存态，代价是 GUI 必须在运行、宿主要新开一条服务端通道。未选。
- 运维工具大类（bridge 状态与安装、性能快照、环境自检）：偏排障场景。未选。
- 插件操作大类（扫描、挂载、启用、调用）：受插件宿主硬规则约束，v1 不碰。未选。
- 正文默认全开：误执行一次列表就倾倒全部会话正文。未选。
- v1 仅元数据、正文锁在 GUI 里：和发起本任务的核心诉求（AI 查会话内容）矛盾。未选。
- handoff 派发进 v1（访谈中推荐过）：用户拍板 v1 只做导出，派发给后续版本。
- 短名 awc.exe：多一个缩写要记。未选。

## 证据引用

- AgentWatcher 现状（2026-09-16 同日会话核实）：`src-tauri/Cargo.toml` 没有额外的 `[[bin]]` 目标，`main.rs` 只有一句 `agentwatcher_lib::run()`；全部 19 个 `#[tauri::command]` 注册在 `lib.rs:3419` 附近，只能由内置 WebView 前端调用；没有本地查询服务（TCP 相关代码是 handoff 拉起 OpenCode / Codex 服务的内部机制，不对外提供查询）。
- 会话 ID 跨进程稳定的假设：`lib.rs` 里六个 provider 的 ID 都是「provider 名加自然键」拼出来的（`codex:<thread_id>` 在 3595 行、`opencode:<row.id>` 在 4479 行、`zcode:<row.id>` 在 5549 行、`copilot:<session_id>` 在 7112 行、`claude:<session_id>` 在 7280 行），不依赖随机数。AI 先 list 拿 ID、再用 ID show，这条路走得通。

## 完成理由

目标（AI agent 优先的查询 CLI）、范围（两个大类加四个明确非目标）、强约束（正文按需输出、插件硬规则、发布合同标识符不动）都有用户的明确选择，没有遗留未答的问题。

## 残余风险

- 发布 zip 多一个 exe 后，README 和 AGENTS.md 里关于发布内容的口径要跟着改，AC-007 已覆盖。
- udf 有 `skill install` 命令把用法文档分发到各 AI 宿主的技能目录，AgentWatcher 要不要同类命令这次没问，v1 不做，已列进设计的未决问题。
