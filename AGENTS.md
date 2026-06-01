# AGENTS.md

AgentWatcher 是一个 Windows 桌面小工具，用 Tauri 2 写的。它盯着 VS Code 里的 Copilot 和 Claude Code 两个 AI 助手，把所有工作区里的 session 状态都拉出来，按"等回复 / 跑着 / 闲着"分类，显示成一个个小卡片。点卡片就能跳到对应的 session，悬停还能看摘要。

这个仓库没有 OpenCode 或 Cursor 的配置文件，AGENTS.md 是唯一的指令文件。改东西请在 `dev` 分支上，动手前先跑 `git status --short --untracked-files=all` 看清楚当前工作区。

## 怎么跑起来

所有命令都在仓库根目录、用 PowerShell 跑，必须是 Windows。

- `npm install` —— 装依赖，只跑一次。
- `npm run dev` —— `tauri dev`：会先打包 `ui/`，起 Vite 在 `http://127.0.0.1:1420`，然后拉起 Tauri 桌面壳。
- `npm run dev:ui` —— 只起 Vite，纯浏览器预览在 `:1420`，没有 Tauri 接口。做纯 UI 时用这个。当 `window.__TAURI_INTERNALS__` 不存在时，页面会自动用 mock 数据兜底。
- `npm run build:ui` —— 用 Vite 把 `ui/` 打到 `ui-dist/` 目录（这个目录是 git 忽略的）。
- `npm run build` —— `tauri build`，跑 Rust + 打包。生成的 exe 在 `src-tauri/target/release/agentwatcher.exe`。
- `npm run generate:icons` —— 重新生成 `src-tauri/icons/icon.ico` 和 `icon-runtime-256.rgba`。**只能在 Windows 上跑**，里面调了 `pwsh` 用 .NET 的 `System.Drawing` 画 Segoe UI 字体。其他系统会报错。
- `npm run package:exe` —— 完整发布流水线：编译、用 `rcedit` 把图标塞进 exe、复制 `vscode-agentwatcher-bridge/`、用 `npx @vscode/vsce` 打 VSIX、最后输出 `artifacts/AgentWatcher/AgentWatcher.exe` + `vscode-agentwatcher-bridge/agentwatcher-bridge-0.1.10.vsix`，再压成 `artifacts/AgentWatcher-v<版本>-windows-x64.zip`。跑这个前会先把老的 `artifacts/AgentWatcher-v*-windows-x64{,.zip}` 清掉。
- Rust 单元测试：`cargo test --manifest-path src-tauri/Cargo.toml`（测试代码就在 `src-tauri/src/lib.rs` 文件最末尾）。
- Bridge 语法检查：`node --check vscode-agentwatcher-bridge\extension.js`。
- Bridge 路由测试：`node .tmp\bridge-handoff-routing-test.cjs`。

## 目录结构

- `src-tauri/src/lib.rs` —— 唯一的 Rust 文件，4678 行左右。`pub fn run()` 是 Tauri 的入口，所有 `#[tauri::command]` 都注册在文件末尾。所有 session 解析、状态机、bridge 版本管理、handoff 确认逻辑全在这里。
- `src-tauri/src/main.rs` —— 5 行，就一句 `agentwatcher_lib::run()`。
- `src-tauri/capabilities/{default,session-preview,handoff-panel}.json` —— Tauri 2 的权限文件，对应三个 webview 窗口：`main`（主窗口）、`session-preview`（悬浮预览）、`handoff-panel`（接续对话框）。
- `ui/index.html` —— 整个前端就这一个文件，5280 行左右，纯 HTML+CSS+JS，没有任何框架。Tauri 接口是动态 import 的，只有检测到 `window.__TAURI_INTERNALS__` 才加载。
- `vscode-agentwatcher-bridge/` —— VS Code 扩展本体。`extension.js` 是 CommonJS 模块，用 `require('vscode')`，没有 TypeScript。Bridge 自己有个版本号（`0.1.10`），跟 App 版本号没关系，最后会打成 VSIX 跟 `AgentWatcher.exe` 放一起发布。
- `scripts/{package-exe,generate-icons}.mjs` —— 发布相关的脚本。
- `ui-dist/`、`src-tauri/target/`、`src-tauri/gen/`、`artifacts/`、`.tmp/`、`.history/`、`vscode-agentwatcher-bridge/*.vsix` —— 都是 git 忽略的构建产物。
- `docs/{plans,retrospectives,insights}/` —— 没被 git 跟踪的规划笔记，要不要进 git 要先问用户。

