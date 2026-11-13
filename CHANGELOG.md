# Changelog

## 未发布

- 修 macOS 会话标题全是原始 prompt 的问题（两个叠加根因）：
  - Mac 上没有独立 codex CLI，GUI/CLI 全走 rollout 文件扫描；文件扫描的注入过滤器只认 4 种旧形态，Codex 桌面端新版塞进 user 轮的 `<external_browser_tabs>`、`<codex_internal_context>`、`<skills_instructions>`、`<multi_agent_*>` 等命名空间注入块全部漏过、顶成了标题。过滤器名单按 2026-11 双端 rollout 实测频次扩充到 18 项（`CODEX_CONTEXT_INJECTION_PREFIXES`），标题/首条消息/最新预览共用同一过滤。
  - 超长 prompt 兜底标题被 80 字硬截成半句话。新规则：超过 40 字符的标题在第一个不早于 12 字的句读（中英文标点，不含空格）处收口并剥掉尾部标点；无句读的长串（URL 等）保持原样。
  - 单测覆盖：注入名单正/反例（真实用户贴的 `<html>`/`<class` 代码不误杀）与句读收口；lib 179 全绿，CLI 分类 19 + 契约 13 与真实 Tauri smoke 通过。
- GUI（Tauri 桌面壳）移植 macOS：`cargo check --features gui` 在 macOS 零错误零警告，`npm run dev` 起真窗口，`tauri build` 产出未签名 `AgentWatcher.app` 与 aarch64 `.dmg`（本机侧载可用，未做签名/公证）。
  - 运行时等价：URL scheme 深链（vscode://、workbuddy://、codex thread）macOS 走 `open`（原 rundll32）；opencode-cli handoff 的可见终端走 osascript `tell application "Terminal" do script`；会话打包的文件剪贴板走 osascript `POSIX file`（Finder 可 Cmd+V，失败仍回退 reveal）。Windows 行为不变。
  - 透明无边框窗口在 macOS 启用 `macos-private-api`（tauri.conf.json `app.macOSPrivateApi` + Cargo feature 对齐声明）。
  - 图标：`generate-icons` 同一 GDI+ 路径追加输出 1024 PNG 源图，`tauri icon` 生成 `icon.icns`，`bundle.icon` 补全（Windows 手绘 15 尺寸 ico 保持不变）。
  - 性能诊断面板 macOS v1 空列表降级（进程采样维持 Windows 专属）；`windows-sys` 显式声明 `Win32_Security` feature（不再依赖传递 feature union）。
  - 验证：macOS（M1 Pro）dev 窗口与 .app 启动均经 CGWindowList 确认在屏；Windows 侧 lib 177 + CLI 分类 19 + 契约 13 + 真实 Tauri smoke 全绿。macOS 无 CDP，像素级效果留人工验收。
- AgentWatcher 技能装入 Mac 上的 ChatGPT：ChatGPT 桌面版（26.x）内嵌 Codex Framework，读 `~/.codex/skills/`（并参考 `~/.agents/skills`）；`agentwatcher-cli skill install` 已把 SKILL.md v3 装入 Mac 的 `~/.codex`、`~/.agents`、`~/.workbuddy` 三处宿主目录。
- agentwatcher-cli 移植 macOS（第一步，GUI 仍为 Windows 专属）：
  - Cargo 结构跨平台化：`winreg` / `windows-sys` / `junction` 移入 `cfg(windows)` 目标依赖，新增 Unix 侧 `libc`；Tauri GUI 壳挂到 `gui` feature（默认开启，Windows 发布链路不变）。macOS 用 `cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --bin agentwatcher-cli` 构建 headless CLI，不要求 ui-dist 前端产物；`cargo check --no-default-features` 同时成为「CLI 永不依赖 GUI」的回归门。
  - 数据源路径跨平台：`USERPROFILE` → `$HOME`、`APPDATA` → `~/Library/Application Support`（VS Code 用户数据、AgentWatcher 自身状态共用该根）；`.codex` / `.claude` / `.local/share/opencode` / `.zcode` / `.workbuddy` 相对布局各平台一致。Windows 侧路径解析优先级不变，返回值与历史版本逐字节一致。
  - Unix 真实现替换桩：CLI 全历史扫描的跨进程缓存锁用 `flock(LOCK_EX|LOCK_NB)` 对齐 Windows `LockFileEx` 语义；app-server / codex exec 子进程树用独立进程组（`process_group(0)` + `kill(-pgid)`）对齐 Job Object kill-on-close；opencode server 端口探测用 `lsof` + `pgrep`（Windows 保持 `netstat.exe` + ToolHelp）。
  - CLI 发现层补 macOS 候选：codex / opencode / node / VS Code code 的 `/opt/homebrew`、`/usr/local`、`~/.npm-global`、`/Applications/Visual Studio Code.app` 包内 bin，沿用「逐个 `--version` 实测才认」。
  - OpenCode `session` 表 schema 容忍层：1.2.x → 1.19.x 陆续加的 `path` / `cost` / `tokens_*` 列按实际列集构造 SELECT，老库（或装了新版但尚未启动迁移的库）缺列用 NULL / 0 占位，扫描不再整体失败；sqlite 打开 / prepare / 行读取失败输出分类 stderr 日志（此前静默返回空列表无处排查）。
  - 验证：Windows `cargo test` 全量（lib 177 + cli 分类 19 + 契约 13）与 `npm run test:tauri:smoke` 通过；macOS（arm64, 27.2）`cargo test --no-default-features` 全绿（lib 171 + CLI 40，另有 6 个 Windows 专属测试按平台门控），真实数据源冒烟覆盖 codex / opencode / workbuddy 的 list / show / usage / handoff export，flock 并发合并经时间戳实证生效。
