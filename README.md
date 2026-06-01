# AgentWatcher

AgentWatcher 是一个 Windows 桌面悬浮小工具，目标是像输入法候选窗一样常驻在屏幕任意位置，监控 VS Code 中 Copilot 和 Claude Code 在所有 workspace 的 session 状态，并把 waiting / running / idle 会话以可跳转的方形卡片展示出来。

当前发布版本：v0.1.2。仓库已切换到 Tauri 2 作为主应用壳方向，前端入口 [ui/index.html](ui/index.html) 是唯一 UI 源文件。仓库只保留源码和必要的源码内资产，release 产物通过打包脚本按需生成。

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
- Bridge handoff 会通过 ack 文件回报成功或失败，`launch_handoff` 等待确认时不阻塞主 UI。
- 运行态页面移除了 VS Code 背景 mock 和 Windows taskbar mock，只保留透明悬浮工具 UI。

## 技术方向

使用 Tauri 2 实现轻量桌面壳，前端由 [ui/index.html](ui/index.html) 提供。MVP 采用本地文件扫描采集 Copilot / Claude Code session 元数据，配合 VS Code bridge extension 实现精确 session 跳转。

## 功能特性

- **实时监控**：自动扫描 VS Code Copilot 和 Claude Code 的所有 workspace sessions
- **状态分类**：按 waiting（待回复）、running（运行中）、idle（闲置）分类显示
- **一键跳转**：点击卡片直接打开对应的 VS Code session
- **悬浮预览**：悬停卡片右下角展开按钮显示最近用户输入和 AI 正文摘要，预览窗口可 resize 并记忆尺寸
- **Handoff**：右键 session 卡片可携带上下文接续到 Copilot、Copilot CLI 或 Claude，新会话由目标 provider 负责承接
- **VS Code Bridge**：首次启动自动安装 VS Code bridge extension，实现精确 session 跳转、handoff 路由和 ack 成功确认
- **悬浮窗口**：Windows 悬浮应用，四角 resize，可任意方向缩放
- **置顶控制**：设置面板内可实时打开或关闭 always-on-top
- **布局切换**：设置面板内支持 Horizontal / Vertical 两种窗口布局，并记忆偏好
- **智能过滤**：workspace 过滤、状态过滤、full-path workspace grouping、自动隐藏归档 sessions
- **多语言**：中英文切换
- **明暗主题**：支持 Dark / Light 主题切换

## 快速开始

### 开发运行

当前机器已安装 Rust/Cargo，可以运行完整 Tauri 桌面壳：

```powershell
npm install
npm run dev
```

### 打包发布版本

构建 release 版本并打包为可分发的 exe：

```powershell
npm run package:exe
```

打包完成后，可分发的文件位于：

```
artifacts/AgentWatcher/
  AgentWatcher.exe
  vscode-agentwatcher-bridge/
    agentwatcher-bridge-<version>.vsix
    extension.js
    package.json
    README.md
```

用户只需将整个 `artifacts/AgentWatcher/` 文件夹复制到目标机器即可使用。发布目录会包含预打包的 VSIX，正常情况下目标机器不需要安装 Node.js 或 `npx`。

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

## VS Code Bridge Extension

AgentWatcher 使用自带的 VS Code bridge extension 实现精确 session 跳转。

当前 Bridge 扩展 ID：`agentwatcher.agentwatcher-vscode-session-bridge-safe4`。旧的 `safe1` / `safe2` / `safe3` 包名只属于历史测试版本，不再作为当前发布路径使用。

Bridge 同时负责 session 跳转和 handoff 路由。handoff 目标覆盖 Copilot、Copilot CLI 和 Claude；执行完成后，Bridge 会写入 AgentWatcher 临时目录下的 ack 文件，让主应用确认成功或显示失败原因。

### 自动安装行为

