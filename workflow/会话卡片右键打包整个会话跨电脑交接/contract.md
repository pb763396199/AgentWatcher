---
schema_version: "1"
protocol: "1.3.0"
id: "wi_25PNYKYZZZEZF9VBX61N79GPEG"
short_id: "70xh5fms"
title: "会话卡片右键打包整个会话跨电脑交接"
status: "done"
kind: "feature"
created_at: "2026-10-27T07:01:46.811750100+00:00"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
branch_or_pr: "feature/会话卡片右键打包整个会话跨电脑交接"
revision: "4"
updated: "2026-10-27"
---
# 会话卡片右键打包整个会话用于跨电脑交接

## 原始请求

用户原话（2026-10-27）："我希望在session card右键除了复制接续提示词外，出一个新的按钮，那就是将整个会话内容进行打包传递，以可以跨电脑交接，你先洞悉出用户真正的需求与意图，再设计出一套合理的方案"

方向确认（2026-10-27，当面提问）：交接包在目标电脑做到"自包含包，不做导入"。

## 目标

右键任意会话卡片，把该会话的完整内容打成一个自包含的 zip 交接包；传到另一台电脑后，不依赖源机器的文件系统就能读到全部会话内容。

## Why

现有「复制接续提示词」跨电脑是断的：提示词里的来源文件路径指向本机磁盘（`ui/index.html` 的 `buildHandoffPrompt` / `handoffPortableSourceFile` 引用 `%TEMP%\AgentWatcher\handoff-sources\…` 或本机 JSONL 路径），换台电脑就是死链；内嵌摘要只取最后一条用户消息 700 字、AI 回复 1200 字（`src-tauri/src/lib.rs` 的 `handoff_inline_summary`），上下文大量丢失。跨电脑交接需要完整、自包含的内容。

## 范围

做：

- 右键菜单新增「打包会话用于跨电脑交接」入口，与「复制接续提示词」并列（`ui/index.html` 的 `handoffContextMenu`，现在有创建接续会话 / 复制接续提示词 / 打开原会话 / 会话分析四项）。
- 新增 Tauri 命令（如 `export_session_package`）：按会话定位源数据，产出单个 zip 文件。
- 六个 provider 的打包内容：
  - Copilot Chat / Copilot CLI：chatSessions 的 JSONL 原样 + transcript 覆盖文件。
  - Claude：projects 下的会话 JSONL 原样。
  - Codex：同 thread 的全部 rollout 片段（含归档 `.jsonl.zst` 解压后的内容）。
  - OpenCode / ZCode：SQLite 里该会话的 session 行与全量 message/part 行导出为 NDJSON。
