---
schema_version: "1"
protocol: "1.3.0"
artifact: "design"
artifact_id: "ar_53PD3ENZXZYMZ9TY0TFB1DB3BF"
work_item_id: "wi_0ZQ8FFAX6K2B4D8F66T96TFDEM"
created_at: "2026-11-23T03:17:35Z"
producer: "aes-brainstorm"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:377edb0dac11131577aa3523826b0b7accc7a012a8b0fef5fe9226ef0433ed54
  artifacts: []
result: "accepted"
---

# 设计：AgentWatcher 支持 Antigravity Provider

## 目标

Antigravity（Google 的智能体开发平台，本机已装）的会话出现在 AgentWatcher 卡片里：
按等回复 / 跑着 / 闲着出状态，悬浮预览显示工作区、标题、最后的用户输入和 AI 正文，
用量卡片带轮次、工具调用和 token 数据；CLI 的 session 命令同步支持。

## 背景：本机实测的数据布局

2026-11-23 在本机（Windows，Antigravity 已登录使用）逐项验证过：

- 数据根是 `%USERPROFILE%\.gemini\antigravity\`。`%APPDATA%\Antigravity` 只是 Electron
  壳的浏览器缓存，`%LOCALAPPDATA%\antigravity` 是安装目录，都没有会话数据。
- 会话清单在 `conversation_summaries.db`（SQLite）的 `conversation_summaries` 表：
  一行一个会话，带 `conversation_id`、`title`、`step_count`、`last_modified_time`、
  `workspace_uris`（JSON 数组，URI 编码的 `file:///` 路径）、`status`、`not_fully_idle`、
  `killed`、`last_user_input_time`、`project_id`。
- 会话正文在 `conversations\<conversation_id>.db`（SQLite，一个会话一个库）的 `steps`
  表，`step_payload` 是 protobuf 编码的二进制，不带 schema 文件。已用裸 wire-format
  遍历器解析成功，字段位置稳定：
  - 步骤类型 14 = 用户轮，正文在字段 19 的子字段 2（明文字符串，中英文都对）。
  - 步骤类型 15 = 模型轮，正文在字段 20 的重复子字段 3；工具调用在字段 20 子字段 7
    （工具名加 JSON 参数）；token 用量在字段 9；模型编号在字段 9 子字段 1。
  - 步骤类型 132 = 工具调用加结果（调用描述在字段 5 子字段 4，结果在字段 140）。
  - 步骤类型 17 = 模型报错（本机样本是 429 配额错误）。
  - 时间戳在字段 5（秒加纳秒两个子字段）。
- 会话级状态枚举（从安装目录 `language_server.exe` 的字符串表提取，写库方就是它）：
  `CASCADE_RUN_STATUS_UNSPECIFIED / RUNNING / IDLE / CANCELING / BUSY`。
  没有等回复那一档。
- 工具清单里有 `ask_question`（本机会话的工具授权列表里可见），结构上与 Claude / ZCode
  的 AskUserQuestion 同构；本机会话没有真实调用样本。
- 状态列会滞留：本机有一个会话因 429 配额错误停在 09:59，`status` 仍是
  `CASCADE_RUN_STATUS_RUNNING`。单看状态列会把死会话当成在跑，必须拿尾部内容时间戳兜底。
- `antigravity://` URL 协议已注册但只用于登录回调；社区实测按会话 ID 拼深链打不开，
  官方没有会话级深链文档。

## 非目标

- 点击卡片跳转：官方无会话深链，本轮返回明确的「暂不支持」提示（ZCode 先例），
  上游能力就绪后按 git 历史恢复。
- AI 建议回复、自动回复、跨引擎生成（含「用 Codex 生成建议」的上下文抽取器）：Antigravity
  没有发送通道，本轮不做（WorkBuddy 首轮同口径；跨引擎建议当时也是后续任务补的）。
- 权限审批等待：Antigravity 的工具审批走浏览器确认流程，`steps` 表有 `permissions`
  列但本机全部为空、无样本可验证，留给后续任务。
