# AgentWatcher Bridge 唯一扩展身份修复审计

日期：2026-06-01
范围：v0.1.2 Bridge stable ID 热修替换决策与最终发布事实审计；不改业务代码、不运行验证、不提交。

## 结论先行

- 完成状态：complete。
- 验证状态：pass by recorded evidence；本审计未重跑验证。
- 需求覆盖：all for Bridge identity fix。
- 计划兑现：as_planned。
- 核心根因：v0.1.2 把临时测试 extension ID 发布化，导致发布包可能留下非稳定 Bridge 身份。
- 核心修复：固定 stable extension ID、stable command namespace、安装前清理 legacy、安装后校验 legacy 不残留，并用回归测试和 artifact manifest 检查闭环。
- 最终发布决策：最终没有新增 v0.1.3；采用 v0.1.2 热修替换现有 GitHub Release asset。Release URL：`https://github.com/pb763396199/AgentWatcher/releases/tag/v0.1.2`。
- 最终发布事实：hotfix commit `a8ab803 fix(bridge): stabilize VS Code Bridge identity`；remote `dev` 和 tag `v0.1.2` 均指向 `a8ab803e85e8de5491d2aee9490b288c2d062558`。
- 最终产物：`AgentWatcher-v0.1.2-windows-x64.zip`，release asset digest / local SHA256 均为 `866522276A933B1C57E173AAF5B6CAB6727463AC98CB1A34ABE1297A25DAB1BB`；替换前后 release asset downloadCount 均为 0，改写风险低。

## 任务结果裁定（Task Outcome Verdict）

本次修复可以裁定为完成。发布身份已经收敛到唯一稳定连接组件身份，命令命名空间保持稳定。安装和更新路径增加 legacy 测试 ID 清理与安装后校验，避免继续把临时测试身份误当成发布身份。

最终发布没有新增 v0.1.3，而是以 v0.1.2 hotfix 方式替换既有 GitHub Release asset。hotfix commit 为 `a8ab803 fix(bridge): stabilize VS Code Bridge identity`，remote `dev` 和 tag `v0.1.2` 都已指向完整 SHA `a8ab803e85e8de5491d2aee9490b288c2d062558`。Release 页面为 `https://github.com/pb763396199/AgentWatcher/releases/tag/v0.1.2`，最终 artifact 为 `AgentWatcher-v0.1.2-windows-x64.zip`。

## 需求台账（Requirement Ledger）

| ID | 需求 | 裁定 |
|---|---|---|
| R1 | 找到并记录 Bridge 多身份问题根因 | satisfied |
| R2 | 将发布身份改回唯一 stable ID | satisfied |
| R3 | 使用 stable command namespace | satisfied |
| R4 | install/update 前清理 legacy 临时 ID | satisfied |
| R5 | 安装后校验 stable 已安装且 legacy 不残留 | satisfied |
| R6 | 增加 regression tests 和 package artifact manifest 验证 | satisfied |
| R7 | 本审计只写 docs/retrospectives，不改业务代码、不运行验证、不提交 | satisfied |
| R8 | 记录最终发布事实、commit、tag/branch 指针、Release URL、digest 和 downloadCount 风险 | satisfied |

## 计划基线（Plan Baseline）

初始计划可从用户提供的修复摘要和最终发布决策推断：修复唯一扩展身份，避免临时测试 ID 继续进入发布包；补齐安装清理、安装后校验、回归测试和发布产物 manifest 检查；最终不新增 v0.1.3，而是用 v0.1.2 hotfix 替换现有 GitHub Release asset。

## 交付物清单（Outcome Inventory）

- Hotfix commit：`a8ab803 fix(bridge): stabilize VS Code Bridge identity`。
- Git refs：remote `dev` 和 tag `v0.1.2` 均指向 `a8ab803e85e8de5491d2aee9490b288c2d062558`。
- GitHub Release：`https://github.com/pb763396199/AgentWatcher/releases/tag/v0.1.2`。
- 已替换的 GitHub Release asset：`AgentWatcher-v0.1.2-windows-x64.zip`，release asset digest / local SHA256 均为 `866522276A933B1C57E173AAF5B6CAB6727463AC98CB1A34ABE1297A25DAB1BB`。
- Release asset 替换风险：downloadCount 替换前后均为 0，改写风险低。
- VS Code 连接组件 package：manifest 使用稳定 package name、publisher 和 version。
- 主程序常量：`src-tauri/src/lib.rs` 固定稳定连接组件身份，并列出 legacy 测试 ID。
- 文档记录：README、CHANGELOG、TODO 已记录 stable ID、legacy 清理和发布验证口径。
- 审计文档：本文件。