## Tauri 命令（前端 → Rust）

命令都注册在 `src-tauri/src/lib.rs:4643`。前端用 `@tauri-apps/api/core` 里的 `invoke('name', { ... })` 调。事件名：`agentwatcher-sessions-changed`（被监控的 JSONL 文件变了之后 250 ms 防抖发一次，被监控的目录在 `session_watch_roots()` 里）。

- `scan_sessions({ options?: ScanOptions })` → `AgentSession[]`。`ScanOptions` 字段：`maxSessions`、`activeWindowDays`（夹在 1 到 30 之间）、`hideArchived`、`includeCopilot`、`includeClaude`。默认最大 80 个、活跃窗口 7 天、其余三个默认 `true`。
- `open_session({ id?, provider?, workspacePath?, sessionResource? })` → 先试 bridge 的 `vscode://…/open?resource=…&target=editor`，不行再试 `vscode://file<path>?session=…` 深链接，最后兜底用 `code --agents <path>`。每一步失败都会降级。
- `launch_handoff({ mode, workspacePath })` → 先开新窗口，再发 `vscode://…/handoff?command=…&insertPrompt=1&ackPath=…&ackToken=…`。`mode` 三个值：`agents` → `workbench.action.chat.openNewSessionEditor.local`，`code-chat` → `…openNewSessionEditor.copilotcli`，`claude-panel` → `claude-vscode.editor.open`。然后等 Bridge 写 ack 文件，最多等 20 秒。
- `get_bridge_status()` → 返回 `{ installed, needsUpdate, localVersion, installedVersion, codePath, message }`。`needsUpdate` 为真的条件：装的版本比本地的旧 *或者* 任何历史 safe ID 还在。
- `install_bridge()` → 先卸掉历史 safe ID，找目录里的 `agentwatcher-bridge-<版本>.vsix`，找不到就用 `npx --yes @vscode/vsce package` 打到 `%TEMP%\AgentWatcher\` 兜底，然后跑 `code --install-extension … --force`，最后再验一遍：稳定 ID 必须装上了、历史 safe ID 一个都不准剩。任何一个没满足都报错。
- `set_window_always_on_top({ alwaysOnTop })` → 调 `window.setAlways_on_top`，Windows 下还会再调一次 `SetWindowPos(HWND_TOPMOST|HWND_NOTOPMOST)` 作用到根窗口上。光靠 Tauri 2 的 flag 在无边框小窗上不靠谱，必须多这一步。

VS Code CLI 查找顺序（在 `find_code_cli_path` 里）：先 PATH 里的 `code` / `code.cmd` / `code.exe`，然后 `%LOCALAPPDATA%\Programs\Microsoft VS Code\bin\code.cmd` → `…\Code.exe` → `C:\Program Files\Microsoft VS Code\bin\code.cmd` → `…\Code.exe` → x86 版本。

## 不能随便改的标识符

这些是发布合同的一部分，改了的话，已经装了的 Bridge、已经发布的 VSIX 全部会坏。

- Bridge 扩展 ID：`agentwatcher.agentwatcher-vscode-session-bridge`（唯一支持的 ID）。
- Bridge 包名：`agentwatcher-vscode-session-bridge`。
- Bridge 版本：`0.1.10`（跨多个 App 版本一直保持不变，别跟着 App 版本一起升）。
- Bridge 命令命名空间：`agentwatcherSessionBridge.openSession` / `.runCommand` / `.runHandoffCommand`。
- Bridge URI 授权段：`agentwatcher.agentwatcher-vscode-session-bridge`（用在 `vscode://…/open|command|handoff` 里）。
- 历史 ID `safe1`/`safe2`/`safe3`/`safe4`（完整字符串在 `LEGACY_BRIDGE_EXTENSION_IDS`）必须继续被检测和卸载，**不要新增任何 "safe" ID**。
- App 标识符：`com.agentwatcher.desktop`。

