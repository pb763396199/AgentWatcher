# AgentWatcher

AgentWatcher 是一个 Windows 桌面悬浮小工具，目标是像输入法候选窗一样常驻在屏幕任意位置，监控 VS Code 中 Copilot 和 Claude Code 在所有 workspace 的 session 状态，并把 waiting / running / idle 会话以可跳转的方形卡片展示出来。

当前仓库状态：已完成本地 HTML 交互设计稿，并已切换到 Tauri 2 作为主应用壳方向。仓库只保留源码和必要的源码内资产，release 产物通过打包脚本按需生成。

## 当前设计稿

- 主设计稿：[agentwatcher-windows-overflow-prototype.html](agentwatcher-windows-overflow-prototype.html)
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
- 运行态页面移除了 VS Code 背景 mock 和 Windows taskbar mock，只保留透明悬浮工具 UI。

## 技术方向

使用 Tauri 2 实现轻量桌面壳，并直接复用 HTML/CSS/JS 设计稿。MVP 采用本地文件扫描采集 Copilot / Claude Code session 元数据，配合 VS Code bridge extension 实现精确 session 跳转。

## 功能特性

- **实时监控**：自动扫描 VS Code Copilot 和 Claude Code 的所有 workspace sessions
- **状态分类**：按 waiting（待回复）、running（运行中）、idle（闲置）分类显示
- **一键跳转**：点击卡片直接打开对应的 VS Code session
- **VS Code Bridge**：首次启动自动安装 VS Code bridge extension，实现精确 session 跳转
- **悬浮窗口**：Windows 悬浮应用，四角 resize，可任意方向缩放
- **智能过滤**：workspace 过滤、状态过滤、自动隐藏归档 sessions
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

### 验证前端设计稿

只验证前端设计稿时可以运行：

```powershell
npm run dev:ui
```

然后访问 `http://127.0.0.1:1420`。

## VS Code Bridge Extension

AgentWatcher 使用自带的 VS Code bridge extension 实现精确 session 跳转。

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
  - `%USERPROFILE%\.vscode\extensions\agentwatcher.agentwatcher-bridge-*\package.json`
  - `%USERPROFILE%\.vscode-insiders\extensions\agentwatcher.agentwatcher-bridge-*\package.json`

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

**Session 跳转失败**：

- 确保 Bridge extension 已启用：在 VS Code 中搜索 "AgentWatcher Bridge"
- 重启 VS Code：有时需要重启 VS Code 使 extension 生效
- 检查 session 路径：确保 session JSONL 文件存在且可访问

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

### 扫描配置

在设置面板可配置：

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

### VS Code Bridge Extension (vscode-agentwatcher-bridge/)

- 监听 `vscode://agentwatcher.agentwatcher-bridge/open` URI
- 解析 session resource 并打开对应的 chat session
- 支持 Copilot 和 Claude Code 两种 provider

## 许可证

UNLICENSED（私有项目）

## Tauri 开发

此部分说明已整合到"快速开始"章节。