- 归档口径：清单表没有归档列（FTS 表的 `archived` 列本机恒为 0），`hideArchived`
  对 antigravity 不适用，上游出归档概念再接。
- macOS 布局验证：家目录布局按 `~/.gemini/antigravity` 同构假设写平台无关路径，
  实际验证归 mac 移植任务。
- 不发版、不改 Bridge、不改版本号。

## 方案对比

| 方案 | 做法 | 代价 | 结论 |
| --- | --- | --- | --- |
| 只读清单层 | 卡片只靠 `conversation_summaries` 表出标题、状态、工作区 | 预览正文和 token 用量拿不到（`preview` 列只是标题复读，FTS 表正文空窗），两条验收标准达不成 | 不选 |
| 清单层加正文库裸解析（选定） | 卡片层一次 SQL；预览和用量对 `steps` 表尾部采样，用一个约两百行的 protobuf wire-format 遍历器按字段号取值，不引 schema 依赖 | 要自己维护字段号映射；Antigravity 升级改字段时解析返回空、预览降级不崩 | 选这个 |
| 完整 protobuf 类型 | 逆向出 .proto 文件，用 prost 生成 Rust 类型再解析 | 新增编译依赖；schema 是逆向产物，上游一改就得重生成，重量级方案套在「读尾部几条」上 | 不选 |
| 什么都不做 | Antigravity 会话不进卡片 | 与用户要求直接冲突 | 不选 |

选第二个方案的核心理由：AgentWatcher 对 SQLite provider 的既有口径就是「尾部采样最近
N 行、不读全文」，裸遍历器只服务这个口径，字段号错位时缺哪项哪项是 null，不会把整个
provider 拖垮。

## 选定方案

### 标识与卡片

- provider 字符串 `antigravity`，徽标 `AG`，会话 ID `antigravity:<conversation_id>`
  （清单表主键，确定性拼接）。
- 工作区取 `workspace_uris` 数组第一项，percent 解码成 `F:\AiProject\...` 形式。
- 标题取 `title` 列。
- 跳过 `killed=1` 的行；`parent_conversation_id` 非空的子会话（battle 模式 / 智能体
  分支）跳过，与 ZCode 过滤 `subagent_child` 同构。

### 状态机

按优先级从高到低：

1. 等回复（waiting）：尾部采样里最后一个模型轮调用了 `ask_question` 工具，且它之后
   没有新的用户轮。与 Claude / ZCode 的 AskUserQuestion 判定同构。本机无真实样本，
   单元测试用构造夹具覆盖，人工核对清单留一条等真机验证。
2. 跑着（running）：`status` 列是 `RUNNING / BUSY / CANCELING`，且尾部内容时间戳在
   10 分钟新鲜窗口内（复用 `STATUS_RECENT_CONTENT_WINDOW_MS`）。超窗的 RUNNING 按
   闲着处理——状态列会滞留是本机实测到的坑。
3. 闲着（idle）：`IDLE / UNSPECIFIED`，或超窗的 RUNNING。

时间戳取尾部步骤的字段 5（秒加纳秒），不用 `last_modified_time` 列做新鲜度（那列的
更新粒度没有验证过，内容时间戳与其他 provider 口径一致）。

### 预览与用量

- 尾部采样最近 24 步（与 OpenCode/ZCode 的 SQLite 采样口径一致），扫描上限常量
  `ANTIGRAVITY_SESSION_SCAN_LIMIT_MAX` 对齐其他 SQLite provider。
- 最后用户输入：最后一条类型 14 步骤的字段 19 子字段 2。
- AI 正文：最后一条类型 15 步骤的字段 20 子字段 3 文本段拼接。
- 用量：用户轮次 = 类型 14 计数；模型调用 = 类型 15 计数；工具调用 = 类型 132 计数；
  token 逐轮从类型 15 的字段 9 累加；活跃时长按内容时间戳聚类（与其他 provider 同一
  5 分钟间隔口径）。token 桶和模型编号的语义见未决问题。
- 用量明细懒加载（`get_session_usage_detail`）：`topTools` 从尾部采样的工具名计数出
  （类型 15 字段 20 子字段 7 的工具名加类型 132 的调用名）。
