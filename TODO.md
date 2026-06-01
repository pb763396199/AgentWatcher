# AgentWatcher TODO

## Phase 0 - 仓库与设计冻结

- [x] 保存当前 HTML 设计稿。
- [x] 编写设计说明。
- [x] 编写完整开发 TODO。
- [x] 初始化本地 git 仓库并完成首次提交。

## Phase 1 - Tauri 2 应用壳

- [x] 创建 Tauri 2 项目骨架。
- [x] 复用 HTML 设计稿作为前端入口。
- [x] 安装 Rust/Cargo 工具链。
- [x] 安装 npm 依赖。
- [x] 验证前端构建。
- [x] 验证 Tauri release exe 构建。
- [x] 验证 MSI / NSIS 安装包打包。
- [x] 实现无边框 / 透明 / always-on-top 窗口配置。
- [x] 用 Tauri window API 接管真实窗口拖动和 resize。
- [ ] 实现窗口位置记忆。
- [ ] 实现主窗口大小记忆。
- [ ] 实现系统托盘图标和显示 / 隐藏入口。
- [ ] 实现全局快捷键显隐窗口。
- [x] 实现启动时恢复主题、语言、布局和扫描设置。

## Phase 2 - UI 组件落地

- [x] 实现 waiting / running / idle 状态筛选。
- [x] 实现 workspace 下拉过滤。
- [x] 实现每排列数控制，支持 1 到 6 列。
- [x] 实现卡片按窗口宽度自适应填满。
- [x] 实现 compact card / micro card 信息密度切换。
- [x] 实现滚动队列和底部分布条。
- [x] 实现中英双语切换。
- [x] 实现 dark / light 主题切换。
- [x] 实现横版 / 竖版布局切换并持久化。
- [x] 实现卡片 hover Session Preview。
- [ ] 实现右键菜单：复制 session id、打开 workspace、静音、隐藏 workspace。

## Phase 3 - 本机数据扫描 PoC

- [x] 扫描 VS Code `workspaceStorage`。
- [x] 解析 `workspace.json`，建立 workspace hash 到路径的映射。
- [x] 探测 Copilot Chat `chatSessions` 文件。
- [x] 只读取 metadata、mtime、文件大小和必要头尾片段，不读取完整正文。
- [x] 扫描 Claude Code projects / sessions index。
- [x] 输出 provider、workspace、session id、mtime、state 给前端模型。
- [x] 验证多 workspace session 扫描。
- [x] 建立解析失败时的 fallback 状态。

## Phase 4 - 状态机

- [x] 定义 unified session model。
- [x] 定义 waiting / running / idle 状态。
- [x] 基于 mtime 和文件增长推断 running。
- [x] 基于显式 AskQuestion / questionCarousel 推断 waiting。
- [x] 基于 idle threshold 推断 idle。
- [x] 为 Claude Code AskUserQuestion / tool_result 建立闭环判定。
- [x] 为 Copilot Chat parser 添加状态结束信号保护。
- [ ] 添加状态变化去抖，避免 UI 闪烁。

## Phase 5 - 本地缓存与设置

- [ ] 引入 SQLite 本地缓存。
- [x] 缓存 workspace alias 和设置状态。
- [ ] 缓存 session 最新状态和最后活动时间。
- [x] 设置扫描频率。
- [x] 设置 idle threshold。
- [ ] 设置是否显示系统通知。
- [ ] 设置隐私模式和路径隐藏。

## Phase 6 - 跳转能力

- [x] MVP 使用 `code --reuse-window <workspace>` 打开 workspace。
- [ ] 枚举 VS Code 进程和窗口标题，尽量聚焦已打开窗口。
- [x] 点击卡片触发 workspace / session 跳转。
- [x] 跳转失败时显示可操作错误提示。
- [x] 研究 VS Code bridge extension 可行性。
- [x] 实现 VS Code bridge extension，尝试聚焦具体 chat/session。

## Phase 7 - 通知策略

- [ ] 只在状态进入 waiting 时提醒。
- [ ] 支持 quiet mode。
- [ ] 支持 workspace 静音。
- [ ] 支持系统 toast。
- [x] 支持 rail badge 数量变化。
- [ ] 避免 running / idle 大量刷屏。

