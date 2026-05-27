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
- [ ] 实现系统托盘图标和显示 / 隐藏入口。
- [ ] 实现全局快捷键显隐窗口。
- [ ] 实现启动时恢复上次窗口大小、位置、主题、语言。

## Phase 2 - UI 组件落地

- [x] 实现 waiting / running / idle 状态筛选。
- [x] 实现 workspace 下拉过滤。
- [x] 实现每排列数控制，支持 1 到 6 列。
- [x] 实现卡片按窗口宽度自适应填满。
- [x] 实现 compact card / micro card 信息密度切换。
- [x] 实现滚动队列和底部分布条。
- [x] 实现中英双语切换。
- [x] 实现 dark / light 主题切换。
- [ ] 实现卡片 hover tooltip。
- [ ] 实现右键菜单：复制 session id、打开 workspace、静音、隐藏 workspace。

## Phase 3 - 本机数据扫描 PoC

- [ ] 扫描 VS Code `workspaceStorage`。
- [ ] 解析 `workspace.json`，建立 workspace hash 到路径的映射。
- [ ] 探测 Copilot Chat `chatSessions` 文件。
- [ ] 只读取 metadata、mtime、文件大小和必要头尾片段，不读取完整正文。
- [ ] 扫描 Claude Code projects / sessions index。
- [ ] 输出 console table：provider、workspace、session id、mtime、state。
- [ ] 验证多 workspace、多 VS Code 实例场景。
- [ ] 建立解析失败时的 unknown 状态。

## Phase 4 - 状态机

- [ ] 定义 unified session model。
- [ ] 定义 waiting / running / idle / unknown 状态。
- [ ] 基于 mtime 和文件增长推断 running。
- [ ] 基于停止增长和最后事件推断 waiting。
- [ ] 基于 idle threshold 推断 idle。
- [ ] 为 Claude Code 预留 hook 事件输入。
- [ ] 为 Copilot Chat parser 添加格式版本保护。
- [ ] 添加状态变化去抖，避免 UI 闪烁。

## Phase 5 - 本地缓存与设置

- [ ] 引入 SQLite 本地缓存。
- [ ] 缓存 workspace alias、隐藏状态、静音状态。
- [ ] 缓存 session 最新状态和最后活动时间。
- [ ] 设置扫描频率。
- [ ] 设置 idle threshold。
- [ ] 设置是否显示系统通知。
- [ ] 设置隐私模式和路径隐藏。

## Phase 6 - 跳转能力

- [ ] MVP 使用 `code --reuse-window <workspace>` 打开 workspace。
- [ ] 枚举 VS Code 进程和窗口标题，尽量聚焦已打开窗口。
- [ ] 点击卡片触发 workspace 级跳转。
- [ ] 跳转失败时显示可操作错误提示。
- [ ] 研究 VS Code bridge extension 可行性。
- [ ] 后续实现 localhost bridge，尝试聚焦具体 chat/session。

## Phase 7 - 通知策略

- [ ] 只在状态进入 waiting 时提醒。
- [ ] 支持 quiet mode。
- [ ] 支持 workspace 静音。
- [ ] 支持系统 toast。
- [ ] 支持 rail badge 数量变化。
- [ ] 避免 running / idle 大量刷屏。

## Phase 8 - 验证与质量

- [ ] 为 parser 添加单元测试。
- [ ] 为状态机添加单元测试。
- [ ] 为 workspace resolver 添加样本测试。
- [ ] 做 8 小时运行内存观察。
- [ ] 验证 100%、125%、150% DPI。
- [ ] 验证窗口贴边、resize、恢复位置。
- [ ] 验证无真实 session 时的空状态。
- [ ] 验证大量 session 时的滚动性能。

## Phase 9 - 打包发布

- [ ] 准备 Windows 应用图标。
- [ ] 准备 self-contained publish。
- [ ] 准备安装包或单文件发行。
- [ ] 添加版本号和 changelog。
- [ ] 添加首次启动引导和隐私说明。

## 当前优先级

1. 启动真实 session 扫描 PoC。
2. 建立 JS 与 Rust backend 的 command/event 通信。
3. 实现窗口位置、大小、主题、语言记忆。

## Phase 1b - Tauri 2 主应用壳

- [x] 创建 Tauri 2 项目骨架。
- [x] 复制 HTML 设计稿为 Tauri 前端入口。
- [x] 安装 Rust/Cargo 工具链。
- [x] 运行 `npm install`。
- [x] 验证 `npm run dev:ui`。
- [x] 验证 `npm run build`。
- [x] 将 HTML 中的桌面背景 mock 拆分为真实窗口 UI。
- [x] 用 Tauri window API 接管四角 resize、拖动、最小化、最大化、关闭。
- [ ] 建立 JS 与 Rust backend 的 command/event 通信。

## 已放弃路线

- WPF 纯控件复刻 HTML UI：视觉对齐成本过高，已删除未提交的 WPF 临时项目。