- 新增「自动回复」：会话停止工作后，程序自动生成一条回复并发回会话。
  - 与 ZCode 的「自动作答」是两件独立的事——自动作答只在 AskUserQuestion 的结构化选项里挑一个（限 ZCode），自动回复是把生成的建议原文发回会话，适用于所有有发送通道的 agent。
  - 开关按会话（卡片右键「自动回复此会话」，`set_auto_reply` / `get_auto_reply`），只有用户显式开启的会话才生效，默认全关。
  - 触发条件是「会话真的停过工」：`running` 或 `waiting` 转 `idle`。冷启动首见的 idle 不算——那不是刚干完活，是程序刚开机。
  - 链路：扫描识别状态转换 → 后台线程用**会话自己的 agent** 生成建议（不跨引擎，回复风格不能跑偏）→ 复核指纹仍然新鲜（会话可能又前进了）→ 按 provider 路由到 codex / zcode / opencode 的发送通道 → 审计写 `%TEMP%\AgentWatcher\auto-replies\audit.jsonl`（含引擎、原文、结果）。
  - 护栏与自动作答同构：每小时 6 次、会话冷却 2 分钟、连续两次同回复熔断且需用户手动发过一条才解除。冷却未满不算拒绝，而是延后到点再发，避免用户刚开开关就错过窗口。
  - provider 覆盖 `codex / zcode / opencode`：workbuddy 能生成建议但没有发送通道，claude/copilot 两者都没有。因此状态跟踪扩展到所有支持发送的 provider，但「建议的自动生成」仍然只对 zcode/codex 生效，避免对全部 opencode 会话刷模型。
- 补齐此前遗漏入库的文件：三份工作流文档（ZCode app-server 发送通道、ZCode 会话 AI 建议回复与自动作答、AgentWatcher 支持 WorkBuddy Provider）、`docs/insights` 下的 ChatGPT 桌面端 imagegen 生图进度条逆向报告。
- 清理未跟踪文件：`.gitignore` 排除 `.workbuddy/`（agent 私有记忆）与 `.aes-workflow/`（嵌套 git worktree 副本，二者都不是项目源码）；删除 `src-tauri/src/bin/agentwatcher_dev_launcher.rs`——这是 7 月的旧副本，Cargo 自动发现机制把它当成 bin target 一直在编译，但无人引用，现行版本是已跟踪的 `tools/agentwatcher-dev-launcher.rs`。
- OpenCode 补齐三条链路：跟随会话生成、跨引擎生成、发送回复。此前只有「能扫到会话、能导出接续上下文」，建议与发送两个能力对 OpenCode 一律关闭——理由是「会话存在 sqlite 里，没有可读的 JSONL 转写」。这个结论是错的：`message` + `part` 两张表本来就能还原完整对话，handoff 导出的 `read_opencode_transcript_messages` 早就在读同一份数据。
  - 上下文抽取 `opencode_suggestion_context`：从 `~/.local/share/opencode/opencode.db` 读 `message` + `part`，只取 `type == "text"` 的部件（滤掉 tool / reasoning / step-*，否则工具调用噪声会把用户意图带偏），取尾部 4 轮并保证至少含一条 user 轮。OpenCode 的 agent 会连发几十条 assistant 消息，用户的原始意图早被挤出尾部窗口，因此额外用 `json_extract(data,'$.role')` 单独捞最近一条 user 轮补进来。
  - 跟随会话生成 `generate_opencode_suggestion`：走 `opencode run --format json --dir <scratch>`。`--dir` 指向空 scratch 目录，避免模型在原工作区里动文件；**不能用 `--session/--fork`**，那会往真实会话写消息。解析器 `parse_opencode_run_output` 先试 JSON 信封、再拼事件流里的 text 部件、最后兜底宽松取行。
  - 跨引擎生成：`--via codex` 现在支持 OpenCode 源会话（此前因为读不到上下文而明确失败）。
  - 发送 `send_opencode_message`：优先 POST 运行中 opencode server 的 `/session/<id>/prompt_async`（204 立即返回，不等模型跑完），无 server 时退回 CLI `opencode run --session <id>`。实测 server 端点返回 204、消息落进会话、模型正常应答。
  - 前端：会话卡片右键对 OpenCode 开放「生成 AI 建议回复」与「用 Codex 生成」，发送按钮走 `send_opencode_message`。
