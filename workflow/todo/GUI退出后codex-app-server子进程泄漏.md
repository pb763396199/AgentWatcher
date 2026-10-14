---
schema_version: 1
protocol: 1.3.0
record: todo
id: td_01M4WCQ2XT000G1XN0V7Y4W9RT
status: done
created_at: 2026-10-14
importance: normal
closed_at: 2026-10-14
outcome: 已由任务 7176h9hq 修复：codex app-server 与 opencode serve 都挂入 kill-on-close 的 Job Object，温和关闭与强杀两条退出路径实测无孤儿；回归测试 job_handle_drop_terminates_whole_descendant_tree 常驻
---

# GUI 退出后 codex app-server 子进程泄漏

2026-10-14 清理任务 worktree 残留时发现：两台测试用 AgentWatcher GUI 实例（worktree 里的 dev 壳）退出后，它们拉起的 codex app-server
子进程没有随之结束，以孤儿进程存活数小时：

- `node codex.js app-server --listen ws://127.0.0.1:9137` + `codex.exe app-server`，工作目录停在 `<worktree>\src-tauri`
- 后果一：占住 worktree 目录句柄，任务收尾删不掉工作区（本次用 PEB 读进程 CWD 才定位到）
- 后果二：每个泄漏实例占一个监听端口和常驻内存，多次启停会累积

对照：CLI 侧已有同类防御——`set_codex_app_server_disabled(true)` 加 stdio 句柄继承清理（AGENTS.md「AI 操作 CLI」一节，
cmd.exe→node→codex 链继承管道写端是踩过的真实坑）。GUI 侧 `scan_codex_sessions` 走 app-server 的路径没有对应的退出清理。

修法方向（做的时候再验证）：GUI 退出时结束自己拉起的 app-server 子进程（Job Object 或显式 kill）；
或者 app-server 子进程的工作目录与生命周期与扫描会话绑定。定位手段：用 PEB 读各进程 CurrentDirectory
（`F:\AiProject\AgentWatcher\.tmp\find-lockers.ps1` 有现成脚本）。
