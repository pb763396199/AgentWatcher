---
schema_version: "1"
protocol: "1.3.0"
artifact: "design"
artifact_id: "ar_35YPMQD4YZVPHVKV26G13Q0D5T"
work_item_id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
created_at: "2026-09-16T03:13:00Z"
producer: "aes-brainstorm"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:32abb9b77345e1c4412d7442f9ae353d1bca154ec66037eeaf403b276872b2ea
  artifacts:
    - artifact_id: "ar_6NMBYQFZVZY8AXPPP9GA39CPMA"
      digest: sha256:0e8ff2b6658c27f01bfb607d719b1cb92485204991d3093add76b15e0d390885
      locator: "research/interview-research.md"
    - artifact_id: "ar_4FPTMQVXYVBPRZ28NJ33D9EJ5B"
      digest: sha256:ad38a4b1ca883b8c6d43874d0b029883e278bcc0fdeda6cdc6ad7b206494a2da
      locator: "research/udf-cli-architecture-research.md"
result: "accepted"
---
# 设计：AgentWatcher AI 操作 CLI

## 要解决什么

AI agent 现在没有任何受控途径读 AgentWatcher 扫出来的会话。六 个 provider 的原始数据散在 JSONL 和 SQLite 里，AI 直读要自己处理六种格式，还绕过了状态机（waiting 判定）、跳过哨兵过滤和噪音过滤这三层已经写好的逻辑。本设计给 AI 一条命令行通道：不启动 GUI，就能查会话元数据、按需取正文、导出接续上下文。

## 定了什么

访谈六个拍板点的结论（证据和被否选项见 `research/interview-research.md`）：

| 拍板点 | 结论 |
| --- | --- |
| 首要消费者 | AI agent，JSON 信封输出一等公民 |
| 大类 | session（查询）+ handoff（导出），共两个 |
| 进程 | 独立 headless 二进制，直读数据源，不连 GUI |
| 正文 | 列表默认只给元数据，正文显式请求 |
| handoff 边界 | v1 仅导出，派发给后续版本 |
| 分发 | `agentwatcher-cli.exe` 进发布 zip |

## 命令面

```
agentwatcher-cli [全局参数] <大类> <动词> [参数]

全局参数：--format json|human（默认 json）、--version
```

| 命令 | 做什么 | 复用哪个现有函数 |
| --- | --- | --- |
| `session list` | 会话元数据列表。筛选参数与 GUI 设置一致：`--provider`（可多选）、`--status waiting\|running\|idle`、`--workspace <路径>`、`--limit <n>`（默认 80）、`--active-days <n>`（默认 7，夹在 1 到 30） | `scan_sessions_blocking`（lib.rs:942） |
| `session show <id>` | 单会话详情：状态、标题、摘要、工作区、时间，与 GUI 悬浮预览同级；`--content` 才带正文全文 | 同上，再按 id 过滤 |
| `session usage <id>` | 用量明细：token、上下文、轮次、工具调用、模型、成本，附 topTools | `session_usage_detail`（lib.rs:1798，命令壳拆分后的 pub 实现函数） |
| `handoff export <id>` | 导出源会话上下文 Markdown，落盘位置和文件名规则与 GUI 接续面板一致 | `prepare_handoff_source_context_blocking`（lib.rs:2335） |
| `skill install [--dir <目录>]` | 把内嵌的 AgentWatcher 用法 SKILL.md 装进 AI 宿主技能目录；默认探测 `.zcode` / `.claude` / `.agents` / `.codex` 下的 `skills` 和 `.config/opencode/skill`，只装目录已存在的宿主；重复执行覆盖升级 | SKILL.md 以 `include_str!` 内嵌进二进制 |
| `skill list [--dir <目录>]` | 报告各候选目录状态：`installed`（内容一致）/ `outdated`（内容过期）/ `not-installed` / `host-absent`（宿主目录不存在） | 同上 |
| `skill remove [--dir <目录>]` | 删除 `agentwatcher` 技能目录，幂等 | 同上 |