## Session 数据源（只在 Windows 下有效）

扫描逻辑在 `session_watch_roots()`、`scan_copilot_sessions`、`scan_claude_sessions` 里。

- Copilot Chat：`%APPDATA%\Code\User\workspaceStorage\*\chatSessions\*.jsonl`（也扫 `Code - Insiders`）。
- Copilot 转录覆盖文件（用来判断 waiting/running 状态）：`<workspaceStorage>\<hash>\GitHub.copilot-chat\transcripts\<sessionId>.jsonl`。
- Claude Code：`%USERPROFILE%\.claude\projects\*\*.jsonl`，加上每个项目里的 `sessions-index.json` 拿预索引的 session。Insiders 路径走扩展目录，不是 projects 目录。

采样大小（都是 `lib.rs` 里的常量）：JSONL 头读 128 KB，尾读 256 KB，标题采样 240 行，Copilot prompt 扫 16 MB，Claude prompt 扫 8 MB，transcript 扫 2 MB。**只看头尾，不读全文**。

## 状态机

状态固定三档：`waiting` > `running` > `idle`（用 `status_rank` 排序）。Waiting 不是看文件最后修改时间认的，**必须**是显式的"在问用户问题"才算：

- Copilot `waiting`：当前响应里有没解决的 `questionCarousel`，或者 `vscode_askQuestions` 工具调用还 `!isComplete && !isUsed && !isSkipped`。一旦出现 `modelState.completedAt`、`result`、`followups`、`elapsedMs`，就清掉之前的 waiting 标记。
- Claude `waiting`：`AskUserQuestion` 的 `tool_use` 进到了 `pending_ask_user_question_ids`（还没被同 `tool_use_id` 的 `tool_result` 关掉）。`toolUseResult.answers == []` 或者 `isSkipped` 都会把它清掉。
- `running`：10 分钟内有内容时间戳（`STATUS_RECENT_CONTENT_WINDOW_MS`），或者 3 分钟内有文件修改（`STATUS_RECENT_FILE_WINDOW_MS`），或者 2 小时内有 Running 标记（`STATUS_RUNNING_HINT_WINDOW_MS`）。
- 跳过的回答（`skip` / `skipped` / `跳过` / `no_answer` 等等，定义在 `is_skip_sentinel`）和 AI 模型的废话（`is_ai_model_noise_text`）一律在生成预览时过滤掉，也永远不会让状态机误判。

## 前端怎么和 Rust 配合

`ui/index.html` 是单文件 SPA。`window.__TAURI_INTERNALS__` 不存在时走 mock 模式，存在就动态 `import('@tauri-apps/api/...')`。运行时它会通过 `WebviewWindow` 新建两个 Tauri webview 窗口（label 是 `session-preview` 和 `handoff-panel`），给它们发事件（`agentwatcher-preview-data`、`agentwatcher-handoff-data`、`agentwatcher-runtime-settings`）。URL 上加 `?preview=1` 或 `?handoff=1` 会让同一个 HTML 文件直接以预览或接续模式启动（body 加 `is-preview-window` / `is-handoff-window` class）。

## 隐私和契约约束（代码里强制）

