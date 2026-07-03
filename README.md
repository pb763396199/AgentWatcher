# AgentWatcher

AgentWatcher 是一个 Windows 桌面悬浮小工具，目标是像输入法候选窗一样常驻在屏幕任意位置，监控以下来源在所有 workspace 的会话状态，并把 waiting / running / idle 会话以可跳转的卡片展示出来：
- VS Code Copilot 会话
- VS Code Claude Code 会话
- Codex 桌面会话

当前发布版本：v0.1.3。仓库已切换到 Tauri 2 作为主应用壳方向，前端入口 [ui/index.html](ui/index.html) 是唯一 UI 源文件。仓库只保留源码和必要的源码内资产，release 产物通过打包脚本按需生成。

## 当前 UI 入口

- Tauri 前端入口：[ui/index.html](ui/index.html)
- 设计说明：[DESIGN.md](DESIGN.md)
- 开发清单：[TODO.md](TODO.md)

## 已验证的界面能力

- Windows 悬浮应用窗口外观，贴近 VS Code 视觉语言。
- 四角 resize，窗口可任意方向缩放；Tauri 运行态通过 window API 接管。
- 左侧 AW 折叠面板贴着主窗口左侧同步移动。
- session 卡片按“每排数量”自适应填满卡片区域。
- 超量 session 使用优先级滚动队列展示。
- workspace 过滤、状态过滤、中英切换、明暗主题切换。
- 同名 workspace 会按 full path 分组和区分，避免不同目录显示为同一个 workspace。
- 永远置顶可在设置面板中实时开关，并持久化偏好。
- 设置面板支持横版 / 竖版布局切换，并持久化布局偏好。
- Session Preview 通过卡片右下角展开按钮悬停触发，显示最近用户输入和 AI 正文摘要。
- Tauri 运行态使用无标题栏、无任务栏的悬浮 preview webview，避免预览被 Watcher 主窗口裁剪。
- Session Preview 支持独立滚动、四角 resize 和尺寸持久化；预览正文只通过运行时事件传递，不写入 localStorage。
- Handoff Panel 会同步主窗口 language/theme/layout/always-on-top 设置。
- 右键 session 卡片可发起 handoff，并按目标路由到 Copilot、Copilot CLI 或 Claude。
- VS Code 连接组件会通过 ack 文件回报 handoff 成功或失败，`launch_handoff` 等待确认时不阻塞主 UI。
- 运行态页面移除了 VS Code 背景 mock 和 Windows taskbar mock，只保留透明悬浮工具 UI。
- 支持主窗口整页缩放（Ctrl + / - / 0、Ctrl+滚轮），并持久化缩放比例（25%~500%）。
- 新增 AgentTask / Todo 面板入口，任务流与会话卡片联动；任务状态变化可驱动会话归档策略。
- 新增 performance-panel，支持 AgentWatcher、VS Code、Codex 进程采样、CPU 历史、列宽记忆和快照复制。
- 接入 Codex 桌面会话扫描，支持与 Copilot / Claude 一起纳入 Watcher 和任务链路。

## 技术方向

使用 Tauri 2 实现轻量桌面壳，前端由 [ui/index.html](ui/index.html) 提供。MVP 采用本地文件扫描采集 Copilot / Claude Code session 元数据，配合 VS Code bridge extension 实现精确 session 跳转。

## 功能特性

