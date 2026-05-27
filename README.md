# AgentWatcher

AgentWatcher 是一个 Windows 桌面悬浮小工具，目标是像输入法候选窗一样常驻在屏幕任意位置，监控 VS Code 中 Copilot 和 Claude Code 在所有 workspace 的 session 状态，并把 waiting / running / idle 会话以可跳转的方形卡片展示出来。

当前仓库状态：已完成本地 HTML 交互设计稿，下一步进入真实 Windows 应用壳和本机 session 扫描 PoC。

## 当前设计稿

- 主设计稿：[agentwatcher-windows-overflow-prototype.html](agentwatcher-windows-overflow-prototype.html)
- 早期草稿：[agentwatcher-prototype.html](agentwatcher-prototype.html)
- 设计说明：[DESIGN.md](DESIGN.md)
- 开发清单：[TODO.md](TODO.md)

## 已验证的界面能力

- Windows 悬浮应用窗口外观，贴近 VS Code 视觉语言。
- 四角 resize，窗口可任意方向缩放。
- 左侧 AW 折叠面板贴着主窗口左侧同步移动。
- session 卡片按“每排数量”自适应填满卡片区域。
- 超量 session 使用优先级滚动队列展示。
- workspace 过滤、状态过滤、中英切换、明暗主题切换。

## 技术方向

优先使用 .NET 8 + WPF 实现 Windows 原生桌面壳；MVP 先用本地文件扫描采集 Copilot / Claude Code session 元数据，后续再考虑 VS Code bridge extension 做精确 session 聚焦。