- Session Preview 的正文**只能**通过 Tauri 事件传，绝不能写 `localStorage`（localStorage 里只准存尺寸，对应 key 是 `agentwatcher.sessionPreviewSize`）。
- Handoff ack 文件**只接受**写在 `<系统临时目录>\AgentWatcher\handoff-acks\` 下面。Bridge 那边硬性校验 `path.resolve(ackPath).startsWith(path.resolve(os.tmpdir()) + 'AgentWatcher' + sep)`。
- 接续 prompt 现在是走剪贴板的（Bridge 用 `vscode.env.clipboard.readText()` 读）。不走剪贴板的通道是 Future，不阻塞发版。

## 怎么测

- Rust 单元测试在 `lib.rs:4333-4636`，覆盖了 Bridge 版本管理、状态转换、transcript 覆盖、工作区身份（UGA 分支、通用插件根、路径归一化）和预览文本提取。跑：`cargo test --manifest-path src-tauri/Cargo.toml`。
- `.tmp/bridge-handoff-routing-test.cjs` 是 Node 写的测试，伪造 `vscode` 模块，验证 Bridge 命令路由（`agents` / `code-chat` / `claude-panel` 三种），包括 `claude-vscode.editor.open` 走剪贴板那条路径和 `safe1` 风格的兜底。它还顺带测了 `AGENTWATCHER_TARGET_BOUND_SENTINEL` 这个 Copilot 目标的正常路径。
- 人工验收清单在 `docs/retrospectives/`：VSIX 必须叫 `agentwatcher-bridge-0.1.10.vsix`；`code --list-extensions | findstr agentwatcher` 应该只显示稳定 ID，不准出现 `safe1..4`；每次发版的 SHA256 要写进 `CHANGELOG.md`。

## 打包时容易踩的坑

- Tauri 编出来的 exe 叫 `agentwatcher.exe`（小写），`scripts/package-exe.mjs` 会拷一份重命名为 `AgentWatcher.exe`，这个过程会先用 `rcedit` 把图标塞进 exe。
- 最终的 exe 里必须嵌全 15 个图标尺寸（`scripts/package-exe.mjs` 会重新解析 PE 资源目录，少任何一个尺寸就报错）：16、20、24、30、32、36、40、48、60、64、72、80、96、128、256。
- `rcedit` 是 `package.json` 里的 devDependency（`package-exe.mjs` 会用）。
- 发布 zip 名字用根目录 `package.json` 里的 version，把不合规字符 `[^a-zA-Z0-9._-]` 替换成 `_`。
- `npm run package:exe` 会删掉 `artifacts/` 下面所有老的 `AgentWatcher-v*-windows-x64{,.zip}`，只留当前版本的 `artifacts/AgentWatcher/` 暂存目录。

## 版本号规则

- App 版本和 Bridge 版本**完全独立**。v0.1.x 的热修发布就直接覆盖上一个 `AgentWatcher-v0.1.2-windows-x64.zip`，不会让用户看到版本号往前跳。
- `CHANGELOG.md` 里要写发布的 zip 名字和对应的 SHA256。每次 `npm run package:exe` 跑完产物变了，要顺手同步过去。

## 发布规范

按这个顺序走，不要只打个包就说发布好了。

1. 先定范围和版本。
	- 先说清楚这次是“新版本”还是“原地热修”。没明确要新版本，就别随手把 `0.1.2` 改成 `0.1.3`。
	- 对齐 App 版本、Bridge 版本、Git tag、zip 名、CHANGELOG。App 版本看 `package.json`、`src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`；Bridge 版本看 `vscode-agentwatcher-bridge/package.json`。
	- Bridge ID 不能拿来破缓存。`agentwatcher.agentwatcher-vscode-session-bridge` 是唯一发布 ID；`safe1` 到 `safe4` 只准当历史垃圾清理，不准再打进发布包。

2. 先清文档和旧口径。
	- 发布前同步 `README.md`、`CHANGELOG.md`、`vscode-agentwatcher-bridge/README.md`、`TODO.md`，必要时看 `DESIGN.md` 和 `AGENTS.md`。
	- 搜旧版本号、旧 Bridge ID、`safe*`、旧命令名、旧路径、过时注释。能改准就改准，不能闭环就写到 Future / Known Issues，别装作完成。
	- `CHANGELOG.md` 必须写最终 zip 名和 SHA256；每次重打包，SHA 都要重算。

3. 让“专家组”按角色过一遍。
	- Release 看版本、tag、zip、SHA、Release Note。
	- Bridge 看 VSIX、extension ID、package name、command namespace、安装/清理/重复安装。
	- UI 看中文/英文、dark/light、横版/竖版、always-on-top、主窗口、Session Preview、Handoff Panel。
	- Docs 看 README/CHANGELOG/TODO/Bridge README 有没有互相打架。
	- Privacy/Security 看 preview 正文、剪贴板、临时文件、Bridge command whitelist。
	- QA 看从发布目录启动、点击 session、右键 handoff、失败提示。

4. 做 code review 和 code simplify。
	- 专门看发布风险：Bridge 安装清理、session 跳转、handoff ack、i18n/theme 同步、隐私边界、错误提示。
	- 删临时调试、硬编码本机路径、废弃 fallback、重复状态判断、过时注释。临发布前不要做大重构。
	- 构建通过不等于 review 通过，最终 zip 正确也不等于用户流程正确。

5. 做 UI 人工验收。
	- 中文和英文都点一遍；dark 和 light 都点一遍；横版和竖版都点一遍。
	- 主窗口、Session Preview、Handoff Panel 都要看语言、主题、布局、置顶设置是否同步。
	- 看长 workspace 名、长错误、长按钮文案、滚动、右键菜单、preview/handoff 是否重叠或溢出。
	- DPI 100% / 125% / 150% 能测就测；8 小时长跑没测就写“未测”，别写成已完成。

6. 跑自动化并打包。
	- 至少跑：`cargo test --manifest-path src-tauri/Cargo.toml`、`node --check vscode-agentwatcher-bridge\extension.js`、`node .tmp\bridge-handoff-routing-test.cjs`、`git diff --check`、`npm run package:exe`。
	- UI 变动明显时再跑 `npm run build:ui`；发布包必须用 `npm run package:exe` 生成。

7. 只认最终 artifact，不只认源码。
	- 检查 `artifacts/AgentWatcher/AgentWatcher.exe`、最终 zip、zip 里的 `vscode-agentwatcher-bridge/package.json`、VSIX 文件名和 SHA256。
	- VSIX 要重复安装两次：`code --install-extension artifacts\AgentWatcher\vscode-agentwatcher-bridge\agentwatcher-bridge-0.1.10.vsix --force`。
	- 装完 `code --list-extensions --show-versions | findstr agentwatcher` 只能看到 `agentwatcher.agentwatcher-vscode-session-bridge@0.1.10`，不能有任何 `safe*`。
	- 从 `artifacts/AgentWatcher/AgentWatcher.exe` 启动，确认不依赖源码目录。

8. 发布和 Release Note。
	- Release Note 跟旧版本格式一致：标题用 `AgentWatcher vX Windows 发布版`，正文用中文，保留 `主要更新`、`发布包`、`压缩包内容`、`验证记录`。
	- 如果是原地替换 GitHub Release，先确认旧 asset 下载量很低或为 0；替换后用 `gh release view` 核对 asset digest 和本地 SHA256 一样。

9. 发布后回看。
	- 再看 GitHub 页面、远端 tag、release asset、下载包 digest、本地 `git status`。
	- `artifacts/`、`ui-dist/`、`target/`、VSIX、`.tmp/` 不能混进提交。
	- 没测过的东西不要写成“已完成”。`docs/` 和 AGENTS 这种辅助文档默认不跟代码一起提交；用户明确要加再加。

## 分支和提交

- 当前分支是 `dev`。最近的提交都是 Conventional Commits（`fix(bridge): …`、`feat(handoff): …`、`chore(release): …`）。
- `node_modules/`、`ui-dist/`、`src-tauri/target/`、`src-tauri/gen/`、`artifacts/`、`vscode-agentwatcher-bridge/*.vsix`、`.tmp/` 都不能提交，都在 git 忽略里。`docs/` 目录也没被跟踪，要加进去得先问用户。