- **实时监控**：自动扫描 VS Code Copilot 和 Claude Code 的所有 workspace sessions
- **状态分类**：按 waiting（待回复）、running（运行中）、idle（闲置）分类显示
- **一键跳转**：点击卡片直接打开对应的 VS Code session
- **悬浮预览**：悬停卡片右下角展开按钮显示最近用户输入和 AI 正文摘要，预览窗口可 resize 并记忆尺寸
- **Handoff**：右键 session 卡片可携带上下文接续到 Copilot、Copilot CLI 或 Claude，新会话由目标 provider 负责承接
- **Codex 会话接入**：接入 Codex 桌面会话，支持与 Copilot、Claude 一起显示状态与跳转
- **VS Code 连接组件**：首次启动自动安装连接组件，实现精确 session 跳转、handoff 路由和 ack 成功确认
- **AgentTask / Todo**：新增 AgentTask 与 Todo 面板入口，任务状态与会话状态联动
- **悬浮窗口**：Windows 悬浮应用，四角 resize，可任意方向缩放
- **置顶控制**：设置面板内可实时打开或关闭 always-on-top
- **布局切换**：设置面板内支持 Horizontal / Vertical 两种窗口布局，并记忆偏好
- **智能过滤**：workspace 过滤、状态过滤、full-path workspace grouping、自动隐藏归档 sessions
- **多语言**：中英文切换
- **明暗主题**：支持 Dark / Light 主题切换
- **窗口缩放**：支持 Ctrl + / - / 0 与 Ctrl+滚轮缩放主窗口，并可在设置中配置缩放比例（默认 100%）
- **性能诊断**：新增 performance panel，支持进程树采样与 CPU 历史（含 AgentWatcher / VS Code / Codex）

## 快速开始

### 开发运行

当前机器已安装 Rust/Cargo，可以运行完整 Tauri 桌面壳：

```powershell
npm install
npm run dev
```

`npm run dev` 默认会给 Windows WebView2 打开本地 CDP 调试端口，并把端口信息写到 `.tmp/tauri-dev-runtime.json`；真实交互测试会优先附着这条 dev 运行时。

### 打包发布版本

构建 release 版本并打包为可分发的 exe：

```powershell
npm run package:exe
```

打包完成后，可分发的文件位于：

```
artifacts/AgentWatcher/
  AgentWatcher.exe
  vscode-connector/
    connector package files
```

用户只需将整个 `artifacts/AgentWatcher/` 文件夹复制到目标机器即可使用。发布目录会包含预打包的 VS Code 连接组件，正常情况下目标机器不需要安装 Node.js 或 `npx`。

GitHub Release 使用 zip 分发，命名格式为：

```
artifacts/AgentWatcher-v<version>-windows-x64.zip
```

打包脚本生成当前版本 zip 前，会清理 `artifacts/` 根目录下旧的 `AgentWatcher-v*-windows-x64.zip` 和 `AgentWatcher-v*-windows-x64` 目录，但会保留 `artifacts/AgentWatcher/` 作为当前输出目录。

### 验证前端设计稿

只验证前端设计稿时可以运行：

```powershell
npm run dev:ui
```

然后访问 `http://127.0.0.1:1420`。

## VS Code 连接组件

AgentWatcher 使用自带的 VS Code 连接组件实现精确 session 跳转和 handoff 路由。组件由 AgentWatcher 自动安装、更新和校验；安装失败时会在设置面板给出可操作提示。

### 自动安装行为

- **首次启动**：AgentWatcher 启动后会在后台自动检查 VS Code 连接组件状态。
- **自动安装条件**：
  - VS Code 连接组件未安装。
  - 或本地组件比已安装组件更新。
  - 或检测到历史测试组件残留，需要清理。
- **安装过程**：
  - 安装前先通过 VS Code CLI 尝试清理历史测试组件。
  - 优先安装随发布包一起提供的预打包连接组件。
  - 开发目录中没有预打包组件时，才回退到现场打包。
  - 安装后校验稳定组件已安装，且历史测试组件不再残留。
  - 安装在后台线程进行，不阻塞主界面使用。

### 手动管理

在设置面板中，可以查看 VS Code 连接组件状态并手动安装或更新。

### 故障排查

**连接组件安装失败**：

1. **检查 VS Code CLI**：
   ```powershell
   code --version
   ```
   如果提示找不到命令，需要将 VS Code bin 目录添加到 PATH，或从 VS Code 内部运行 "Shell Command: Install 'code' command in PATH"。

