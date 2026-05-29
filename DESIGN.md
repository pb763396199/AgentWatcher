# AgentWatcher 设计稿

## 设计目标

AgentWatcher 是一个 Windows 桌面悬浮工具，用来集中观察 VS Code 中 Copilot 和 Claude Code 的所有 workspace session。它要长期挂在屏幕边缘，像输入法候选窗一样轻、不打断，但在某个 session 等待用户回复时能立刻被看见。

## 当前 UI 文件

Tauri 2 前端入口为 [ui/index.html](ui/index.html)，它是当前唯一 UI 源文件，可通过 Vite 在浏览器中预览。页面已移除 VS Code 背景 mock 和 Windows taskbar mock，只保留 AgentWatcher 主面板与 AW rail。当前运行态使用透明窗口背景，让主面板和 rail 像悬浮工具一样贴在桌面上。

## 窗口形态

- 主窗口是 Windows 小工具样式，默认悬浮在 VS Code 右侧。
- 标题栏包含应用图标、标题、中英切换、明暗主题切换、最小化、最大化、关闭。
- 四个角都可以 resize，不只支持右下角。
- 左侧 AW rail 是折叠态预览，保持贴在主窗口左侧，窗口移动或缩放时同步位置。
- 设置面板提供 always-on-top 开关，运行时可实时切换置顶状态。

## 会话展示

- 状态顺序固定为 waiting、running、idle。
- Waiting 始终最高优先级，红色标识。
- Running 使用蓝色标识，显示 live / running 语义。
- Idle 使用绿色标识，默认低优先级。
- 当 session 数量超过可视区域时，中间区域滚动，底部显示 scroll queue 数量和三色分布条。

## Session Preview

- 卡片右下角展开按钮是预览触发区，卡片主体点击仍用于跳转 VS Code session。
- 预览向右下展开，跟随触发按钮定位。
- 浏览器预览模式使用 fixed overlay；Tauri 运行态使用无标题栏、无任务栏的 `session-preview` tooltip webview，避免被主窗口边界裁剪。
- 预览窗口支持独立滚动、四角 resize 和尺寸持久化。
- preview webview 采用 hidden + ready/show 流程，避免首次弹出时闪出完整 Watcher 页面。
- preview webview 会复用并 hide/show，避免 hover 时频繁创建和销毁窗口。

## 卡片布局

- 卡片不是固定像素大小，而是按“每排几个”控制。
- 用户可以选择 1 到 6 个 icon per row。
- 卡片宽度由窗口宽度和列数自动计算，优先填满卡片区域。
- 3 列及以上启用 compact card，隐藏底部文字，避免内容重叠。
- 5 列及以上进入 micro card，进一步减少信息密度。

## 过滤与控制

- 状态筛选：All / Wait / Run / Idle。
- Workspace 下拉筛选：选中后只展示对应 workspace 的 session。
- Per row 控制：滑杆连续改变每排数量，按钮在预设数量间循环。
- 中英切换：标题、筛选、状态、队列、底部信息即时切换。
- 明暗主题：Dark / Light 即时切换。

## 隐私边界

卡片默认只展示 provider、workspace alias、状态、时间和极短任务标题，不直接暴露 prompt 正文、模型输出正文或代码片段。Session Preview 属于用户主动悬停展开后的详情层，只展示最近用户输入和 AI 正文摘要，并过滤工具、terminal、thinking、模型错误等噪声。预览正文只在当前运行时内存和 Tauri event 中传递，不写入 localStorage。

## 最大技术风险

点击卡片直接聚焦 VS Code 内某个具体 Copilot / Claude session 还没有公开稳定 deep link。MVP 先做到 workspace 级跳转，精确 session 聚焦放到 VS Code bridge extension 阶段验证。

## 落地技术路线

当前主路线为 Tauri 2 + HTML/CSS/JS。选择它是为了让设计稿直接成为真实 UI，同时比 Electron 更轻。当前已完成 Tauri 2 壳、真实窗口页面、Tauri window API 拖动/resize/最小化/最大化/关闭接入、VS Code bridge、Session Preview tooltip webview 和 release 打包脚本验证。