## 需求到结果对账矩阵（Requirement-To-Outcome Matrix）

| 需求 | 预期结果 | 实际结果 | 覆盖状态 | 证据 | 缺口 |
|---|---|---|---|---|---|
| R1 根因 | 明确 bad 临时发布身份风险 | 复盘记录 v0.1.2 现有 release asset 存在 bad 临时身份风险 | satisfied | 本审计记录、用户修正 | none |
| R2 stable ID | 发布只支持稳定连接组件身份 | README、连接组件 README、Rust 常量、安装结果均指向 stable ID | satisfied | README 连接组件章节、src-tauri/src/lib.rs | none |
| R3 stable commands | command namespace 不随测试 ID 变化 | package.json 和 extension.js 使用稳定命名空间 | satisfied | 连接组件 package manifest、extension.js | none |
| R4 legacy cleanup | 安装稳定 VSIX 前清理 safe legacy | 文档和 changelog 记录 install/update 会 best-effort 卸载 legacy | satisfied | CHANGELOG、README | none |
| R5 post-install verify | 安装后不允许 legacy 残留伪成功 | README 记录 stable 已安装且 legacy 不残留，否则返回明确错误 | satisfied | README | none |
| R6 regression/package checks | 自动化和产物检查覆盖身份回归 | 用户提供验证事实显示 cargo/node/test/diff/package/zip/重复安装检查均通过 | satisfied | validation facts、TODO checklist | none |
| R8 final release facts | 记录 v0.1.2 hotfix 真实发布落点 | 未新增 v0.1.3；Release asset 已替换；remote `dev` 和 tag `v0.1.2` 均指向 `a8ab803e85e8de5491d2aee9490b288c2d062558`；downloadCount 0 -> 0 | satisfied | 用户提供最终发布事实 | none |

## 偏差登记表（Deviation Register）

| ID | 偏差类型 | 原计划 / 原需求 | 实际变化 | 理由 | 价值 | 代价 | 证据 | 状态 |
|---|---|---|---|---|---|---|---|---|
| D1 | artifact_change | v0.1.2 现有 GitHub Release asset 可继续沿用 | 最终没有新增 v0.1.3；重打包并替换 v0.1.2 release asset | v0.1.2 存在 bad 临时身份风险，但最终决策是不新增版本号；downloadCount 替换前后均为 0 | 发布身份恢复稳定，同时保持版本号不变；改写风险低 | GitHub release asset 被原地替换，需要依赖 digest 和 tag 指针审计发布事实 | 用户修正、本审计记录 | evidence_backed |
| D2 | validation_scope | 本审计可重跑验证 | 本审计不运行验证，只记录已完成事实 | 用户明确要求不运行验证 | 保持审计边界清晰 | 证据依赖本轮已记录验证输出 | 用户请求、CHANGELOG/TODO | evidence_backed |

## 修正后的完整计划定稿版（Finalized Revised Plan）

- 定稿状态：ready。
- 适合复用程度：条件通用。
- 修正后目标：任何发布 Bridge 都只允许 stable extension ID 和 stable command namespace；临时 safeN 只能作为开发历史残留被清理，不能作为发布身份；本次最终没有新增 v0.1.3，而是作为 v0.1.2 hotfix 替换现有 GitHub Release asset。
- 适用条件：AgentWatcher v0.1.x、VS Code Bridge VSIX、本地 `code` CLI 可用于安装或列出扩展。
- 不适用条件：多 publisher、多 channel 并行安装、VS Code CLI 不可用且无法扫描 extension 目录的环境。
- 修正后执行步骤：固定 Bridge package name 和 publisher；固定 URI authority / extension ID；固定 command namespace；安装前清理 legacy safe IDs；安装 stable VSIX；安装后检查 stable ID 存在且 safe legacy 不存在；运行 regression tests；检查 zip manifest；重复安装最终 VSIX 验证只留下 stable ID。
- 与原计划差异：v0.1.2 的临时测试发布路径废弃；最终决策并执行为重打包并替换 v0.1.2 release asset，不新增版本号。
- 验证门槛：cargo test、node syntax check、handoff routing regression、diff whitespace、package exe、zip manifest、重复安装 VSIX 后 extension list 检查；重打包后更新 `AgentWatcher-v0.1.2-windows-x64.zip` 的 SHA256。
- 不采用方案：继续增加 `safe5` 或继续用 safeN 当发布 authority。