动词词表：`list` 列元数据、`show` 看单个、`usage` 查用量、`export` 导出落盘；skill 组（2026-09-16 补充拍板）再加 `install` / `remove`。跨命令的动词语义和帮助文本一致性测试照 udf 的做法立起来——`list` 在 session 和 skill 两组里都是「列出资源」，语义一致。

会话 ID 可以跨命令引用：ID 是「provider 名加自然键」的确定性拼接（如 `opencode:<row.id>`），跨进程、跨次扫描稳定。AI 先 list 拿 ID，再用 ID 做 show / usage / export。

## 输出契约

JSON 模式下一条命令只往 stdout 输出一个信封文档，其余什么都不写：

```json
{
  "command": "session list",
  "ok": true,
  "data": { "sessions": [], "total": 0 },
  "error": null,
  "messages": []
}
```

- 字段名 camelCase；`messages` 收进度和警告文本，解决人读输出污染机器解析的问题。
- 失败也走信封：`ok=false`，`error` 带 `code`（如 `session_not_found`）和 `message`。
- 退出码：成功 0；命令失败（含 ID 不存在）1；用法错误 2（clap 默认）。空结果不是失败：空列表 `ok=true`、`total=0`。某个 provider 没装（数据源目录不存在）也不是失败，返回该 provider 零会话。
- human 模式输出表格，作为次要模式存在，测试断言它不混进 JSON 模式的输出。

## 架构

- 新增二进制：`src-tauri/src/bin/agentwatcher-cli/`（目录形式 bin，main.rs 加 cli 定义、commands、output 模块），`Cargo.toml` 加一个 `[[bin]]` 和 clap 依赖。
- 复用方式：`scan_sessions_blocking` 和 `prepare_handoff_source_context_blocking` 直接改 `pub`；`get_session_usage_detail` 带 `#[tauri::command]` 宏，宏与 `pub` 不兼容，拆成命令壳加 pub 实现函数 `session_usage_detail`。CLI 直接调用。`AgentSession`、`SessionUsageDetail`、`ScanOptions` 本来就是 pub 且带 Serialize，CLI 的 JSON 输出直接序列化这些类型，不造第二套字段。
- clap 加进 `[dependencies]` 后 GUI 和 CLI 共享依赖树。判断：Tauri 项目依赖数量本来就大，clap 4 的编译增量在秒级，共享依赖树的代价小于拆 workspace 的结构改动。

## 方案对比

| 方案 | 怎么做 | 代价 | 为什么没选 |
| --- | --- | --- | --- |
| 独立 bin 复用 lib crate（选定） | 上面写的架构 | 发布 zip 多一个 exe；依赖树共享 clap | — |
| GUI exe 加 `--cli` 子入口 | `agentwatcher.exe --cli …` 复用同一 exe | 少一个发布文件 | 进程入口混进 GUI 依赖面，Tauri 初始化路径要分叉，出错面更大 |
| MCP server | stdio 协议的 Model Context Protocol 服务 | 目标消费方是 AI 时更标准 | 用户拍板做 CLI 参考 udf；MCP 留作后续选项 |
| 什么都不做 | AI 直读 JSONL / SQLite | 零改动 | 绕过状态机和过滤，六种格式各自处理，正是本任务要消掉的负担 |

大类结构对比：noun-verb（`session list`，选定，udf 同款）对 agent 的可预测性最好；verb-noun（kubectl 风格 `awc get sessions`）在命令多时才显优势；扁平单层（`awc sessions`）最短但第二版扩类就要重构命令面。

## 隐私边界

