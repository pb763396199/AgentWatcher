---
schema_version: 1
protocol: 1.3.0
record: todo
id: td_01M4W42Z1X000DYX8T06W7358D
status: open
created_at: 2026-10-14
importance: normal
---

# 筛选流程的 todo 提示步骤在双实例环境下易失败

`npm run test:tauri:flow -- 筛选` 末尾的 `verifyTodoWorkspaceScope` 会把一条 ready todo 写进
`%APPDATA%\AgentWatcher\todos.v1.json`，重载页面后期望主窗口出现「可推进待办」提示。2026-10-14
三次运行全部失败在「临时 todo fixture 写入后仍未显示 ready nudge」：新代码两轮、dev 分支旧代码
对照一轮，错误一致，与本任务改动无关。

怀疑机制：测试写入 fixture 后，同一台机器上常驻的另一个 AgentWatcher 实例（例如用户桌面开着的
dev 壳）会用自己的内存态覆写同一份 todos.v1.json，重载后 fixture 任务已不在文件里，提示自然不出现。
另一个可能：fixture 选中的目标工作区在重载前后出现了 waiting/running 会话，提示被活跃会话抑制。

修法方向（做的时候再验证假设）：fixture 写入后回读文件确认没被覆写；或测试改用独立的 todo 状态文件
路径；或对「活跃会话抑制提示」的场景直接跳过该断言并说明原因。

证据：worktree 的 `.tmp/tauri-realtest/20261014-102441/result.json` 与主检出同日运行记录。