- 正文库缺失（云端会话未同步到本机）时降级：卡片和状态照常出（清单表还在），预览与
  用量为 null。

### 接线位置（以 WorkBuddy/ZCode 为参照实测的改动面）

- Rust：`lib.rs` 新增一块 provider 实现区（WorkBuddy 的在 14960-15630，约 670 行）；
  另有 9 处接线：`ScanOptions` / `ResolvedScanOptions` 两结构、`resolve_scan_options`
  默认值、`session_watch_roots` 挂 `conversations` 目录加清单库（不挂 `daemon`/`presence`，
  避免秒级状态文件刷爆防抖，WorkBuddy 不挂根目录同理）、`scan_sessions_blocking`
  分发、全历史定向查找两处、`get_session_usage_detail` 分支、性能面板计数、
  `prepare_handoff_source_context` 的 antigravity 专属分支（导出 Markdown，见下）。
- 打开会话：`open_session` 加 antigravity 分支，返回「暂不支持跳转」的明确提示
  （ZCode `open_zcode_session` 同款提示模板）。
- 接续来源：照 ZCode 强口径——用抽取器把整个会话解码成用户/AI 正文 Markdown，落
  `%TEMP%\AgentWatcher\handoff-sources\antigravity\<会话ID>.md`（文件名 = 卡片会话 ID）。
  不能照 WorkBuddy 只给原始路径：WorkBuddy 的原始文件是 JSONL 文本可直读，Antigravity
  的是 protobuf 二进制。Antigravity 仅作接续来源、不作目标。
- 打包：`export_session_package` 把会话 db 与清单行 NDJSON 落 `raw/antigravity/`。
- CLI：`cli.rs` 帮助文本、`KNOWN_PROVIDERS`、`--provider` 映射、默认值，4 处；
  `session show --content` 按「其余 provider」分级给最后输入输出节选加原始 db 路径
  （OpenCode/ZCode 给导出路径的口径不适用——正文是二进制，节选由抽取器出）。
- 技能分发：`skill.rs` 宿主候选表加 `~/.gemini/config/skills`（宿主名 antigravity），
  `cli_contract.rs` 的宿主名单断言同步；本机该目录已有用户手动装的 agentwatcher
  技能副本，装过的副本不归 CLI 管照旧例处理。
- UI：设置开关、provider 筛选下拉、徽标与配色（中英文、dark/light）、性能面板计数串、
  显示名映射，共 6 组位置。
- 真实交互流程：`tools/tauri-realtest/cli.mjs` 注册 `antigravity-session` 流程
  （别名表、显示名、分发、流程函数各一条）。

## 与历史 provider 任务的对照

按用户要求对照了 ZCode（`workflow/agentwatcher-zcode-provider`）与 WorkBuddy
（`workflow/AgentWatcher-支持-WorkBuddy-Provider`）两个任务的验收标准和 AGENTS.md 口径，
初版设计漏了五项，本版已补：