- 修 codex CLI 路径发现：`codex_cli_path()` 原来只认 `%APPDATA%\npm\node_modules\@openai\codex\bin\codex.js` 一个位置，但 npm 的全局前缀不唯一（装 npm 本体的 prefix、WorkBuddy 自带 node 的 prefix、用户自定义 prefix 都可能是落点）。现在 AppData 与 `~/.workbuddy/binaries/node/versions/*` 下的 prefix 全部探测，取第一个命中的。
- 修 CLI 全历史扫描缓存的跨进程并发合并：`FileLock::acquire` 原来用 `open(...).ok()?`，而 Windows 上文件被独占锁定时再次 open 会直接返回「拒绝访问」，于是竞争进程把「打不开文件」误判成「等锁超时」而放弃合并、各自真实扫描一遍（`test_concurrent_full_history_scan_merges` 长期失败，counter 为 3~4 而非 1）。现在 open 失败与锁冲突一样按「锁被占」轮询重试，超过等待上限才降级为直接扫描。4 进程并发回归通过。
- 修 opencode CLI 发现：npm 全局装的 `opencode-ai` 常因跳过 postinstall 而损坏（`bin/opencode.exe` 是占位脚本），且平台包二进制可能下载截断（PE 节表声明 88MB 而文件只有 62MB，Windows 报「不是有效的 Win32 应用程序」）。`find_opencode_cli_path` 增加旁路目录探测（`~/.workbuddy/binaries/opencode/...`、`~/.local/bin`、`~/.opencode/bin`），逐个候选实测 `--version` 才认；缺失时错误提示直接给出 postinstall 修法。
- WorkBuddy playground 会话默认纳入卡片：`workbuddy_candidate_rows` 里 `is_playground` 从写死排除改为受第 4 个绑定参数控制（默认 true）；GUI 设置新增「WorkBuddy playground sessions」复选框（默认勾选），CLI `session list` 新增 `--exclude-playground` 回到旧视图，全历史扫描缓存键带上该开关。无工作区、软删、活跃窗口、路径黑名单、每源配额五层过滤不变；本机 11 条 playground 会话里 6 条有工作区可见，另外 5 条空工作区旧行仍排除。
- 顺手删掉 `workbuddy_package_entries_use_session_path_as_raw_jsonl` 重复的 `#[test]` 标注：两个实例并发写同一临时文件会让全量测试偶发权限错误。
- 同步文档口径：`skill/SKILL.md`（内嵌用法技能）、`README.md`、`AGENTS.md`；升级后重跑一次 `agentwatcher-cli skill install` 可刷新宿主里的技能副本。
- 技能分发补 WorkBuddy 宿主：`skill.rs` 的候选目录表加入 `~/.workbuddy/skills`，此前只覆盖 ZCode / Claude Code / Codex / 通用 agents / OpenCode，WorkBuddy 的全局技能目录一直没被写入过。内嵌技能文档的 provider 清单与归档口径同时补上 WorkBuddy（`deleted_at` 软删即其归档口径），`--include-archived` 帮助文本同步。

验证：`cargo test --manifest-path src-tauri/Cargo.toml`（lib 148 + CLI 分类 11 + 契约 10）、`npm run test:tauri:flow -- workbuddy-session`、`npx tauri build --no-bundle` 均通过。发布包名与 SHA256 待发版时补。

## v0.1.11 - 2026-11-06