2. **检查随包组件**：
   确认发布目录下存在 VS Code 连接组件。源码目录缺少预打包组件时，才需要 Node.js 和 `npx` 现场打包。

3. **检查 Node.js 和 npx（仅源码回退打包需要）**：
   ```powershell
   npx --version
   ```

4. **手动安装连接组件**：
   ```powershell
   code --install-extension <连接组件 VSIX> --force
   ```

5. **确认当前组件唯一**：
   ```powershell
   code --list-extensions | findstr agentwatcher
   ```
   预期只看到当前稳定连接组件，不应再出现旧的 AgentWatcher 测试组件。

**Session 跳转失败**：

- 确保 VS Code 连接组件已启用。
- 重启 VS Code：有时需要重启 VS Code 使组件生效。
- 检查 session 路径：确保 session JSONL 文件存在且可访问。

## Known Issues

- Prompt handoff 的提示词插入目前整体仍依赖 clipboard 通道：Copilot、Copilot CLI 和 Claude 都由 VS Code 连接组件读取 clipboard 后，再通过各自的目标命令填入。非剪贴板 prompt 通道列入 Future，不作为本次 v0.1.3 发布阻断。
- 未来如果引入 prompt 临时文件，禁止写入项目目录，只允许写入 AgentWatcher 自身运行时临时目录或系统临时目录，例如 `%TEMP%\AgentWatcher\...`，并需要 token、TTL 和读取后清理。
- 历史 VS Code 连接组件测试扩展可能残留在开发机上；当前发布只以稳定组件为准，install/update 会尝试清理旧 ID。
- docs 是否进入 git 仍由发布前人工决定，当前文档仅记录准备状态，不默认改变仓库策略。

## Post-v0.1.x Roadmap

- 设计并验证非剪贴板 prompt handoff 通道，优先评估 VS Code command arg、AgentWatcher runtime temp file + token、localhost bridge、named pipe。
- 增加窗口位置记忆、系统托盘入口和全局快捷键。
- 补充状态变化去抖、workspace 静音、quiet mode 和系统 toast。
- 引入本地缓存，减少长时间运行时重复解析 session 文件的成本。
- 扩展干净机器发布验证矩阵，覆盖 VS Code Stable/Insiders、100%/125%/150% DPI 和长时间运行。

## 发布验证清单

- [ ] 从发布目录启动 `AgentWatcher.exe`，确认不依赖源码目录。
- [ ] 未安装 VS Code 连接组件时自动安装成功；旧版本连接组件时可更新；失败时 UI 有可操作提示。
- [ ] 手动 `code --install-extension <连接组件 VSIX> --force` 可重装 VS Code 连接组件。
- [ ] 点击 Copilot / Claude session 卡片可打开目标 workspace/session；失败时显示错误。
- [ ] 右键 session 卡片可 handoff 到 Copilot、Copilot CLI 和 Claude，成功后 UI 收到连接组件 ack。
- [ ] 中英切换、dark/light 切换能同步到主窗口、Session Preview 和 Handoff Panel。
- [ ] Session Preview 内容只通过运行时事件传递，不写入 localStorage 或项目目录临时文件。
- [ ] 发布包包含 `AgentWatcher.exe` 和 VS Code 连接组件。

## 数据来源

### Copilot Sessions

扫描路径：`%APPDATA%\Code\User\workspaceStorage\*\chatSessions\*.jsonl`

- 从 `workspace.json` 读取 workspace 路径
- 从 JSONL 文件头尾采样读取 session 元数据
- 支持缓存机制：基于文件长度和修改时间判断是否需要重新解析

### Claude Code Sessions

扫描路径：`%USERPROFILE%\.claude\projects\*\*.jsonl` 和 `sessions-index.json`

- 支持 JSONL 文件直接解析
- 支持从 `sessions-index.json` 读取预索引的 session 信息
- 自动提取 workspace 路径、分支名、消息计数等元数据

### Session 状态判断

