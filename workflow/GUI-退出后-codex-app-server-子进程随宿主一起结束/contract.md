---
schema_version: "1"
protocol: "1.3.0"
id: "wi_5SYZXNKVTFYTV99EVBBBMJK0S6"
short_id: "7176h9hq"
title: "GUI 退出后 codex app-server 子进程随宿主一起结束"
status: "done"
kind: "fix"
created_at: "2026-10-14T07:37:55.018970700+00:00"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
branch_or_pr: "feature/GUI-退出后-codex-app-server-子进程随宿主一起结束"
revision: "2"
updated: "2026-10-14"
---
# GUI 退出后 codex app-server 子进程随宿主一起结束

## 原始请求

用户原话（2026-10-14）："开新worktree修"。待办正本：`workflow/todo/GUI退出后codex-app-server子进程泄漏.md`，证据来自同日 worktree 残留清理：两台测试 GUI 壳退出后各留下一组 codex app-server 孤儿进程（node codex.js 与 codex.exe，工作目录钉在 worktree 的 src-tauri）。

## 目标

AgentWatcher GUI 无论以哪种方式退出（正常关窗、任务管理器强杀、崩溃），都不留下 codex app-server 进程树的孤儿。

## Why

`ensure_codex_app_server` 拉起的是多层进程树（cmd/node 包装层加 codex.exe）；现有清理 `server.child.kill()` 只终止直接子进程，孙进程全部存活；强杀宿主时连这一步也没有。CLI 侧已有同类防御（`set_codex_app_server_disabled`），GUI 侧缺失。

## 范围

做：

- 查清 spawn 链与两种退出路径的泄漏机制，落施工回执。
- 拉起 app-server 时将其纳入 kill-on-close 的 Windows Job Object，宿主无论怎么死，整棵子树随宿主句柄关闭被系统终止。
- 加一个会失败的回归测试（杀掉直接子进程后孙进程必须也消失）。
- 真实壳验证：正常关窗与 taskkill 强杀两种退出后，查不到残留 app-server 进程。

不做：

- 不改 CLI 的 `set_codex_app_server_disabled` 语义。
- 不改扫描期间对 app-server 的使用方式（thread/list、usage 聚合口径不变）。

## 强约束

- 扫描行为不回退：Codex 会话卡片、resume 去重、usage 聚合与现状一致。
- 不引入常驻后台线程轮询。

## 自主边界

| 不用问直接定 | 必须停下来问 |
| --- | --- |
| Job Object 方案与 CREATE_SUSPENDED 挂接方式 | 无。修法方向待办里已列，用户点名开修 |
| 回归测试用 cmd/powershell 模拟多层子进程树 | |
| worktree 端口隔离方案（沿用前两个任务的做法） | |

## 验收条件

- AC-001: 根因落档：spawn 链层级、正常退出与强杀两条路径各自的泄漏机制有代码行号与进程证据。Verify: manual:核对施工回执的调试记录节
- AC-002: 真实壳两种退出方式（关窗、taskkill /F）后，按进程 CWD 与监听端口查不到存活的 codex app-server 进程树。Verify: manual:worktree 起真实壳触发 Codex 扫描，分别用关窗和 taskkill 退出，用 PEB 读 CWD 脚本加端口扫描核对
- AC-003: cargo test 全过，含新增回归测试；冒烟流程不回退。Verify: manual:跑 cargo test 与 npm run test:tauri:smoke

## 访谈记录

| 时间 | 谁说的 | 内容 | 结论 |
| --- | --- | --- | --- |
| 2026-10-14 | 用户 | "开新worktree修" | 按待办正本范围执行 |

## 设计取舍

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| Windows Job Object kill-on-close | CREATE_SUSPENDED 起进程、挂作业、恢复主线程；正常退出走 TerminateJobObject，强杀由系统关句柄兜底 | 新增约一百行 Windows 代码与一个回归测试；覆盖强杀与崩溃 | 采用 |
| 退出钩子里 taskkill /T | 复用系统命令杀整树 | 只覆盖正常退出；强杀与崩溃仍泄漏 | 不采用（作为对照保留在记录里） |
| 只杀直接子进程（现状） | 维持 Child::kill | 孙进程（node、codex.exe）全部存活，两台测试壳实证泄漏 | 不采用 |
| 学 CLI 禁用 app-server | GUI 也走本地文件扫描 | 改变 GUI 扫描口径（thread/list 去重、usage 聚合），违反强约束 | 不采用 |

实施中并入的同源缺陷：opencode serve（`start_owned_opencode_web_server`）与 codex app-server 同模式多层进程树，且连正常退出的清理都没有；共用同一个作业辅助一并修复，退出钩子同时调用两个 shutdown。

## revision 历史

| 版本 | 日期 | 为什么改 |
| --- | --- | --- |
| 1 | 2026-10-14 | 首版，取舍待根因核实后定案 |
| 2 | 2026-10-14 | 取舍定案：Job Object kill-on-close；实施中发现 opencode serve 同源缺陷并入修复 |
