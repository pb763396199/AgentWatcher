---
schema_version: "1"
protocol: "1.3.0"
artifact: "manual-test"
artifact_id: "ar_01M7AG8TY3000DG3NNJSY9D33Q"
work_item_id: "wi_1QYFZ7SPZ2TGAMZAMK448JP45C"
created_at: "2026-11-13T13:06:39Z"
producer: "aes-validate"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:807ccdf792d9da9509eddb8e1cc706954c00ea16bf8f9c44bb645b13b854af56"
  artifacts: []
result: "pending"
---
# 人工验收测试表

## 推荐入口：让 AI 调技能

本轮已提供 v5 技能入口。用户无需自行建 fixture 或编写 JSON；下面的命令表只保留给接口排障。

在新 Codex 会话中输入：

```text
$agentwatcher 在隔离目录中开启一个任务流程测试。先读取技能和 workflow guide，完成一个有实际检查证据的小任务并提交原作者 handoff。默认等待我确认，不接受交付，不启动下游。最后告诉我如何用本技能继续。
```

也可以直接接续本轮已运行的真实 Codex 测试：

```text
$agentwatcher 使用 workflow-home F:/AiProject/AgentWatcher/.tmp/skill-task-test/ec76b294-da45-496f-a0e7-3a1be81012e5，读取项目 skill-test 的 next 和 snapshot。向我说明 handoff 和证据，等待我的决定，不新建任务或启动下游。
```

人只需判断：技能是否能被选中；AI 是否读出原任务及原作者证据；是否在确认前保持下游未启动。机器已验证的创建、游标、重启和唯一归属无需再手抄命令重测。新测试会调用模型并产生用量。

这份表只记录机器无法替人判断的内容。请在真实 Tauri 窗口中按顺序执行，每行只测一件事。
完成后把首列的 `[ ]` 改成 `[x]`；失败改成 `[!]`，并在表格下方写出实际现象。不要替其他人勾选。

## 测试前准备

1. 在仓库根目录打开 PowerShell，确认分支是 `dev`：`git branch --show-current`。
2. 启动真实桌面壳：`npm run dev`。不要用 `npm run dev:ui` 作为本表验收。
3. 另开 PowerShell，查看指南：`cargo run --manifest-path src-tauri/Cargo.toml --no-default-features --bin agentwatcher-cli -- workflow guide`。
4. 若要用真实 Codex 施工，先使用临时仓库和单独 `--workflow-home`；不要把本表的测试请求发给真实项目。
5. 证据截图和日志放在 `F:\AiProject\AgentWatcher\workflow\多会话任务服务与-Codex-调度\assets\manual-<编号>.*`。

如果没有现成的 `submitted` handoff，先运行：

```powershell
pwsh -File .\tools\workflow-manual-fixture.ps1
```

复制脚本输出的 `workflowHome`，在同一个 PowerShell 设置 `$env:AGENTWATCHER_WORKFLOW_HOME`，再运行 `npm run dev`。
脚本只在 `.tmp` 下创建隔离数据库，不接触真实项目；M-003 可以直接使用它生成的待确认 handoff。

## 验收表

| 状态 | 编号 | 操作 | 通过条件 |
| --- | --- | --- | --- |
| [ ] | M-001 | 在主窗口点击“工作流总控”，刷新一次。 | 窗口显示真实项目、路线版本、节点状态和通知；刷新后内容一致。 |
| [ ] | M-002 | 用 `workflow apply` 提交 `control.set` 暂停测试项目，再关闭并重新打开只读总控。 | 项目仍是暂停；没有因为界面关闭而自动恢复或新开执行。 |
| [ ] | M-003 | 用 `workflow next` 找到 `submitted` handoff，再用 `workflow apply` 提交 `handoff.accept`。 | 节点变为已接受；没有自动启动未明确选择的下游节点。 |
| [ ] | M-004 | 用 `workflow apply` 提交带 `continueNodes` 的 `handoff.accept`。 | 只有选定的下游获得授权；其他下游保持等待。 |
| [ ] | M-005 | 用 `workflow apply` 提交带 `reason` 的 `handoff.reject`。 | 原执行回到施工状态；没有生成第二个作者会话。 |
| [ ] | M-006 | 在另一个 PowerShell 执行 `agentwatcher-cli service stop`，观察正在施工的 Codex 窗口。 | 调度服务停止，但已有施工窗口没有被杀掉；重新执行工作流 CLI 后能继续看到同一 thread。 |
| [ ] | M-007 | 在项目有活动施工时尝试恢复一个备份。 | 恢复被拒绝，并明确要求先安全停止活动执行；原数据库和工作区不变。 |
| [ ] | M-008 | 用 `workflow apply` 提交 `roadmap.propose`，再用 `workflow watch --after <旧游标>` 读取事件。 | 影响闭包被冻结并出现暂停通知；不受影响的项目/节点仍可查看和继续。 |
| [ ] | M-009 | 在两个不同 workspace 登记同一项目的不同节点，再分别打开总控。 | 每个 workspace 显示自己的执行与分支；同一节点不会同时出现两个有效作者。 |
| [ ] | M-010 | 用 CLI 挂载一个插件但保持未启用，再执行 `workflow plugin`；启用后重试。 | 未启用时明确拒绝且不执行进程；启用后只执行描述文件声明的命令 ID。 |
| [ ] | M-011 | 用 `workflow next` 读取审批，再用 `workflow apply` 提交匹配审批 ID 的 `runner.approval_reply`。 | 只回复当前审批 ID；拒绝或刷新后不会批准另一条审批。 |
| [ ] | M-012 | 用 CLI 执行 `service stop`，再关闭总控并重新启动 CLI 查询。 | 主窗口只是只读诊断；服务停止但已有施工状态保留，重新启动后可继续收件。 |

## 失败记录

| 编号 | 实际现象 | 截图/日志绝对路径 | 复测结果 |
| --- | --- | --- | --- |
|  |  |  |  |

## 完成规则

- 全部 `[x]`：把 `result` 改为 `passed`，并把每行的实际证据补进表格。
- 有 `[!]`：保留 `result: failed`，在失败记录中写复现步骤；不要继续合入。
- 还有 `[ ]`：保持 `result: pending`，任务仍等待人工验收。