- **首次启动**：AgentWatcher 启动后会在后台自动检查 bridge extension 状态
- **自动安装条件**：
  - Bridge extension 未安装
  - 或本地版本比已安装版本更新
- **安装过程**：
  - 优先安装随 `artifacts/AgentWatcher/` 一起发布的 `agentwatcher-bridge-<version>.vsix`
  - 开发目录中没有 VSIX 时，才回退到 `npx @vscode/vsce` 现场打包
  - 自动安装：调用 VS Code CLI `code --install-extension <vsix> --force`
  - 不阻塞 UI：安装在后台线程进行，不影响主界面使用
- **VS Code CLI 查找顺序**：
  1. PATH 环境变量中的 `code` / `code.cmd` / `code.exe`
  2. `%LOCALAPPDATA%\Programs\Microsoft VS Code\bin\code.cmd`
  3. `%LOCALAPPDATA%\Programs\Microsoft VS Code\Code.exe`
  4. `C:\Program Files\Microsoft VS Code\bin\code.cmd`
  5. `C:\Program Files\Microsoft VS Code\Code.exe`
  6. `C:\Program Files (x86)\Microsoft VS Code\bin\code.cmd`
  7. `C:\Program Files (x86)\Microsoft VS Code\Code.exe`

### 手动管理 Bridge

在设置面板（点击右上角 ⚙ 图标）中，可以查看 bridge 状态并手动安装/更新：

- **Checking...**：正在检查 bridge 状态
- **Bridge extension not installed**：未安装，点击 "Install" 按钮安装
- **Update available**：有新版本，点击 "Update" 按钮更新
- **Bridge installed and up to date**：已安装且是最新版本

### Bridge 版本管理

- **本地版本**：读取 `vscode-agentwatcher-bridge/package.json` 中的 `version` 字段
- **已安装版本**：从 VS Code extensions 目录读取：
  - `%USERPROFILE%\.vscode\extensions\agentwatcher.agentwatcher-vscode-session-bridge-safe4-*\package.json`
  - `%USERPROFILE%\.vscode-insiders\extensions\agentwatcher.agentwatcher-vscode-session-bridge-safe4-*\package.json`

### 手动安装或重装

发布包用户优先使用随包 VSIX：

```powershell
code --install-extension .\vscode-agentwatcher-bridge\agentwatcher-bridge-*.vsix --force
```

在源码目录中验证时使用：

```powershell
code --install-extension .\artifacts\AgentWatcher\vscode-agentwatcher-bridge\agentwatcher-bridge-*.vsix --force
```

需要强制重装时，先卸载当前 Bridge，再重新安装随包 VSIX：

```powershell
code --uninstall-extension agentwatcher.agentwatcher-vscode-session-bridge-safe4
code --install-extension .\vscode-agentwatcher-bridge\agentwatcher-bridge-*.vsix --force
```

如果机器上曾安装过历史测试扩展，VS Code 扩展面板里可能还能看到 `safe1` / `safe2` / `safe3`。这些旧扩展不影响 `safe4` 的当前 URI 路由，但建议在发布验证机上卸载，避免排障时混淆。

### 故障排查

**Bridge 安装失败**：

1. **检查 VS Code CLI**：
   ```powershell
   code --version
   ```
   如果提示找不到命令，需要将 VS Code bin 目录添加到 PATH，或从 VS Code 内部运行 "Shell Command: Install 'code' command in PATH"。

2. **检查随包 VSIX**：
  确认 `artifacts/AgentWatcher/vscode-agentwatcher-bridge/` 下存在 `agentwatcher-bridge-<version>.vsix`。如果使用的是源码目录而不是发布目录，缺少 VSIX 时才需要 Node.js 和 `npx`。

3. **检查 Node.js 和 npx（仅源码回退打包需要）**：
   ```powershell
   npx --version
   ```
  源码目录现场打包 Bridge 需要 `npx` 和 `@vscode/vsce`。

