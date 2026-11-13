---
schema_version: "1"
protocol: "1.3.0"
artifact: "delivery"
created_at: "2026-11-13T16:05:00Z"
producer: "zcode"
outcome: "delivered"
landing_branch: "dev"
---

# 交付：AgentWatcher GUI 移植 macOS + Mac 上的 ChatGPT 装入 AgentWatcher 技能

## 任务边界

上一任务完成 CLI 的 macOS 移植后，本次：① GUI（Tauri 壳）在 macOS 跑起来并打出未签名 `.app`（用户拍板：dev 跑通 + .app；dmg 由 tauri build 顺手产出但签名/公证不做；性能面板 v1 空列表降级）；② 给 Mac 上的 ChatGPT 桌面版装 AgentWatcher 技能。通道复用 SSH 2222 → Mac `pengbo`，代码 `git archive | ssh tar -x` 直传，WIP 不进 GitHub。

## 合入了什么（dev 分支 4 个提交）

- `a48bb27` feat(gui-mac)：运行时等价——`open_system_url`（原 rundll32，mac 走 `open`，覆盖 vscode://、workbuddy://、codex thread、opencode 深链）；`spawn_visible_terminal` mac 走 osascript Terminal do script；`copy_file_to_clipboard` mac 走 osascript `POSIX file`（Finder 可 Cmd+V）。
- `e0ecf1d` fix(build)：tauri.conf.json 补 `app.macOSPrivateApi: true` 对齐 `macos-private-api` feature（透明无边框窗口在 macOS 的前置）；windows-sys 显式声明 `Win32_Security`（CreateJobObjectW 签名依赖，此前靠传递 feature union 兜着）；删除无引用的 junction 依赖。
- `4db97a8` feat(icons)：generate-icons 同一 GDI+ 路径追加输出 1024 PNG 源图（ps1 内注释用英文避免 PowerShell 5.1 ANSI 读无 BOM 中文乱码的坑）；`npx tauri icon` 生成 icon.icns；bundle.icon 补全 ico/png/icns。Windows 手绘 15 尺寸 ico 字节不变。
- `296e9d0` chore(cfg)：Windows 专属性能采样（PERFORMANCE_PROCESS_SAMPLES / ProcessCpuSample / performance_agent_process_role / bytes_to_mb）与 netstat 解析器挂 `cfg(windows)`，Unix 补 lsof 解析对等测试（解析器拆出 `_with_ids` 纯函数便于注入测试）。

## 验证记录

- **macOS（M1 Pro, 27.2.1）**：
  - `cargo check --features gui` 零错误零警告；`cargo test --no-default-features --lib` 171 全绿（与 Windows 177 差值 = Windows 专属测试平台门控）。
  - `npm run dev` 起真窗口：进程存活 + **CGWindowList 枚举确认窗口在屏**（owner=agentwatcher，396×580 与配置一致）。
  - `npx tauri build` 产出未签名 `AgentWatcher.app` + `AgentWatcher_0.1.11_aarch64.dmg`；从 `.app` 启动复验：release 进程存活 + 窗口在屏。
  - 限制如实记录：macOS 无 WebView2 CDP（smoke 工具链不适用）；SSH 会话的 `screencapture` 受 TCC 屏幕录制权限限制，像素级效果留人工看屏验收。
- **Windows**：lib 177 + cli_taxonomy 19 + cli_contract 13 全绿；`cargo build`、`cargo check --no-default-features` 通过；`npm run test:tauri:smoke` 通过（语言切换/性能窗口/接续面板/AgentTask 真实点击）。中途一次 rustc STATUS_STACK_BUFFER_OVERRUN 与 bash fork 失败为宿主机资源波动，重跑即绿，非代码问题。

## ChatGPT 技能安装

- **形态确认**：ChatGPT.app 26.1106.51428 内嵌 `Codex Framework.framework`（共享 `codex://` scheme）；app.asar 字符串分析确认其读取 `~/.codex/skills`（含 CODEX_HOME 展开逻辑）与 `~/.agents/skills`（技能发现正则含 `.system` 子目录，与 imagegen skill 同源）。
- **安装结果**：Mac 上 `agentwatcher-cli skill install`（headless 构建）装入三处宿主：`~/.codex/skills/agentwatcher/SKILL.md`（ChatGPT 桌面版 + Codex CLI 共享）、`~/.agents/skills/agentwatcher/SKILL.md`（ChatGPT 引用的通用目录，目录为本任务创建）、`~/.workbuddy/skills/agentwatcher/SKILL.md`。`skill status` 确认 docVersion v3 / 11529 bytes 三处 installed；zcode/claude/copilot/opencode 宿主目录在该 Mac 不存在，按既有语义跳过。

## 已知边界 / Future

- .app/.dmg 未签名未公证：侧载首次打开需右键绕 Gatekeeper；后续可做 ad-hoc 签名。
- 性能诊断面板 macOS 空列表（进程采样待 `ps` 或 sysinfo 实现）。
- WKWebView 与 WebView2 的行为差异（Ctrl+滚轮缩放等）未逐一排查，遇到再修。
- ZCode 发送通道在 macOS 返回明确错误（依赖注册表）；WorkBuddy/VSIX bridge 在 Mac 未验证。
- Mac 上 `~/.local/bin/opencode` 是无执行权限的坏占位文件（内容 "File Not Found"，会挡 PATH 命中 `~/.opencode/bin/opencode`），属用户环境，建议删除。

## 交付物索引

- Mac 构建副本：`/Users/pengbo/AiProject/AgentWatcher-mac`；`.app`：`src-tauri/target/release/bundle/macos/AgentWatcher.app`；dmg：`bundle/dmg/AgentWatcher_0.1.11_aarch64.dmg`
- 技能落点：Mac `~/.codex` `~/.agents` `~/.workbuddy` 三处 `skills/agentwatcher/SKILL.md`
- 本文档：`workflow/mac版GUI移植与ChatGPT技能/delivery.md`；CHANGELOG「未发布」、README「平台支持」、AGENTS.md「怎么跑起来」已同步口径