- **waiting**：只认显式等待用户输入的工具状态。Copilot 需要当前有效响应仍停在 `vscode_askQuestions` 对应的 `questionCarousel`，或 `vscode_askQuestions` 尚未完成；Claude Code 需要存在未被同 `tool_use_id` 的 `tool_result` 关闭的 `AskUserQuestion`。历史上出现过这些字段但后续已继续运行，不算 waiting。
- **running**：检测到最近的 assistant 活动，或当前最新有效响应仍在运行。
- **idle**：超过近期活跃窗口后没有新的内容时间戳或文件修改。
- 已完成的 Copilot 请求会通过 `result`、`followups`、`elapsedMs`、`modelState.completedAt` 等结束信号清理旧 waiting，skipped / skip / 跳过等回答不会被当作用户输入。

### Codex Sessions

- 通过 Codex app-server 获取当前桌面会话摘要与状态，并按会话时间与活跃度接入统一状态模型。
- Codex 会话在主列表与 AgentTask 流程中与 Copilot/Claude 表现一致：支持筛选、跳转与 handoff 流程对齐。

### Session Preview

- 卡片本身点击仍然用于跳转 session；右下角展开按钮只负责预览。
- 浏览器预览模式使用 DOM fixed overlay；Tauri 运行态使用单独的 frameless `session-preview` webview，解决主窗口边界裁剪。
- preview webview 默认隐藏，收到 ready 信号后再显示，避免首次弹出时闪出完整 Watcher 页面。
- preview webview 会复用并 hide / show，避免 hover 时频繁创建和销毁窗口。
- 最近用户输入和 AI 正文摘要只保存在当前 JS 运行时并通过 Tauri event 传递；localStorage 只保存 preview 尺寸。

### 扫描配置

在设置面板可配置：

- **Layout**：横版 / 竖版窗口布局，适配侧边停靠或窄窗贴边使用
- **Active window (days)**：活跃窗口天数，默认 7 天
- **Max sessions**：最大显示 session 数，默认 80
- **Refresh interval (sec)**：刷新间隔秒数，默认 15 秒
- **Hide archived**：隐藏已归档的 sessions
- **Copilot sessions** / **Claude sessions**：是否包含对应 provider 的 sessions
- **Codex sessions**：是否包含 Codex 会话来源

## 架构说明

### Rust Backend (src-tauri/src/lib.rs)

- `scan_sessions`：扫描并返回所有符合条件的 sessions
- `scan_app_server_sessions`：扫描 Codex app-server 会话并合并入统一会话列表
- `open_session`：通过 VS Code CLI 或 deep link 打开指定 session
- `get_bridge_status`：检查 bridge extension 状态
- `install_bridge`：打包并安装 bridge extension
- 自动安装机制：`auto_install_bridge_on_startup` 在后台线程检查并安装

### Frontend (ui/index.html)

- 单文件 HTML + CSS + JS：无需构建即可在浏览器预览
- 使用 Tauri invoke API 与 Rust backend 通信
- 支持 browser prototype 模式：无 Tauri 时显示 mock 数据
- 实时刷新：按配置间隔自动扫描和更新 sessions
- 管理横版 / 竖版窗口布局切换和设置持久化
- 管理 Session Preview 的 DOM fallback 和 Tauri `session-preview` tooltip webview
- 管理 Handoff Panel、跨 provider handoff 目标和连接组件 ack 状态提示
- 管理 AgentTask / Todo / Performance 面板的子窗口事件、同步和持久化交互

### VS Code 连接组件

- 监听 AgentWatcher 的 VS Code 跳转 URI
- 解析 session resource 并打开对应的 chat session
- 支持 Copilot、Copilot CLI 和 Claude Code handoff 目标
- 通过 AgentWatcher 临时目录 ack 文件回传 handoff 成功或失败

## 许可证

UNLICENSED（私有项目）

## Tauri 开发

此部分说明已整合到"快速开始"章节。