- 三档信息分级与 GUI 现状一致：list 给卡片级元数据（状态、标题、工作区、provider、时间）；show 默认给悬浮预览级（摘要、最后用户输入，无全文）；`--content` 给 AgentWatcher 已有的最深内容视图——OpenCode / ZCode 返回完整 transcript Markdown（复用接续导出的同一条链路），JSONL provider 返回最后输入输出节选加原始会话文件路径（GUI 今天也不渲染 JSONL 全文，AI 要全文可按路径直读原始文件）。
- 信任模型（访谈拍板）：本机 AI 进程可信。AI 本来就能直读原始 JSONL / SQLite，CLI 不扩大边际暴露面；按需输出挡住的是误操作倾倒，不是敌意进程。
- `handoff export` 落盘 `%TEMP%\AgentWatcher\handoff-sources\`，与 GUI 同一位置同一命名规则，不新增落盘点。

## 场景走查

- 顺利：AI 执行 `session list --status waiting`，从 `data.sessions` 拿到 ID；`session show <id>` 判断值不值得介入；`session show <id> --content` 取最深内容视图；`handoff export <id>` 拿导出文件路径转给下一个会话。
- 出错：ID 拼错时 `ok=false`、`error.code=session_not_found`、退出码 1，AI 可据 code 分支重试 list；JSON 模式下错误也在信封里，stdout 永远只有一个可解析文档。
- 中断后回来：查询无状态，重跑即可；export 每次落一个带时间戳的新文件（文件名规则与 GUI 相同：`自然键-毫秒时间戳.md`），重复执行产生新文件而不是冲突。

## 影响面

动手前用 grep 复算过：

| 改动 | 数字 | 复算命令 |
| --- | --- | --- |
| `lib.rs` 当前行数 | 13438 行 | `wc -l src-tauri/src/lib.rs` |
| 需要改 `pub` 的函数 | 3 个（942、1798、2335 行） | `grep -n "fn scan_sessions_blocking\|fn get_session_usage_detail\|fn prepare_handoff_source_context_blocking" src-tauri/src/lib.rs` |
| 现有 `scan_*` 私有函数（不动，经三个入口间接复用） | 16 个 | `grep -c "fn scan_" src-tauri/src/lib.rs` |
| `package-exe.mjs` 里现有 exe 引用点 | 1 处，要加 CLI exe 的拷贝与打包 | `grep -c "agentwatcher.exe" scripts/package-exe.mjs` |
| 新增文件 | bin 目录（入口、命令模块、输出模块）+ 集成测试 | — |
| 现有测试 | 纯增量，无修改 | `cargo test` 现状基线 |

另有一处文档事实：AGENTS.md 写 `lib.rs`「4678 行左右」，实际已 13438 行，AC-007 同步口径时一并修正。

## 边界与失败

- CLI 读不到 GUI 的内存态（watcher 防抖、插件运行时状态）。这两个状态都能从文件推导，且 v1 不做插件命令，不构成缺口。
- 扫描是全量冷启动（GUI 同款逻辑，六 provider 采样读，不读全文），单次查询耗时与 GUI 刷新一次同量级。AI 高频轮询的成本靠调用方自律，v1 不做缓存。
- 无副作用承诺（执行中发现的真实约束回填）：GUI 的 Codex 扫描会拉起常驻 app-server（cmd→node→codex 进程链），该链会继承 CLI 的 stdout 管道写端，导致管道读者永远等不到 EOF，且 app-server 在 CLI 退出后变成孤儿进程。CLI 进程启动时禁用 app-server（Codex 会话走 `~/.codex/sessions` 本地文件兜底扫描），并清掉自身 stdio 句柄的继承标志。
- Windows 文件名不允许冒号，`handoff export` 的文件名用会话 ID 的自然键部分（GUI 现有规则，直接复用）。

## 怎么算做对

对照 work-item 的 AC-001 到 AC-007：构建产物（001）、大类与词表测试（002）、信封契约测试（003）、正文按需输出测试（004）、导出与 GUI 落盘一致（005）、测试全绿（006）、文档口径同步（007）。

## 未决问题

1. ~~skill 分发命令~~ 已定（2026-09-16 用户拍板并入本任务）：做 `skill install / list / remove`，SKILL.md 内嵌二进制、幂等覆盖升级；自动化测试只经 `--dir` 打临时目录。与 udf 的差异：不做 workspace init 顺手安装（AgentWatcher 没有 workspace init 命令）。
2. 信封加 `nextCommand` 指路字段：命令面还小，v1 不加。扩类后命令超过十个再评估。
3. human 模式的表格排版细节（列宽、颜色）：实现时定，不影响契约。
