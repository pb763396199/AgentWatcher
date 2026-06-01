# Changelog

## v0.1.2 - 2026-06-01

- 新增 full-path workspace grouping：同名 workspace 使用完整路径区分，减少跨目录 session 混淆。
- 新增卡片右键接续能力，可从当前 session 发起跨 provider handoff。
- 热修替换现有 v0.1.2 release asset，不新增 app/release 版本号，避免用户看到新增版本。
- 修复 Bridge 唯一扩展身份：唯一支持 ID 固定为 `agentwatcher.agentwatcher-vscode-session-bridge`。
- Bridge VSIX package name 稳定为 `agentwatcher-vscode-session-bridge`，Bridge 自身版本保持 `0.1.10`。
- Bridge command namespace 稳定为 `agentwatcherSessionBridge.*`。
- Bridge install/update 会在安装稳定 VSIX 前 best-effort 清理历史 `safe1` / `safe2` / `safe3` / `safe4` 扩展。
- Bridge 安装后会校验稳定 ID 已安装且 legacy 临时 ID 不再残留，避免重复生成新的临时扩展身份。
- 新增 Copilot、Copilot CLI、Claude 目标路由，handoff 可按目标 provider 打开新会话并插入 prompt。
- `launch_handoff` 改为后台执行，等待 Bridge ack 时不阻塞主 UI。
- 修正子窗口设置同步：language/theme/layout/always-on-top 会同步到 `session-preview` 和 `handoff-panel`。
- 发布打包脚本会清理旧 release zip / 解压目录，清理 Bridge 输出目录旧 VSIX，并生成当前版本 Windows zip。
- 明确 clipboard bridge 真实口径：当前 prompt 插入仍依赖 clipboard，非剪贴板通道列入 Future。

发布包：`AgentWatcher-v0.1.2-windows-x64.zip`

SHA256：`866522276A933B1C57E173AAF5B6CAB6727463AC98CB1A34ABE1297A25DAB1BB`

验证：`cargo test --manifest-path src-tauri\Cargo.toml`、`node --check vscode-agentwatcher-bridge\extension.js`、`node .tmp\bridge-handoff-routing-test.cjs`、`git diff --check`、`npm run package:exe`、zip/manifest/SHA256 检查、重复安装最终 VSIX 后只保留稳定 Bridge ID。

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