| 历史规范 | 出处 | 初版状态 |
| --- | --- | --- |
| 接续来源导出可读 Markdown 到 `handoff-sources\<provider>\`，仅作来源不作目标 | ZCode AC-003 / AC-005 | 漏了。初版照 WorkBuddy 弱口径只给原始路径，但 WorkBuddy 原始文件是 JSONL 文本、Antigravity 是 protobuf 二进制，该照 ZCode 强口径 |
| CLI `session show --content` 内容分级（agent-cli 任务拍板：其余 provider 给最后输入输出节选加原始文件路径） | AGENTS.md「内容分级」节 | 未写明，已补 |
| 技能分发宿主候选表（WorkBuddy/copilot 都是后补进 `skill.rs` 的，`cli_contract` 有名单断言） | skill.rs 宿主表 + cli_contract.rs | 漏了。本机 `~/.gemini/config/skills` 已有用户手动装的 agentwatcher 技能副本，是现成依据 |
| 扫描上限常量与 provider 配额（如 `WORKBUDDY_SESSION_SCAN_LIMIT_MAX`） | WorkBuddy 实现块 | 未写明，已补 |
| 用量明细懒加载的 `topTools` | `get_session_usage_detail` 返回结构 | 未写明，已补 |

照旧核过没漏的：性能面板计数（ZCode AC-006）、点击明确提示（ZCode AC-002）、打包
raw 口径、双语言双主题、文档同步、watch roots 防抖（不挂有秒级心跳文件的目录）、
macOS 平台路径假设。

## 边界与失败

- 数据根不存在：静默返回空列表，未装 Antigravity 的机器零工作。
- SQLite 只读打开（mode=ro，兼容 WAL），不连 daemon 端口、不拉起任何进程。
- protobuf 字段错位（上游升级改号）：遍历器对取不到的字段返回 None，预览降级为空，
  不崩、不影响其他 provider。
- 会话 db 与清单表不一致（一边删了一边没删）：以清单表为卡片正本，正文侧缺就降级。
- 中文与空正文：字段 19 子字段 2 是 UTF-8 明文，直接展示；空串按其他 provider 的
  空态处理。

## 影响面

改动规模的查数命令与数字（2026-11-23 在 dev 基线 6d0aca0 上测得）：

| 项 | 数字 | 复算命令 |
| --- | --- | --- |
| lib.rs 既有 `ScanOptions {` 字面量要补字段 | 约 20 处 | `grep -c "ScanOptions {" src-tauri/src/lib.rs` |
| lib.rs WorkBuddy 参照实现块 | 约 670 行 | 看 14960-15630 |
| ui/index.html 里 workbuddy 接线位 | 6 组约 30 行 | `grep -n workbuddy ui/index.html` |
| cli.rs / commands.rs 接线位 | 4 处 | `grep -n workbuddy src-tauri/src/bin/agentwatcher-cli/*.rs` |
| 文档 | AGENTS.md 约 12 处、README 7 处、CHANGELOG 1 条 | `grep -n -i workbuddy AGENTS.md README.md CHANGELOG.md` |
| 新增真实流程测试 | cli.mjs 3 处注册加约 60 行函数 | 看 `runWorkBuddySessionFlow` |

新增 Rust 代码预计 700 行上下（实现块加测试夹具），无新第三方依赖。

## 怎么算做对

对齐任务的八条验收标准：cargo test 全绿（含 antigravity 夹具测试）、真实 Tauri 流程
出现 AG 卡片、预览与数据源一致、设置与筛选双语言双主题、点击返回明确提示、用量可为
null 的项为 null、文档与 CLI 契约测试同步（含技能宿主表断言）、接续面板能读到
`handoff-sources\antigravity\` 的 Markdown 导出。人工核对清单留两条：真机上 Antigravity
问问题时卡片状态是等回复；429 配额停掉的会话在超窗后变闲着。

## 未决问题

1. token 桶语义：模型轮字段 9 里有 35457 / 97 / 9587 / 26 一组数，哪个是输入、输出、
   缓存、思考没有官方口径。选项：实现期对着 Antigravity 界面里的用量显示逐项校准；
   校准不了就先只展示轮次和工具数。下一步：实现时校准，错也只影响显示口径。
2. 模型编号映射：字段里是 1402、1418 这类枚举值，没有名字表。先显示空或原值，
   映射表等官方口径。下一步：实现期处理，不阻塞。
3. `ask_question` 等回复判定没有真实样本。选项：照结构推断上线（构造夹具测试加
   人工清单验证），或先不判等回复、全部按跑着或闲着。建议前者，结构与 Claude / ZCode
   同构，风险低。下一步：等用户真机遇到时人工核对。
4. 子会话过滤口径（`parent_conversation_id` 非空跳过）同样无样本。建议照推断上线，
   人工清单验证。
5. 技能宿主目录 `~/.gemini/config` 可能与 Gemini CLI 共享（同目录还有 `config.json`、
   `sidecars`）。宿主名先叫 antigravity；若实现期发现 Gemini CLI 也会读同一目录，
   宿主名与说明文案再议，目录本身不变。