- 新增 ZCode 会话 AI 建议与自动作答：ZCode 会话新进入等待时，后端用 ZCode 自身无头会话（`zcode.cjs -p`，零配置，模型与账号沿用现有配置）生成一条不超过 120 字的建议回复；建议显示在悬浮预览窗顶部常驻回复框（可手打字、可 AI 填充、可一键发送），检测到结构化 AskUserQuestion 时升级为选项芯片。发送走 `send_zcode_message`（注册表反解 ZCode.exe，`ELECTRON_RUN_AS_NODE=1` 运行 `zcode.cjs --resume`，实测桌面版 3.16.3 可用）。全自动作答按会话开关（卡片右键），仅限选项域，护栏内置：每小时 6 次、同答案熔断、审计写 `%TEMP%\AgentWatcheruto-answersudit.jsonl`。设置面板新增 AI 建议区与全局热键；建议正文只走事件不进 localStorage。
- 新增 Codex 会话 AI 建议回复：与 ZCode 同构的建议管线覆盖 Codex 会话（真实交互流程 `test:tauri:flow` 断言随任务合入）。
- CLI `session list` 全历史扫描跨进程合并缓存：带全历史参数（`--active-days 0` 或 `--include-archived`）时走 `%TEMP%\AgentWatcher\cli-scan-cache\` 的独占文件锁 + 120 秒 TTL 缓存，同参数并发/紧随调用只真实扫描一次（本机 17297 会话实测：冷扫描 59.5s、命中 0.76s、四进程并发总耗时 66s 且结果一致），修复多 agent 并发全历史查询把磁盘 I/O 成倍叠加卡死机器的问题；`--no-cache` 绕过，默认 7 天窗口不缓存，缓存损坏静默降级直扫。
- 依赖钉回 v0.1.10 基线：Cargo.lock 中 tauri 系 crate 曾被顺带升到 2.12.x，与 npm `@tauri-apps/api` 2.11 不匹配导致打包失败；本次发布把 tauri 系锁回 2.11.2（tauri-build 2.6.2、runtime 2.11.2、utils 2.9.2）。

发布包：`AgentWatcher-v0.1.11-windows-x64.zip`

SHA256：`E089EC9A2DCA795DC863D630A00C8415091F9492C3CF3101481240F0DFABF602`

验证：`cargo test --manifest-path src-tauri/Cargo.toml`（lib 146 + CLI 单元 4 + 契约 9 + 分类学 10 全通过）、`npm run test:tauri:flow -- zcode-session`（真实 Tauri 壳：切中文、点真实 ZCode 卡片通过）、`npm run build:ui`、Bridge 语法与 handoff 路由测试、`git diff --check`、`npm run package:exe`；最终 VSIX 重复安装两次仅稳定 ID `agentwatcher.agentwatcher-vscode-session-bridge@0.1.12`（无任何 safe*）；发布目录 EXE 独立启动实证；CLI 并发合并/缓存命中/故障回退以单元测试与真实二进制计时锁定。已知限制：ZCode 建议生成的模型档位取决于用户现有 ZCode 配置；缓存命中与真实扫描的输出在 f64 字段（如 costUsd）末位存在 1 ulp 十进制表示差，语义等价。

## v0.1.10 - 2026-10-30

- 新增 WorkBuddy 桌面端会话来源（第 7 个 provider，徽标 WB）：只读扫描本机 `~/.workbuddy/workbuddy.db`（SQLite，跳过已删除、playground、无工作区的行）与 `projects\<工作区>\<会话 id>.jsonl` 正文，按待回复/运行中/闲置出卡片；悬浮预览带最后用户输入与 AI 正文（过滤身份注入块）；用量带 token 求和、上下文占用、credits（数据源没有的项为 null）。
- WorkBuddy 点击跳转走其注册的 `workbuddy://chat/<会话 id>` 会话级深链（会话 ID 百分号编码）；右键打包交接同样可用（正文 JSONL 原样进包）；文件监听挂 `projects` 与 `workbuddy.db/-wal`（不挂根目录，根下秒级心跳文件会刷爆防抖）；CLI `--provider workbuddy` 同步可用，显式查询绕开共享用量预算直读。
- 新增逐轮降智审计：会话分析面板与悬浮预览可见每一轮是谁在回答、什么档位、花了多少、有没有换模型或重试（四类信号），用量时间轴上模型切换与异常轮有标记；右键打包的交接包新增 `audit/turns.ndjson`，接手方无需 AgentWatcher 即可核对。
- 新增真实交互流程 `npm run test:tauri:flow -- workbuddy-session`；WorkBuddy provider 单测 14 条、逐轮审计单测随任务合入。

发布包：`AgentWatcher-v0.1.10-windows-x64.zip`

SHA256：`714FFA1C5E05BB07CE89CCDDF7AD1025AC1AEF3804C2C80D65EA9BDA09D47098`

验证：`cargo test --manifest-path src-tauri/Cargo.toml`（140 lib + CLI 分类 9 + 契约 8 全通过）、`npm run test:tauri:flow -- workbuddy-session`（真实 Tauri 壳：卡片渲染、深链点击 toast 实证）、Bridge 语法与 handoff 路由测试、`git diff --check`、`npm run package:exe`；本机 9 条真实 WorkBuddy 会话卡片与 CLI 查询实证（含中文工作区路径）。已知限制：WorkBuddy 的 waiting 判定（AskUserQuestion 类工具未闭合）本机暂无真实样本，以合成事件流单测锁定；深链落点、浅色主题徽标等 4 项人工核对进行中。

## v0.1.9 - 2026-10-27