4. **手动安装 Bridge**：
   ```powershell
  code --install-extension artifacts/AgentWatcher/vscode-agentwatcher-bridge/agentwatcher-bridge-*.vsix --force
   ```

5. **确认当前 Bridge ID**：
  ```powershell
  code --list-extensions | findstr agentwatcher
  ```
  预期至少看到 `agentwatcher.agentwatcher-vscode-session-bridge-safe4`。

**Session 跳转失败**：

- 确保 Bridge extension 已启用：在 VS Code 中搜索 "AgentWatcher Bridge"
- 重启 VS Code：有时需要重启 VS Code 使 extension 生效
- 检查 session 路径：确保 session JSONL 文件存在且可访问

## Known Issues

- Prompt handoff 的提示词插入目前整体仍依赖 clipboard bridge：Copilot、Copilot CLI 和 Claude 都由 Bridge 读取 clipboard 后，再通过各自的目标命令填入。非剪贴板 prompt 通道列入 Future，不作为本次 v0.1.2 发布阻断。
- 未来如果引入 prompt 临时文件，禁止写入项目目录，只允许写入 AgentWatcher 自身运行时临时目录或系统临时目录，例如 `%TEMP%\AgentWatcher\...`，并需要 token、TTL 和读取后清理。
- 历史 `safe1` / `safe2` / `safe3` Bridge 扩展可能残留在开发机上；当前发布只以 `safe4` 为准。
- docs 是否进入 git 仍由发布前人工决定，当前文档仅记录准备状态，不默认改变仓库策略。

## Post-v0.1.x Roadmap

- 设计并验证非剪贴板 prompt handoff 通道，优先评估 VS Code command arg、AgentWatcher runtime temp file + token、localhost bridge、named pipe。
- 增加窗口位置记忆、系统托盘入口和全局快捷键。
- 补充状态变化去抖、workspace 静音、quiet mode 和系统 toast。
- 引入本地缓存，减少长时间运行时重复解析 session 文件的成本。
- 扩展干净机器发布验证矩阵，覆盖 VS Code Stable/Insiders、100%/125%/150% DPI 和长时间运行。

## 发布验证清单

- [ ] 从发布目录启动 `AgentWatcher.exe`，确认不依赖源码目录。
- [ ] 未安装 Bridge 时自动安装成功；旧版本 Bridge 时可更新；失败时 UI 有可操作提示。
- [ ] 手动 `code --install-extension .\vscode-agentwatcher-bridge\agentwatcher-bridge-*.vsix --force` 可重装 Bridge。
- [ ] 点击 Copilot / Claude session 卡片可打开目标 workspace/session；失败时显示错误。
- [ ] 右键 session 卡片可 handoff 到 Copilot、Copilot CLI 和 Claude，成功后 UI 收到 Bridge ack。
- [ ] 中英切换、dark/light 切换能同步到主窗口、Session Preview 和 Handoff Panel。
- [ ] Session Preview 内容只通过运行时事件传递，不写入 localStorage 或项目目录临时文件。
- [ ] 发布包包含 `AgentWatcher.exe`、`vscode-agentwatcher-bridge/`、`agentwatcher-bridge-<version>.vsix`。

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

## 架构说明

### Rust Backend (src-tauri/src/lib.rs)

- `scan_sessions`：扫描并返回所有符合条件的 sessions
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
- 管理 Handoff Panel、跨 provider handoff 目标和 Bridge ack 状态提示

### VS Code Bridge Extension (vscode-agentwatcher-bridge/)

- 监听 `vscode://agentwatcher.agentwatcher-vscode-session-bridge-safe4/open` URI
- 解析 session resource 并打开对应的 chat session
- 支持 Copilot、Copilot CLI 和 Claude Code handoff 目标
- 通过 AgentWatcher 临时目录 ack 文件回传 handoff 成功或失败

## 许可证

UNLICENSED（私有项目）

## Tauri 开发

此部分说明已整合到"快速开始"章节。
