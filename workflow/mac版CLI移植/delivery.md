---
schema_version: "1"
protocol: "1.3.0"
artifact: "delivery"
created_at: "2026-11-13T12:30:00Z"
producer: "zcode"
outcome: "delivered"
landing_branch: "dev"
---

# 交付：agentwatcher-cli 移植 macOS（第一步）

## 任务边界

为 AgentWatcher 开发 mac 版的第一步：整套 `agentwatcher-cli` 在 macOS 上可用。GUI（Tauri 悬浮窗）保持 Windows 专属，不在本次范围。开发途径按用户指示复用既有 SSH 通道（`127.0.0.1:2222` → Mac `pengbo`，密钥 `F:/AiPhotographer/.local/jsrport/id_ed25519`），代码同步走 SSH 直传（`git archive | ssh tar -x`），WIP 不进 GitHub。

## 合入了什么（dev 分支 6 个提交）

- `4534c50` build(cross-platform)：平台依赖拆分（winreg/windows-sys/junction → `cfg(windows)`，新增 unix libc）+ `gui` feature 门控（默认开启，Windows 发布链路不变）+ build.rs 条件执行 tauri_build。
- `e3d0f53` feat(paths)：`user_home` / `app_data_root` / `path_list_separator` / `workspace_path_display` 四个平台助手；约 35 处数据源路径调用点切换；`decode_file_uri` Unix 直通；workspace 显示口径按平台。
- `a71168e` feat(unix)：flock 缓存锁、进程组 KillOnCloseJob（`process_group(0)` + `kill(-pgid)`）、lsof/pgrep 端口探测、codex/opencode/node/code CLI 发现补 macOS 候选。
- `fix(unix)` + `test(...)` + `fix(opencode)`：Unix 分支编译修（Debug impl）、单测去 Windows 假设（6 个）、OpenCode session 表 schema 容忍层与扫描失败分类日志。

## 验证记录

- **Windows**：`cargo test` lib 177 全绿；`--test cli_taxonomy --test cli_contract` 19 + 13 全绿；默认（gui）编译零警告；`npm run test:tauri:smoke` 通过（附着 dev 的 CDP，真实点击：语言/性能窗口/接续面板/AgentTask）。
- **macOS（arm64, macOS 27.2.1, cargo 1.93.1）**：`cargo build --no-default-features --bin agentwatcher-cli` 成功；`cargo test --no-default-features` 全绿（lib 171 + CLI 内嵌 8 + cli_contract 13 + cli_taxonomy 19；与 Windows 差的 6 个为 Windows 专属测试按平台/feature 门控，非丢失覆盖）。
- **Mac 真实数据源冒烟**：codex（含 live running 会话）/ opencode / workbuddy 的 `session list`（human+json）、`session show`（正文预览）、`session usage`（token + topTools）、`handoff export`（SQLite → NDJSON 导出）全部通过；copilot 在该 Mac 无数据（空结果正确）；zcode 未安装（空结果正确）。
- **flock 并发合并实证**：两个全历史扫描进程，第二个晚 0.4s 启动、与第一个同瞬间结束（等待锁后读缓存，未各自重扫）；缓存目录产出 `.json` + `.lock`。

## 过程中发现并处理的用户环境问题

- Mac 的 opencode 是 1.2.25（2026-03 旧版），`session` 表没有 `path`/`cost`/`tokens_*` 列导致扫描全空。按用户指示经官方安装脚本更新到 **1.19.5**（与 Windows 同版），并短暂拉起 `opencode serve` 15 秒触发 db 迁移完成更新。代码侧同时落了 schema 容忍层——任何未迁移的老库都能扫出基本会话，不依赖用户环境恰好是新版。

## 已知边界 / Future

- **KillOnCloseJob 的 Unix 差异**：宿主自身被 SIGKILL 时没人触发 Drop，进程组会留下（Windows Job Object 由内核兜底）。正常退出与显式终止路径等价。
- Mac 上 `~/.local/bin/opencode` 是一个无执行权限的 14 字节坏占位文件（内容 "File Not Found"），会挡 PATH 命中真二进制 `~/.opencode/bin/opencode`。属用户环境，未动；建议用户删除或修复。
- ZCode 发送通道在 macOS 返回明确错误（依赖注册表反解，Unix 无桩）；WorkBuddy 建议生成走 codex exec，理论可用但未在 Mac 冒烟真实调用（会产生模型用量）。
- suggest generate / imagegen 在 Mac 未跑真实生成（会产生模型用量），仅验证 codex CLI 发现任路可达。
- Tauri GUI macOS 版、VSIX bridge、.app/.dmg 打包：均不在本次范围，待后续任务。

## 交付物索引

- Mac 构建验证副本：`/Users/pengbo/AiProject/AgentWatcher-mac`（每次 `git archive` 幂等覆盖，非 git 仓库）
- 本文档：`workflow/mac版CLI移植/delivery.md`
- CHANGELOG「未发布」段、README「平台支持」段、AGENTS.md（构建命令 / 数据源 / CLI 章节）已同步口径
