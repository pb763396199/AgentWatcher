# Changelog

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