## Phase 8 - 验证与质量

- [x] 为 parser 添加单元测试。
- [x] 为状态机添加单元测试。
- [ ] 为 workspace resolver 添加样本测试。
- [ ] 做 8 小时运行内存观察。
- [ ] 验证 100%、125%、150% DPI。
- [ ] 验证窗口贴边、resize、恢复位置。
- [x] 验证无真实 session 时的空状态。
- [x] 验证大量 session 时的滚动与 preview 基础性能。

## Phase 9 - 打包发布

- [x] 准备 Windows 应用图标。
- [x] 准备 self-contained publish。
- [x] 准备 Windows zip 发行。
- [x] 添加版本号和 changelog。
- [x] 添加首次启动 Bridge 自动安装和隐私说明。

## 当前优先级

1. 完成 v0.1.2 发布前验证清单，优先确认 Bridge 安装/重装、session 跳转、handoff ack、i18n/theme 子窗口同步和 zip/SHA256 更新。
2. 验证长时间运行时的扫描和 UI 刷新稳定性。
3. 补充窗口位置记忆、托盘入口和全局快捷键。
4. 继续收敛 Session Preview 和 Handoff Panel 的真实 Tauri 交互体验。

## v0.1.2 发布验证清单

- [ ] 从发布目录启动 `AgentWatcher.exe`，确认不依赖源码目录。
- [ ] 未安装 Bridge 时自动安装成功；旧版本 Bridge 时可更新；失败时 UI 有可操作提示。
- [ ] 手动重装 `agentwatcher.agentwatcher-vscode-session-bridge-safe4` 后，session 跳转仍可用。
- [ ] 点击 Copilot / Claude session 卡片可打开目标 workspace/session；失败时显示错误。
- [ ] 右键 session 卡片可 handoff 到 Copilot、Copilot CLI 和 Claude，并收到 Bridge ack 成功确认。
- [ ] 中英切换、dark/light 切换同步到主窗口、Session Preview 和 Handoff Panel。
- [ ] Session Preview 正文只通过运行时事件传递，不写入 localStorage 或项目目录临时文件。
- [x] `npm run package:exe` 会清理旧 release zip / 解压目录，并重新生成 `AgentWatcher-v0.1.2-windows-x64.zip`。
- [x] CHANGELOG 已更新 v0.1.2 发布包和 SHA256：`34A8698ADA2922E965CC3C1D4FD9B035C76C916F46E328167BE4E8BB9F56B0FE`。
- [x] 发布包包含 `AgentWatcher.exe`、`vscode-agentwatcher-bridge/`、`agentwatcher-bridge-<version>.vsix`。
- [ ] docs 是否进入 git 由用户在发布前确认，不作为当前实现默认假设。

## Future / Post-v0.1.x

- [ ] 设计 prompt 非剪贴板通道；本次 v0.1.2 不实现，也不作为发布阻断。
- [ ] 评估 VS Code command arg、AgentWatcher runtime temp file + token、localhost bridge、named pipe 等方案。
- [ ] 如果未来使用临时文件，禁止写入项目目录；只能写入 AgentWatcher 自身运行时临时目录或系统临时目录，例如 `%TEMP%\AgentWatcher\...`。
- [ ] 为 prompt 临时文件方案补 token 防复用、TTL 过期删除、成功读取后删除、启动时清理陈旧文件。
- [ ] 扩展发布验证到 VS Code Stable/Insiders、100%/125%/150% DPI、8 小时长时间运行。

## Phase 1b - Tauri 2 主应用壳

- [x] 创建 Tauri 2 项目骨架。
- [x] 复制 HTML 设计稿为 Tauri 前端入口。
- [x] 安装 Rust/Cargo 工具链。
- [x] 运行 `npm install`。
- [x] 验证 `npm run dev:ui`。
- [x] 验证 `npm run build`。
- [x] 将 HTML 中的桌面背景 mock 拆分为真实窗口 UI。
- [x] 用 Tauri window API 接管四角 resize、拖动、最小化、最大化、关闭。
- [x] 建立 JS 与 Rust backend 的 command/event 通信。

## 已放弃路线

- WPF 纯控件复刻 HTML UI：视觉对齐成本过高，已删除未提交的 WPF 临时项目。