## 理由与决策记录（Rationale & Decision Log）

- stable ID 是用户和主程序唯一能长期依赖的扩展身份，不能随临时测试名变化。
- command namespace 和 extension ID 分离但都要稳定，否则 handoff routing、URI authority 和用户排障会互相混淆。
- legacy 清理放在安装前，安装后再校验，是为了同时覆盖旧残留和新安装失败两类问题。
- package artifact manifest 检查必要，因为源码正确不代表 release zip 中的 VSIX/package.json 一定正确。
- 不新增 v0.1.3 的决策依赖两个事实：v0.1.2 仍是当前发布标签，且 release asset downloadCount 替换前后均为 0，因此原地替换对已下载用户的影响风险低。
- remote `dev` 与 tag `v0.1.2` 同指向 `a8ab803e85e8de5491d2aee9490b288c2d062558`，说明源码发布点和标签发布点一致。

## 证据包（Evidence Pack）

- 本审计记录：最终发布决策为不新增 v0.1.3，重打包并替换现有 v0.1.2 GitHub Release asset；Release URL 为 `https://github.com/pb763396199/AgentWatcher/releases/tag/v0.1.2`。
- Hotfix commit：`a8ab803 fix(bridge): stabilize VS Code Bridge identity`；remote `dev` 和 tag `v0.1.2` 均指向 `a8ab803e85e8de5491d2aee9490b288c2d062558`。
- Artifact 证据：`AgentWatcher-v0.1.2-windows-x64.zip` 的 release asset digest / local SHA256 均为 `866522276A933B1C57E173AAF5B6CAB6727463AC98CB1A34ABE1297A25DAB1BB`。
- GitHub Release 风险证据：release asset downloadCount 替换前后均为 0，rewrite risk low。
- README Bridge 章节：记录唯一 stable ID、自动安装行为、安装前清理和安装后校验。
- 发布验证清单：记录 legacy 不残留、artifact manifest、VSIX 文件名、package 输出和 zip 内容检查。
- VS Code 连接组件 package manifest：记录 package name、publisher、version 和 command contributions。
- `src-tauri/src/lib.rs`：记录 stable ID 常量和 legacy safe ID 列表。
- 用户提供验证事实：cargo test 18 passed；`node --check`；bridge routing regression；`git diff --check`；`npm run package:exe`；package manifest stable；重复 VSIX install 后只留下稳定连接组件。

## 执行时间线（Execution Timeline）

1. 发现 v0.1.2 发布包存在临时 Bridge 身份发布化风险。
2. 将 Bridge 身份、URI authority 和 command namespace 收敛到 stable 命名。
3. 在安装更新路径补充 legacy safe ID 清理和安装后残留校验。
4. 补充回归验证和 package artifact manifest 检查。
5. 最终决策改为不新增版本号，重打包 `AgentWatcher-v0.1.2-windows-x64.zip` 并替换现有 GitHub Release asset；SHA256 `866522276A933B1C57E173AAF5B6CAB6727463AC98CB1A34ABE1297A25DAB1BB`。
6. 发布落点固定：hotfix commit `a8ab803`，remote `dev` 和 tag `v0.1.2` 均指向 `a8ab803e85e8de5491d2aee9490b288c2d062558`；Release URL 为 `https://github.com/pb763396199/AgentWatcher/releases/tag/v0.1.2`。
7. 替换风险确认：release asset downloadCount 替换前后均为 0，因此原地替换风险低。
8. 本审计只读核对文件与用户提供验证事实，并写入 retrospective。

## Agent 表现与过程轨迹

- 本审计遵守“不改业务代码、不运行验证、不提交”的边界。
- 审计依据来自仓库文档、Bridge package 声明、Rust 常量、artifact 目录和用户提供验证事实。
- 没有执行 build/test/package 命令。
- 本次补充只更新 retrospective 的最终发布事实，不改变发布产物、标签或源码。

