# Changelog

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