- 包内固定结构：`manifest.json`（provider、会话 ID、标题、工作区路径、时间范围、导出时间、App 版本、文件清单与 SHA256；工作区是 git 仓库时另带 git 锚点：远端、分支、HEAD、未提交修改数、未推送提交数）、`HANDOFF.md`（短的定位块：provider、会话 ID、工作区、代码锚点一行、导出时间、文件清单，加一行隐私提醒；不含摘要与转录）、`README.md`（十几行的数据格式说明：包结构、各 provider 的 raw 口径、NDJSON 与双重编码、SHA256 校验方法、快照时点语义；不含会话正文）、`raw/`（原始内容，全量是本功能的核心承诺）。
- git 锚点只记元数据（repo/branch/head/未提交数/未推送数），不打包任何代码或工作区文件（2026-10-27 第二轮反馈拍板：模型等产物明确不要，代码走 git 正交通道，包只交对话与锚点）。
- 临时产物清理（2026-10-27 第三轮拍板"都要有清理"）：每次打包与接续源导出时，顺手删掉 `%TEMP%\AgentWatcher\session-packages\` 与 `handoff-sources\` 下最后修改超过 7 天的文件；只扫这两个子目录，尽力而为（删除失败忽略），不影响主流程，不引入定时器。
- 打包完成后把 zip 文件本身写入系统剪贴板（Windows CF_HDROP 文件剪贴板），toast 确认"可直接粘贴发送"；剪贴板写入失败时 toast 显示完整路径并在资源管理器中揭示所在文件夹（复用 `tauri-plugin-opener`）。

不做：

- 目标电脑上的导入、识别、原生恢复（用户已确认本期不做，理由见设计取舍）。
- 不加 `tauri-plugin-dialog`（不做原生另存为对话框）。
- 不改 `agentwatcher-cli`（CLI 打包入口留待后续任务）。
- 不改「复制接续提示词」的生成逻辑。
- 不做完整人可读转录（`transcript.md`）：用户拍板接续说明只要定位块、agent 要了解就读原始文件（2026-10-27 反馈，见访谈记录）。
- 不把代码或工作区文件（含 git bundle、构建产物、模型文件）塞进包：会话包交对话与锚点，代码走 git 正交通道（2026-10-27 第二轮反馈拍板）。
- 不做正文脱敏：原始文件全量与 SHA256 校验是包的承诺，脱敏会同时破坏两者；隐私靠 HANDOFF 的提醒与人的传输判断。

## 强约束

- 打包是显式用户动作；会话正文只存在于输出 zip 与其源数据中，不写 localStorage、不自动外发。
- 剪贴板只在打包完成那一次写入 zip 文件本身（CF_HDROP），不写正文文本；现有「复制接续提示词」的剪贴板行为不受影响。
- 宿主规则不变：不新增对具体插件或外部可执行文件的依赖；打包逻辑放 `lib.rs` 通用层。
- 现有右键菜单行为（创建接续会话、复制接续提示词、打开原会话、会话分析）不回退。
- 中英文、dark/light 同步。

## 自主边界

| 不用问直接定 | 必须停下来问 |
| --- | --- |
| 包格式用 zip（单文件便于传输，Windows 原生可解） | 无。方向与两条反馈已确认 |
| 输出目录 `%TEMP%\AgentWatcher\session-packages\`（用户指定放 temp） | |
| 文件名 `provider-<自然键>-<时间戳>.zip` | |
| 剪贴板写 zip 文件本身（CF_HDROP，Win32 API），失败回退路径提示与文件夹揭示 | |
| manifest.json 里记每个文件的 SHA256 | |

## 要落盘的东西

- 单元测试需要六个 provider 的最小 fixture（沿用 `lib.rs` 文件末尾现有测试的 fixture 写法），覆盖：Codex 多片段与 zst 归档、OpenCode/ZCode 的 SQLite 行导出、manifest 与文件清单一致性、源缺失时的错误路径。
## 验收条件

- AC-001: 右键菜单出现「打包会话用于跨电脑交接」：中英文文案齐全，dark/light 下样式与现有菜单项一致；点击后对当前卡片会话产出 zip 到 `%TEMP%\AgentWatcher\session-packages\`，zip 文件写入系统剪贴板（可在 IM/资源管理器中直接粘贴），toast 确认；剪贴板失败时 toast 显示完整路径并揭示文件夹；打包同时清理 `session-packages\` 与 `handoff-sources\` 下 7 天前的旧产物（单元用例 `session_package_cleans_stale_temp_artifacts` 锁行为）。Verify: manual:npm run test:tauri:flow 新增的打包流程步骤通过（真实 Tauri 壳、window.__TAURI_INTERNALS__ === true）
- AC-002: 六个 provider 各能打包一个真实会话且 raw/ 内容完整：文件型原样携带源文件（Codex 含同 thread 的全部片段与归档 zst 解压内容，Copilot 含 chatSessions 与 transcripts），OpenCode/ZCode 含全量 message/part 的 NDJSON。Verify: case:session_package_covers_all_providers
- AC-003: 包自描述且瘦身：manifest.json 字段齐全（含文件清单与 SHA256，清单与 raw/ 实际文件一一对应；工作区是 git 仓库时含 repo/branch/head/未提交数/未推送数锚点，非 git 工作区无该块），HANDOFF.md 只含定位块（provider、会话 ID、工作区、代码锚点一行、导出时间、文件清单）与一行隐私提醒，README.md 只讲数据格式不含会话正文；三者都不含摘要与转录。Verify: case:session_package_manifest_matches_files
- AC-004: 失败路径有明确提示：源文件缺失、被占用或不可读时打包报可读错误，不产出半截 zip。Verify: case:session_package_reports_source_errors
- AC-005: 现有右键菜单四项行为与既有冒烟、流程测试不回退。Verify: manual:npm run test:tauri:smoke 通过

## 访谈记录

| 时间 | 谁说的 | 内容 | 结论 |
| --- | --- | --- | --- |
| 2026-10-27 | 用户 | 原始请求照录见「原始请求」节 | 要跨电脑交接的完整会话打包 |
| 2026-10-27 | 用户 | 方向提问的回答：「自包含包，不做导入」 | 本期只做导出侧，不做目标侧导入或恢复 |
| 2026-10-27 | 用户 | "产物我建议放在系统temp里的文件夹，你觉得能，默认是打包后会写入系统剪贴板，方便粘贴发送" | 产物放 `%TEMP%\AgentWatcher\session-packages\`；打包后把 zip 文件本身写入剪贴板（按"粘贴发送"理解为 CF_HDROP 文件剪贴板，非路径文本） |
| 2026-10-27 | 用户 | "接续说明我希望短一点，没必要额外包含上下文，目前我用你的右键接续会话都只会拷贝【来源定位】那一段，其他的agent想了解就一定会去读文件，这就要求每种会话对应的agent的原始文件一定要全" | HANDOFF.md 只放定位块；去掉 transcript.md；原始文件全量成为核心要求 |
| 2026-10-27 | 用户 | 转述另一 agent 对真实交接包的评价并问有无道理；表明立场："不希望交接包里有大量项目的文件，比如说建好的模型文件之类的；工作产物本身(代码)可能比较轻量，理论上能接受，但我觉得没啥必要吧；但它说的有仓库+分支+提交这我觉得挺有意思" | 采纳 git 锚点（repo/branch/head/未提交数/未推送数进 manifest 与 HANDOFF 一行）与 README 格式说明；代码与工作区文件明确不进包；脱敏不采纳 |
| 2026-10-27 | 用户 | 问 temp 产物多久清、什么机制清；答复"现在无机制清理"后拍板"都要有清理" | session-packages 与 handoff-sources 都加清理：导出动作时顺手删 7 天前旧文件 |

## 设计取舍

决策 1：交接闭环做到哪一步（用户拍板）

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 自包含导出包 | zip 内含原始文件全量 + 定位块接续说明，人工传输解压 | 目标侧要人工把文件交给新会话 | 采用（用户当面确认） |
| 包 + 目标侧 AgentWatcher 识别 | 解包后作为接续来源，生成提示词时指向解包后的本地路径 | 新增解包 UI 与外部会话模型，工作量明显更大 | 不采用，留作后续任务 |
| 包 + 尽力原生恢复 | Claude/Codex 的 JSONL 放回原生目录即可 resume | 要写别的 agent 的数据目录；OpenCode/ZCode 得写 SQLite，风险高；Copilot 的 workspaceStorage 哈希目录跨机器不可行 | 不采用 |

决策 2：包格式

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| zip 单文件 | 加 `zip` 依赖（纯 Rust 实现） | 新增一个依赖 | 采用：传输场景单文件最方便，Windows 原生可解压 |
| 目录 | 直接拷目录，不加依赖 | "打包传递"语义不符，传输前还要自己压缩 | 不采用 |
| 单 JSON bundle | 原始文件多为文本，可嵌进一个 JSON | 自定义格式没有通用工具链，等于自造 zip | 不采用 |

决策 3：输出位置与传递方式（用户拍板）

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| temp 目录 + zip 文件入剪贴板 | 写 `%TEMP%\AgentWatcher\session-packages\` 后用 Win32 CF_HDROP 写入文件剪贴板 | temp 会被系统清理；文件打包后立即发送的场景下这个代价不成立 | 采用（用户指定） |
| Downloads 目录 + 资源管理器揭示 | 写入后揭示，toast 显示路径 | 用户还得自己去文件夹里找文件再拖进 IM | 不采用（用户改为 temp+剪贴板） |
| 原生另存为对话框 | 加 `tauri-plugin-dialog` 与对应 capability | 新增插件与权限面 | 不采用，可作后续增强 |

决策 4：包内是否含完整转录 transcript.md（用户拍板）

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 不含，只带定位块 HANDOFF.md | 包 = manifest + HANDOFF + raw/ | 想直接读可读转录的人要自己去解原始文件 | 采用：用户明确"agent 想了解就一定会去读文件"，原始文件全量是核心；且文件型 provider 没有现成全量转录渲染器，为不读它的读者造一套不值得 |
| 含完整转录 | 为六个 provider 各写全量转录渲染器 | 工作量大头落在没人读的产出上；与"接续说明短一点"的意图相反 | 不采用 |

决策 5：剪贴板写什么

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| zip 文件本身（CF_HDROP） | Win32 剪贴板 API 写文件句柄，IM/资源管理器可直接 Ctrl+V 粘贴出发送 | 需要 windows-sys 补剪贴板相关 feature；剪贴板被占时可能失败 | 采用；失败回退 toast 显示路径并揭示文件夹 |
| 路径文本 | `navigator.clipboard.writeText` | 粘出来是一串字符串，对方拿不到文件，与"粘贴发送"不符 | 不采用（仅作剪贴板失败时的提示内容，不主动写） |

决策 6：代码与工作产物要不要进包（用户拍板；起因是外部 agent 评审指出"包交了对话没交工作"）

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| git 锚点元数据进 manifest/HANDOFF | 打包时对工作区跑只读 `git rev-parse/status/rev-list`，记远端、分支、HEAD、未提交数、未推送数 | 多两次子进程调用；非 git 工作区静默省略 | 采用：接替方拿到"代码在哪个提交"的确切锚点，代码本体仍走 git 正交通道 |
| git bundle / diff 塞进 zip | 把未推送提交打成 bundle 一起打包 | 体积不可控；会话包开始承担版本库职责，与"交对话"的定位混淆 | 不采用（用户拍板"没啥必要"） |
| 工作区文件（模型、构建产物）进包 | 按扩展名挑文件打包 | 体积爆炸、隐私面扩大、与会话数据无关 | 不采用（用户明确不要） |

决策 7：包内要不要数据格式说明（README.md）

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 十几行 README.md | 说明包结构、raw 口径、NDJSON 双重编码、SHA256 校验、快照时点 | 多一个小文件 | 采用：外部评审实证"跨 agent 要摸索 schema"；它讲格式不带正文，不违背"接续说明短" |
| 只靠 manifest 自解释 | 不加说明 | 非 SQLite provider 的双重编码与表结构要猜 | 不采用 |

决策 8：temp 产物清理（用户拍板"都要有清理"）

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 导出动作时顺手清 7 天前旧文件 | 打包/接续源导出/解压导出的收尾扫一遍两个子目录，删 mtime 超 7 天的文件 | 每次导出多一次目录扫描（两个目录、文件量个位数到几十，毫秒级） | 采用：无定时器无后台线程；7 天与 Windows 存储感知口径一致，接续提示词引用的近期文件不会被误删 |
| 后台定时清理 | 起定时器周期扫描 | 常驻线程与调度复杂度，收益不匹配 | 不采用 |
| 不清理（现状） | 依赖用户手动清 temp | 无限累积；依赖存储感知（默认关闭） | 不采用（用户拍板要清理） |

## revision 历史

| 版本 | 日期 | 为什么改 |
| --- | --- | --- |
| 1 | 2026-10-27 | 首版：方向确认为自包含包不做导入，产物定 Downloads + 揭示，包含完整转录 transcript.md |
| 2 | 2026-10-27 | 用户反馈两条：产物改放 `%TEMP%\AgentWatcher\session-packages\` 且 zip 文件写入剪贴板；接续说明缩短为定位块，去掉 transcript.md，原始文件全量升为核心要求 |
| 3 | 2026-10-27 | 外部 agent 评审采纳两刀：manifest/HANDOFF 加 git 锚点（repo/branch/head/未提交数/未推送数）；包内加十几行 README.md 格式说明。代码、工作区文件、脱敏明确不采纳 |
| 4 | 2026-10-27 | 用户拍板"都要有清理"：session-packages 与 handoff-sources 在导出动作时顺手清掉 7 天前旧文件 |