## 用户纠正记录

empty。用户直接给出了根因、修复点、验证事实、发布产物、发布落点和审计边界。

## 验证质量（Validation Quality）

- 覆盖充分：Rust 单元测试、JS syntax、handoff routing regression、diff whitespace、package、zip manifest 和真实 VSIX 重复安装后 extension list 都覆盖了身份回归链路。
- 本审计验证状态：not-run by design；结论基于 recorded evidence。
- 剩余验证缺口主要在人工 UI smoke，而不是唯一扩展身份修复本身。
- 发布事实未由本审计重查 GitHub API 或 git remote；remote/tag 指针、downloadCount 和 digest 均按用户提供的最终事实记录。

## 系统改进建议

- 发布前继续保留“artifact manifest 检查”和“重复安装最终 VSIX 后 extension list 检查”作为固定 gate。
- 临时 safeN 测试 ID 以后只允许在开发说明中出现，不能进入 release changelog 的当前身份字段。
- package 脚本可以持续输出 Bridge package name、publisher、version 和 zip SHA256，降低人工核对成本。
- 对原地替换 release asset 的场景，建议固定记录 tag SHA、asset digest、local SHA256、downloadCount before/after 和 Release URL，避免后续无法还原发布事实。

## Knowledge Curator Handoff

- 可回流经验：发布身份类修复必须同时检查源码声明、运行时常量、VSIX manifest、安装行为和已安装扩展列表。
- 可回流经验：临时扩展 ID 一旦被用于真实发布，需要替换受影响 release asset，而不是只在文档里解释；是否新增版本号取决于发布策略，本次最终决策为 v0.1.2 热修替换。
- 可回流经验：release asset 原地替换只有在下载量、tag 指针和 digest 证据充分时才适合采用；本次 downloadCount 0 -> 0 支撑低风险判断。
- 本审计不写长期 memory。

## 后续动作（Follow-up Actions）

1. 完成 TODO 中仍未勾选的发布目录启动、Bridge 自动安装/更新 UI 提示、session 跳转、handoff ack、主题/语言同步和 Session Preview 存储边界 smoke。
2. 后续若继续采用原地替换 release asset，必须同步记录 tag SHA、asset digest、local SHA256、downloadCount before/after 和 Release URL。
3. 若用户机器仍显示 legacy safe 扩展残留，按 README 手动卸载后再重复最终 VSIX 安装检查。

## 附记：AGENTS.md 发布规范更新

- 完成状态：complete。
- 变更范围：仅更新 `AGENTS.md` 发布规范与顶部状态描述，未改业务代码；用户随后选择“提交 AGENTS.md”。
- 变更原因：用户指出第一版发布规范漏项，遗漏发布前准备、文档更新、中英切换、light/dark、过时注释、code review、code simplify、专家组清单和版本对比等关键发布工作。
- 过程记录：已派 UE Session Retrospective / UE Reviewer / Explore 复盘，并综合三方结果，把第一版偏窄清单扩展为完整、有序、可执行的大白话发布流程。
- 已完成修正：`AGENTS.md` 的“发布规范（大白话版）”已改为 9 步有序清单：范围和版本、清文档旧口径、专家组角色、code review / simplify、UI 人工验收、自动化打包、最终 artifact 验证、发布 / Release Note、发布后回看。
- 规范新增覆盖：发布前准备和版本策略对齐；README、CHANGELOG、TODO、Bridge README 等文档同步；旧版本号、旧 Bridge ID、safeN、过时注释和旧路径清理；中英语言、dark/light、横竖布局、always-on-top、主窗口、Session Preview、Handoff Panel 人工验收；Release / Bridge / UI / Docs / Privacy / QA 专家组角色检查；发布风险 code review；临发布前 code simplify；最终 zip、VSIX、SHA256、digest、tag 和 release asset 对账。
- 额外清理：修掉了 `AGENTS.md` 顶部“仓库现在是干净的”这类会快速过时的状态描述，避免把一次性工作区状态写成长期规则。
- 提交与推送事实：提交 `17f9ae0 docs: add AgentWatcher release discipline` 已推送到 `origin/dev`，完整 SHA 为 `17f9ae09d92c9d60a7caee44266043efb85884f7`。
- 发布 tag 状态：`v0.1.2` tag 仍停在 hotfix commit `a8ab803e85e8de5491d2aee9490b288c2d062558`；AGENTS 文档提交没有移动 release tag。
- 记录的验证事实：`get_errors AGENTS.md` 显示 No errors；grep 已确认旧口径消失。本附记未重新运行验证。
- 工作区状态事实：AGENTS.md 提交后，工作区只剩未跟踪的 `docs/*` 文件；本次追加记录不改业务代码、不运行验证、不提交。
- 剩余风险：发布规范仍属于人为流程约束，不能替代 package 脚本、CI 或 release checklist 的自动 gate；UI 人工验收、专家组审查和 release asset 对账需要每次发布时按清单真实执行，不能只因为文档存在就视为已完成。未跟踪的 `docs/*` 仍需后续按用户意图决定是否保留、提交或清理。

