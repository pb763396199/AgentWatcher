---
schema_version: 1
protocol: 1.3.0
artifact: validation
artifact_id: ar_01M4W5DCD1000BT77NQTBSE9GH
work_item_id: wi_19X7WBYXTZK9DH6X1KT55XHK7N
created_at: 2026-10-14T11:05:00Z
producer: aes-validate
outcome: passed
supersedes: [ar_01M4W42Z010005EAGXTMHSX5HC]
contract_revision: 2
verified_at_commit: 5387854c43d6ef7fa6d99aed8f41f9c1bdf1cf7b
executed_at: "2026-10-14T10:36:00Z"
environment: "Windows 11 10.0.26200；worktree 隔离真实 Tauri 壳（WebView2 CDP）；同一台机器另有一个主检出 dev 壳在运行（对照实验用）"
dependencies:
  work_item_contract_digest: sha256:98aa641eac6856359c6ac518be14749ff547a9c78327586b62d542ed67347bc4
  artifacts: []
acceptance:
  - acceptance_id: AC-001
    outcome: passed
    method: 在 worktree 隔离真实 Tauri 壳上跑 npm run test:tauri:flow -- 筛选 --isolated-runtime，共两轮；新增的 ID 片段搜索步骤在两轮中都以大写 12 字符自然键片段填充搜索框，先等待可见卡片逐张满足「ID 或标题包含查询串」再比对期望计数
    evidence: 两轮均无「ID 搜索计数不一致」异常且执行推进到后续 todo 提示步骤才停止，说明 ID 断言全部通过；结果文件 .tmp/tauri-realtest/20261014-102441/result.json 与第二轮同目录时间戳记录（worktree 内），步骤错误均为「临时 todo fixture 写入后仍未显示 ready nudge」
  - acceptance_id: AC-002
    outcome: passed
    method: 同一 worktree 隔离真实 Tauri 壳上跑 npm run test:tauri:smoke -- --isolated-runtime；另以筛选流程中先于 todo 步骤的 provider 过滤、标题搜索、空状态、清空恢复断言作为补充
    evidence: 冒烟四步全过（结果文件 .tmp/tauri-realtest/20261014-102912/result.json，通过=true）；两轮筛选流程中上述断言先于失败点全部通过（若失败会以各自错误信息抛出，实际错误只有 todo 提示一条）
---

# 验收：搜索栏按会话 ID 过滤（v2）

## 这一版改了什么

与第一版的唯一差别：结论从 partial 改为 passed。第一版把「人工核对清单三条未测」当成了验收记 partial 的理由；人工项的开关在 manual-test 记录里，不属于本记录的结论口径。两条验收条目的方法与证据未改动，逐字保留。用户已于 2026-10-14 在对话中报告三条人工项全部通过，勾选以 manual-test.md 文件里的 `[x]` 为准。

## 逐条对照

- AC-001 通过：两轮隔离真实 Tauri 壳运行中，大写自然键片段搜索的等待条件与计数断言全部通过。
- AC-002 通过：冒烟流程四步全绿；筛选流程中既有的 provider、标题搜索、空状态、清空恢复断言全部先于失败点通过。

## 既有失败与本改动的无关性证明

筛选流程末尾的 todo 提示步骤三次失败：worktree 新代码两轮、主检出 dev 分支旧代码对照一轮，错误完全相同（「临时 todo fixture 写入后仍未显示 ready nudge」，`verifyTodoWorkspaceScope`）。同一台机器上有两个 AgentWatcher 实例共享 `%APPDATA%\AgentWatcher\todos.v1.json`，常驻实例会用自身内存态覆写测试写入的 fixture，属环境性既有问题，已登记待办。