- 新增「打包会话用于跨电脑交接」：右键会话卡片把整个会话打成单个 zip 交接包——`manifest.json`（逐文件 SHA256；工作区是 git 仓库时附 repo/分支/HEAD/未提交数/未推送提交数锚点）、`HANDOFF.md`（纯定位块，含一行代码锚点与隐私提醒，不带正文）、`README.md`（数据格式说明）与 `raw/` 原始内容全量。落 `%TEMP%\AgentWatcher\session-packages\` 并把 zip 文件本身写入系统剪贴板（CF_HDROP），在 IM/资源管理器 Ctrl+V 直接粘贴出发送；剪贴板被占用时退回显示完整路径并揭示所在文件夹。
- raw 全量口径：Claude/Copilot 原样 JSONL（Copilot 另带 transcripts 覆盖文件；路径缺失时按会话 id 在各自目录重定位）；Codex 带同 thread 的全部 rollout 片段，`archived_sessions` 的 `.jsonl.zst` 解压进包（app-server 侧无本地转录的 `aw-` 线程如实报错）；Copilot CLI/OpenCode/ZCode 从 SQLite 全量导出 session/message/part（或 turns）行为 NDJSON。
- temp 产物 7 天清理：打包与接续源导出收尾时顺手删除 `%TEMP%\AgentWatcher` 下 `session-packages\` 与 `handoff-sources\` 中超期旧文件（只删文件不删目录，失败忽略）。
- 新增真实交互流程 `npm run test:tauri:flow -- 打包`（跳过静态占位卡、无本地转录的 Codex 卡自动回退）；新增 5 条单元测试覆盖六 provider 收集、manifest/SHA256 一致性、git 锚点、清理与错误路径。

发布包：`AgentWatcher-v0.1.9-windows-x64.zip`

SHA256：`C5B5CB783DB277B2F24C8FA881FACEC20C0F40FBC36677FE47F776D50C760614`

验证：`cargo test --manifest-path src-tauri/Cargo.toml`（117 lib + CLI 分类 9 + 契约 8 全通过）、`npm run test:tauri:flow -- 打包`（真实打包 Codex 会话，剪贴板写入实证）、`npm run test:tauri:smoke`、`npm run build:ui`、Bridge 语法与 handoff 路由测试、`git diff --check`、`npm run package:exe`；真实归档 `.jsonl.zst` 打包实证；VSIX 重复安装两次仅稳定 ID；用户实测右键打包、IM 粘贴发送、外包内容（外部 agent 对真实包核验"对话层 100% 还原"）。已知限制：英文界面与浅色主题下的新菜单样式未逐项目视（文案与样式类已双语言同构）；黑窗修复以未复报计。

## v0.1.8 - 2026-10-24

- 新增「会话分析」面板（悬浮预览用量区块或卡片右键进入）：单会话全生命周期视图——七个总指标、按模型分色的用量时间轴（自动粒度按会话跨度选桶宽，短会话分钟级、长会话到天）、上下文占用曲线（压缩回落自动标注，Codex 附窗口上限参考线）、活动节奏条（用户轮次菱形 + 工具密度）、按类型/按模型/高频工具三组分解；六源逐调用采集（Claude 按 message.id 去重、Codex 沿 rollout 链、ZCode 走 model_usage 观测表、OpenCode 走消息 tokens，Copilot CLI 数据源无用量如实降级）。
- 多智能体会话新增轨迹泳道：每个子代理一条泳道（颜色 = 模型），主代理泳道带 spawn/send/wait 编排标记与等待区间（估算标注），并行度随时间曲线，子代理开销排行含效率列（输出/毛输入）；Codex 子代理按 `thread_spawn.parent_thread_id` 精确归属、guardian 审查者按工作区 + 时间窗估算；任一泳道点击展开该代理自己的用量与上下文合并明细。
- CLI `session list` 支持全历史与归档会话搜索（`--active-days 0`、归档源），补 `updatedMs` 绝对时间戳。
- 分析面板交互经 12 轮实测打磨：两入口结果统一、窗口缩放只出整体竖向滚动（主体块级流布局，任何子面板不再被压缩裁切）、关闭按钮与拖拽互不干扰、悬停详情跟随光标、滚动条与主窗口同款；窄窗断点适配。

发布包：`AgentWatcher-v0.1.8-windows-x64.zip`

SHA256：`B3019EB71E01B468132A51DC057DB3E5E62C8E84A8A3D57649C6E05F121CBFEC`

验证：`cargo test --manifest-path src-tauri/Cargo.toml`（128 通过，含本任务 9 条新增单测；CLI 分类 8 + 契约 8）、Bridge 语法与 handoff 路由测试、`git diff --check`、`npm run package:exe`；真实 Tauri 运行时 CDP 走查多轮（含真实会话对数：Claude 24MB 样本 808 次模型调用与调研独立核算一致、ZCode 53.7M/234 次、多智能体会话 16 worker + 10 guardian 归属精确）；入口统一性、窄窗四档尺寸几何扫描零塌缩。已知限制：125%/150% DPI 与 8 小时长跑未测；guardian 归属为时间窗估算；跨会话聚合面板（阶段 B）未实施。

## v0.1.7 - 2026-09-18

- 新增 AI 操作 CLI `agentwatcher-cli.exe`（随发布包分发）：`session list/show/usage`、`handoff export`、`skill install/list/remove` 六个命令大类，JSON 信封输出契约（`{command, ok, data, error, messages}`，退出码 0/1/2），内容分级与 GUI 隐私边界一致（list 无正文，show 默认悬浮预览级，`--content` 才给正文）；无副作用扫描（不拉起 Codex app-server、清理 stdio 句柄继承），用法技能可一键装进本机 AI 宿主。
- 修复 Codex resume 后的重复会话卡片：resume 会在 `~/.codex/sessions` 另开一个文件名带 `_<新uuid>` 后缀的 rollout 片段（`session_meta` 仍写原 thread id），app-server `thread/list` 对同 id 各返回一条；现在按 id 去重保留 updatedAt 最新的一条，消除「同一会话两张卡」。
- 用量沿 rollout 片段链聚合：轮次/工具/模型调用/时长跨片段求和，token 等累计口径取最新片段——resume 后的会话不再丢失或重复计用量。
- 会话用量新增活跃时长（`activeDurationMs`，预览显示「X 活跃 · Y 跨度」）：Codex/Claude/Copilot Chat 按内容时间戳以 5 分钟空闲阈值聚类；OpenCode 走 message 表 SQL 同口径；ZCode 用 `model_usage` 调用区间并集（并行调用不重复计墙钟，保证活跃 ≤ 跨度）；Copilot CLI 数据源无用量如实标注。

发布包：`AgentWatcher-v0.1.7-windows-x64.zip`

SHA256：`D862A17B56BE07A8B9358B3A274459FD0292F9B35287ABA57DB6ED4921F2BF36`

验证：`cargo test --manifest-path src-tauri/Cargo.toml`（98 通过、1 项外部插件握手按设计忽略；CLI 分类 8 + 契约 8）、`cargo clippy --all-targets -- -D warnings`、Bridge 语法与 handoff 路由测试、`git diff --check`、真实 Tauri 冒烟通过；CLI 30 天全量 133 会话与 GUI 当前 79 会话断言「活跃 ≤ 跨度」零违规；`npm run package:exe`、zip 内容与 15 尺寸图标检查、最终 VSIX 重复安装两次（仅稳定 ID `agentwatcher.agentwatcher-vscode-session-bridge@0.1.12`）、发布目录 EXE 与 CLI 独立启动均通过。已知限制：OpenCode 的活跃时长已在明细数据中计算但预览界面暂不合并显示（维持「跨度 only」）；本机 7 天窗口内无 OpenCode/Copilot 用量样本，二者用量口径以单测与 SQL 直查验证。


## v0.1.6 - 2026-09-15

- 会话用量展示（全部 provider）：悬浮预览新增「用量」区块——累计毛输入（带缓存占比）、累计输出、当前上下文（含窗口上限）、对话轮次、工具调用（含 top 工具排行）、模型调用、模型名、时长；数据源没有的指标显示 — 而不是 0（ZCode 订阅制成本、Copilot CLI 全部用量字段均如实标注）。
- 卡片新增紧凑用量行（轮次 · 工具 · token），compact 及更小密度随分支名一起隐藏，中英文与明暗主题全量适配。
- 采集实现：Claude 按 distinct `message.id` 去重聚合（不去重会虚高约 2.75 倍），Codex 读最后一条 `total_token_usage`（毛口径，cached 为子集；上下文跳过压缩后的全零事件），Copilot Chat 聚合补丁日志最终状态；三者带（路径、大小、修改时间）增量缓存与每扫描 10 文件的渐进预算。
- ZCode 走 `model_usage`/`tool_usage` 观测表批量聚合（毛输入已含缓存读取，用户轮次按 `semantics.origin=real_user` 排除合成消息）；OpenCode 读 `session` 表汇总列（真实美元成本）并新增懒加载命令 `get_session_usage_detail` 查询上下文/轮次/工具明细，避免大库逐扫描聚合。
- JSONL 累计值与调研期独立脚本交叉核对一致（Claude 808 次调用/1302 工具/1.88M 输出；ZCode sess_743d9fde 221.5M 毛输入/693 工具）。

发布包：`AgentWatcher-v0.1.6-windows-x64.zip`

SHA256：`43DD43B08BE7884DA836E927538C29E333297386A79134D2947CC3BA9D5A3E97`

验证：`cargo test --manifest-path src-tauri/Cargo.toml`（91 通过、1 项外部插件握手按设计忽略）、`cargo clippy --all-targets -- -D warnings`、Bridge 语法与 handoff 路由测试、`npm run build:ui`、`git diff --check`、真实 Tauri 冒烟与 zcode-session 流程、真实壳端到端核验（68/68 会话带用量、预览懒加载合并 top 工具）、中英文×dark/light 人工核对、`npm run package:exe`、zip/manifest/15 尺寸图标检查、最终 VSIX 重复安装两次、发布目录 EXE 独立启动均通过。已知限制：`opencode-session` 流程在本机因 7 天活跃窗口内无 OpenCode 会话无法运行（环境性，SQL 直查证实扫描管线正常）。


## v0.1.5 - 2026-09-14

- 接入 ZCode 作为第 6 个会话 provider（徽标 ZC）：从本机 `~\.zcode\cli\db\db.sqlite` 只读扫描，状态机（waiting=AskUserQuestion pending、running=内容时间戳新鲜度）、provider 筛选、设置开关、性能计数与悬浮预览全部可用；子 agent 会话（`task_type=subagent_child`）默认过滤。
- ZCode 接续上下文导出：右键 zcode 卡片可将全量转录导出为 Markdown 落盘 `%TEMP%\AgentWatcher\handoff-sources\zcode\`，作为跨 provider 接续来源（含可移植摘要与 parser hint）。
- ZCode 点击跳转暂不实现（官方无会话级深链 zai-org/feedback#465、桌面发行版 3.11.2/3.12.1 均不含 `@zcode/tui`、独立 CLI 未公开分发），点击返回明确提示；上游能力就绪后可恢复。
- 修复接续面板上下文串号（跨 provider 通用缺陷）：来源会话切换后不再复用上一次导出的 sourceContext，`handoffContextMatchesSource` 不变量在发布/复用/渲染全链路校验归属，OpenCode/ZCode handoff flow 增加导出文件归属断言。
- 抽取 OpenCode 专有的状态 hint 与 handoff transcript 结构为共享 helper（`agent_part_status_hint` 等），ZCode 复用而非复制。
- 新增 `zcode-session` / `zcode-handoff-context` 真实 Tauri 流程；性能面板 provider 计数含 ZC。

发布包：`AgentWatcher-v0.1.5-windows-x64.zip`

SHA256：`EC356AF8FABAECE38FA8E1C75DB1B1D19C43BDC902C17C76E324AF22EF653DE7`

验证：`cargo test --manifest-path src-tauri/Cargo.toml`（81 通过、1 项外部插件握手按设计忽略）、`cargo clippy --all-targets -- -D warnings`、Bridge 语法与路由测试、UI 构建、真实 Tauri 冒烟及 zcode-session/zcode-handoff-context 流程、ZCode 悬浮预览与接续导出逐项对照 db 审计、中英文×dark/light 四组合徽标程序化核验、`git diff --check`、`npm run package:exe`、zip/manifest/15 尺寸图标检查、最终 VSIX 重复安装两次、发布目录 EXE 独立启动均通过。

## v0.1.4 - 2026-08-07

- 将 OpenCode、Copilot CLI 作为独立 provider 接入统一会话列表，补齐扫描、状态、跳转与真实 Tauri 流程验证。
- 会话数量上限改为按 provider 分别计算，避免单一来源占满全局配额。
- 新增工作区路径黑名单，默认过滤 `%TEMP%`，支持用户按行配置目录且不删除任何原始会话数据。
- 修复 Session Preview、Handoff、Todo、Performance 等悬浮面板的窗口层级与多显示器可见性，预览仍只由卡片右下角入口触发。
- 改进 Copilot CLI provider 的独立筛选、标识与卡片图标，避免与 VS Code Copilot 或其他 CLI 混淆。
- 将 AgentWatcher 收敛为零插件也可完整启动、构建和测试的通用插件宿主；具体插件通过独立 `*.awplugin` 描述文件与受控 stdio 协议接入。
- 完善静默开发入口、WebView2 CDP 真实 Tauri 自动化、OpenCode 会话流程与多窗口验收基础设施。

发布包：`AgentWatcher-v0.1.4-windows-x64.zip`

SHA256：`7E80FBF07C6E1C8395315971402873D51C89F4A72D94FA089330C56E453D7103`

验证：`cargo test --manifest-path src-tauri/Cargo.toml`（75 通过、1 项外部插件握手按设计忽略）、`cargo clippy --all-targets -- -D warnings`、Bridge 语法与路由测试、UI 构建、真实 Tauri 冒烟及任务/性能/接续/OpenCode 流程、`git diff --check`、`npm run package:exe`、zip/manifest/15 尺寸图标检查、最终 VSIX 重复安装两次、发布目录 EXE 独立启动均通过。

## v0.1.3 - 2026-06-09

- 接入 Codex 桌面会话扫描，主列表与 AgentTask / Todo 流程并入统一会话模型，支持状态筛选与跳转。
- 新增 Codex 会话来源的 handoff 目标入口，与 Copilot/Claude 一致走 session 跳转与承接链路。
- 新增主窗口整页缩放（Ctrl+/-/0 与 Ctrl+滚轮）与持久化缩放状态，支持 25%~500%。
- 新增 AgentTask 与 Todo 面板窗口（含可缩放手柄）以及性能诊断面板（CPU/进程采样与快照）。
- 修复 Claude 会话卡片打开路径，支持直接在 Claude editor 打开目标会话。

发布包：`AgentWatcher-v0.1.3-windows-x64.zip`

SHA256：`85A69EDB914DA8A342FC87800A8FC36EF9D2C959D4286F7CAB7592023673660C`

验证：`cargo test --manifest-path src-tauri\Cargo.toml`、`node --check vscode-agentwatcher-bridge\extension.js`、`node .tmp\bridge-handoff-routing-test.cjs`、`npm run build:ui`、`git diff --check`、`npm run package:exe`、zip/manifest/SHA256 检查、重复安装最终 VSIX 后只保留稳定 Bridge ID。

## v0.1.2 - 2026-06-01

- 新增 full-path workspace grouping：同名 workspace 使用完整路径区分，减少跨目录 session 混淆。
- 新增卡片右键接续能力，可从当前 session 发起跨 provider handoff。
- 热修替换现有 v0.1.2 release asset，不新增 app/release 版本号，避免用户看到新增版本。
- 修复 Bridge 唯一扩展身份：唯一支持 ID 固定为 `agentwatcher.agentwatcher-vscode-session-bridge`。
- Bridge VSIX package name 稳定为 `agentwatcher-vscode-session-bridge`，Bridge 自身版本更新为 `0.1.11`。
- Bridge command namespace 稳定为 `agentwatcherSessionBridge.*`。
- Bridge install/update 会在安装稳定 VSIX 前 best-effort 清理历史 `safe1` / `safe2` / `safe3` / `safe4` 扩展。
- Bridge 安装后会校验稳定 ID 已安装且 legacy 临时 ID 不再残留，避免重复生成新的临时扩展身份。
- 新增 Copilot、Copilot CLI、Claude 目标路由，handoff 可按目标 provider 打开新会话并插入 prompt。
- 修复点击 Claude session card 时没有打开 Claude Code editor 的问题。
- 修复 Session Preview 在副屏、多 DPI 或靠近屏幕边界时可能显示到屏幕外的问题。
- `launch_handoff` 改为后台执行，等待 Bridge ack 时不阻塞主 UI。
- 修正子窗口设置同步：language/theme/layout/always-on-top 会同步到 `session-preview` 和 `handoff-panel`。
- 发布打包脚本会清理旧 release zip / 解压目录，清理 Bridge 输出目录旧 VSIX，并生成当前版本 Windows zip。
- 明确 clipboard bridge 真实口径：当前 prompt 插入仍依赖 clipboard，非剪贴板通道列入 Future。

发布包：`AgentWatcher-v0.1.2-windows-x64.zip`

SHA256：`93DAFE31A9CC95085226797BEE534E8C0C81624580E5C7CF29DA306313F647C1`

验证：`cargo test --manifest-path src-tauri\Cargo.toml`、`node --check vscode-agentwatcher-bridge\extension.js`、`node .tmp\bridge-handoff-routing-test.cjs`、`npm run build:ui`、`git diff --check`、`npm run package:exe`、zip/manifest/SHA256 检查、重复安装最终 VSIX 后只保留稳定 Bridge ID。

## v0.1.1 - 2026-05-29

- 新增设置面板 always-on-top 开关，并统一以 [ui/index.html](ui/index.html) 作为唯一 UI 源。
- 新增横版 / 竖版布局切换，可在设置面板中实时切换并持久化偏好。
- 修复浅色主题下滚动条、segment、toggle、标题栏按钮、session card、rail 等控件的视觉不一致。
- 修正 Copilot / Claude session 的用户输入、交互式回答和 AI 正文摘要提取，过滤工具、terminal、thinking、模型错误等噪声。
- 修复 skipped / skip / 跳过回答被误采集为用户输入的问题。
- 修复 Copilot 已完成请求残留 questionCarousel 时误判 waiting 的问题。
- 新增并优化 Session Preview：通过卡片展开按钮悬停触发，Tauri 运行态使用 frameless tooltip webview，避免被 Watcher 主窗口裁剪。
- 优化 Session Preview 性能和隐私：复用隐藏的 preview webview，ready 后显示避免首帧闪窗，预览正文通过事件传递而不写入 localStorage。
- 拆分 `session-preview` capability，收敛预览窗口权限范围。

发布包：`AgentWatcher-v0.1.1-windows-x64.zip`

SHA256：`0D8A9B96336D91BCA692BDC8C1ACBB12825C443619FD37801CD3DB5C63C32EF8`

验证：`git diff --check`、`npm run build:ui`、`cargo test --manifest-path src-tauri/Cargo.toml`、`npm run package:exe`。

## v0.1.0 - 2026-05-28

- 初始 Windows 发布版。
- 新增 Tauri Windows 桌面应用外壳。
- 扫描 Copilot 和 Claude Code session 日志，并按待回复、运行中、闲置分组展示。
- 新增 VS Code Bridge，用于从 AgentWatcher 跳转到对应 workspace/session。
- 首次启动时自动安装或更新随包附带的 VS Code Bridge 扩展。
- 发布物整理为 Windows zip 包，包含 `AgentWatcher.exe` 和 `vscode-agentwatcher-bridge/`。