## 附记：Claude Code session 打开与 Session Preview 副屏定位修复

- 完成状态：complete。
- 验证状态：pass by recorded evidence；本附记未重跑验证。
- 变更范围：Bridge 0.1.11 的 Claude Code session 打开路由；Session Preview 子窗口在副屏 / DPI / 主窗口移动场景下的位置计算和 settle reposition。
- 用户原始问题 1：点击 Claude Code session card 后打开的不是 Claude Code editor。
- 根因 1：Bridge `openSession` 对 `claude-code:/...` resource 仍沿用 `workbench.action.chat.openSessionInEditorGroup`，把 Claude Code session 当成 VS Code chat session 打开。
- 修复 1：Bridge 0.1.11 识别 `claude-code` scheme，从 resource 解析 `sessionId` 后调用 `claude-vscode.editor.open(sessionId)`；本机 Anthropic Claude Code extension 实现确认签名为 `sessionId, initialPrompt, ViewColumn`。
- 用户原始问题 2：Session Preview 在副屏 hover 可以出现，但主窗口移动后位置乱飞。
- 根因 2：Preview 子窗口定位没有纳入 current monitor、DPI scale factor 和可见区域 clamp；同时 show/ready 与主窗口移动之间存在异步旧坐标竞争。
- 修复 2：按 current monitor + scale factor + visible clamp 重新计算 preview 位置，并在 show/ready 后增加 80ms / 220ms settle reposition，回收异步旧坐标。
- 交付 commit：`ab95dc4 fix(session): open Claude cards in Claude editor`。
- 记录的验证事实：`node --check` 通过；bridge routing regression 覆盖 Claude Code 和 VS Code Chat 路由；`npm run build:ui` 通过；`git diff --check` 通过；`cargo test` 18 passed（连接组件修复后、UI settle 后 Rust 未改）；`npm run package:exe` 通过；最终包内连接组件版本稳定；重复安装 VSIX 后 extension list 只剩稳定连接组件。
- 用户现场确认：Session Preview 能在副屏 hover 出现；后续位置乱飞问题已修后按用户要求提交，但未发布 / 未覆盖 GitHub Release。
- 需求到结果对账：Claude Code card 打开目标从 VS Code chat editor 改为 Claude Code editor，覆盖状态 `satisfied`；Session Preview 副屏和主窗口移动后的定位漂移已通过 monitor / DPI / clamp / settle 处理，覆盖状态 `satisfied by recorded evidence`；发布覆盖状态 `intentionally_not_done`。
- 主要偏差：本轮修复已打包并提交，但没有发布到 GitHub Release；这是用户要求的范围边界，不是遗漏。
- 剩余风险：Claude Code extension 的命令签名来自本机实现确认，若上游扩展未来变更参数顺序或命令名，需要重新适配；多显示器环境仍可能受 Windows 工作区、任务栏位置、DPI 热切换和显示器插拔影响，后续发布前应做实际多屏 smoke；本轮 GitHub Release 未发布 / 未覆盖，用户拿旧 release zip 不会获得该修复。

## 跳过记录 / 标记（Skip / Marker）

- status：not-skipped。
- marker：lightweight audit written；business code unchanged；validation not rerun；AGENTS.md release discipline committed and pushed separately；Claude Code / Session Preview bugfix retrospective appended；this retrospective append not committed。
