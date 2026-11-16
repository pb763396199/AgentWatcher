# AgentWatcher workflow service

Use `agentwatcher-cli workflow apply --file <request.json>` (or `--file -` for stdin).
Requests have `schemaVersion: 1`, a unique `requestId`, a declared `command`, and an object `payload`.
Use `workflow snapshot --project <id>` to inspect persisted state and notifications.
Use `workflow watch --project <id> --after <event-sequence> --wait-ms 30000` to receive only new events.
Use `workflow next --project <id>` to receive the ordered action inbox; this command never changes state.
The client wakes a stopped service. `service status` is read-only; `service stop` stops scheduling, not external work.

Create a project with `project.create`, publish an explicitly human-confirmed roadmap with `roadmap.publish`,
and claim a ready node with `node.claim`. Keep the returned execution token private.
Pass `executionId`, `token` and `expectedRevision` when reporting progress or submitting a handoff.
Read current state before starting, changing write scope, or handing off; acknowledge pause notifications at a safe point.
Do not claim a second execution because the first appears idle or disconnected.

`execution.handoff` includes a summary, passing verification for each acceptance criterion, and an artifacts array.
Artifacts use absolute paths and SHA-256 digests. Receipt is not acceptance and does not start downstream nodes.
An independent session submits `review.submit`; a human accepts or rejects the exact handoff.
Roadmap changes use `roadmap.propose` followed by explicit human confirmation of a new revision.
Never invent human confirmation or replace another session's work.

Context size is a planning estimate, not a runtime deadline. Continue working toward the node's completion conditions.
Two consecutive follow-ups without progress trigger reflection; a third pauses for human input.
Commands never accept arbitrary program, args or shell execution.

The Tauri workflow window is a read-only diagnostic view. It is not the control plane and does not approve,
pause, reject, recover or authorize downstream work. An AI operator should use `next`, read `watch` from its
last cursor, explain the required decision to the human, and submit the approved JSON through `apply`.

## 从讨论到开工

先用规划技能和用户讨论目标、依赖、写入范围、验收、风险及每个节点的人要判断什么。
将讨论结果整理成节点合同，经过用户确认后发布。`humanConfirmed` 是调用者对已取得授权的声明，
agent 不得自行编造授权。该字段和本地服务身份不是对恶意本机程序的安全隔离。

每个 JSON 请求保存为文件，再运行 `agentwatcher-cli workflow apply --file request.json`。
请求 ID 重试时复用；修改内容时使用新 ID。以下示例中的 ID、路径必须换成实际值：

```json
{"schemaVersion":1,"requestId":"project-1","command":"project.create","payload":{"id":"my-project","title":"我的项目"}}
```

```json
{"schemaVersion":1,"requestId":"route-1","command":"roadmap.publish","payload":{"id":"route-1","projectId":"my-project","title":"已经讨论清楚的路线","goal":"最终交付什么","constraints":["用户确认的约束"],"expectedRevision":0,"humanConfirmed":true,"nodes":[{"id":"implement","title":"实现并验证功能","goal":"一个会话能够完成的明确目标","code":true,"writeScope":["src/feature.rs"],"acceptance":["实际可检查的验收条件"]},{"id":"document","title":"根据实现更新文档","goal":"使用上游真实成果","dependencies":["implement"],"acceptance":["文档与实际行为相符"]}]}}
```

代码节点先准备隔离 worktree，再开独立 Codex 会话。普通查询不会触发这两项操作：

```json
{"schemaVersion":1,"requestId":"prepare-1","command":"node.prepare","payload":{"roadmapId":"route-1","nodeId":"implement","expectedRevision":1,"repositoryPath":"F:/MyProject","targetBranch":"dev"}}
```

```json
{"schemaVersion":1,"requestId":"launch-1","command":"node.launch","payload":{"roadmapId":"route-1","nodeId":"implement","expectedRevision":1,"humanConfirmed":true}}
```

非代码节点启动时提供 `workspace`。已有手工会话用 `node.claim`，填写同样的路线/节点/版本、
`sessionId` 和实际 `workspace`；它只绑定归属，不争抢已打开会话的控制锁。
认领结果提供私有 token、节点合同、整体约束、已接受上游 handoff 和产物摘要。
通过 snapshot 检查项目、通知、节点和评审。原会话主动提交：

```json
{"schemaVersion":1,"requestId":"handoff-1","command":"execution.handoff","payload":{"executionId":"实际执行ID","token":"认领时的私有token","expectedRevision":1,"summary":"人能读懂的完成结论、检查和限制","codeRevision":"代码节点当前已提交HEAD","verification":[{"criterion":"实际可检查的验收条件","outcome":"passed","evidence":"真正执行的检查及结果"}],"artifacts":[]}}
```

产物条目为 `{"path":"绝对路径","sha256":"64位十六进制摘要"}`。数据库记录实际使用的输入清单；
下游认领会重新检查产物。代码依赖必须已经进入该 roadmap 的集成分支，跨仓库代码缺失时明确阻塞。
`writeScope` 随合同交给执行者并供独立评审检查；它不是操作系统级文件写入沙箱。

自动运行器会为代码节点及 `requiresReview:true` 节点启动独立只读评审。
手工评审使用 `review.submit`，提供 `handoffId`、`reviewerSessionId`、`verdict: approved|changes_requested`、
`summary`。作者会话不能自审。人通过界面或 `handoff.accept/reject` 确认具体交付。
拒绝需填写 `reason`，回到原会话修复。接受默认不启动下游；明确选择 `continueNodes:["document"]`
才能建立一次性的下游授权，也可以显式调用 `authorization.grant`。

## 改路线和止损

`roadmap.propose` 提供 `roadmapId`、`expectedRevision`、`reason`、`affectedNodes`。
服务冻结这些节点及所有下游，原执行者收到暂停通知；不替执行者伪造停止。
原执行者保留现场后通过 `execution.pause_ack` 提交 `executionId/token/expectedRevision/summary`。
人讨论新方案后再次 `roadmap.publish`，携带当前 `expectedRevision` 和 `proposalId`，发布完整节点集合。
上游要求变化会使下游旧结果待复核，并取消旧授权。未受影响节点继续沿用原合同。
旧执行通过 `execution.release` 释放前必须有安全暂停回执；待复核节点通过
`node.revalidate` 明确 `decision: redo|withdraw` 和 `reason`，不能把旧成果直接标为新版本通过。
已有代码 worktree 会保留；重做时应先人工核对基线，框架不删除或重置它。

`control.set` 提供 `projectId`、`state: active|paused|stopped`、`humanConfirmed:true`。
这是持久控制状态，重启不会清除。关闭主窗口仅收起，托盘“退出”停止调度服务，已经运行的施工保留。
工作结束或提交进展时，CLI 可再次唤起服务。另一个自动回复开关不能同时接管已经受此框架管理的会话。

`execution.progress` 提供私有执行身份、版本、`summary`、`evidence` 数组。
证据确实变化才计进展；相同证据连续两次触发指定反思，第三次转人。没有运行期上下文上限。

## 中断恢复、整合和备份

网络中断、启动结果不明不会自动创建第二个会话。核对原进程、原 thread 和工作区后，人可调用
`runner.recover`，提供 `operationId/reason/humanConfirmed:true`；仅当旧运行器锁已释放且 thread 已知时，
恢复同一个 thread。thread 身份未知时停在待核对，不能盲目重放。
审批请求保留原始 ID，通过界面或 `runner.approval_reply` 一次性回复，不自动批准。

接受代码交付后服务生成候选；快进候选可以按该次人工接受整合。并行分支合并导致候选变化时，
需重新验证并独立评审该 `candidateRevision`，再 `integration.publish`；CLI 要求
`candidateOperationId/expectedRevision/verifiedRevision/verificationPassed/independentReviewApproved/humanConfirmed`。
这些确认应有实际检查依据。主分支只在里程碑推进：`milestone.check` 提供项目和
`nodes:[{roadmapId,nodeId,expectedRevision}]`，验证返回的完整 `repositoryVersions` 后，
`milestone.land` 提供 `milestoneId/verifiedRepositoryVersions/expectedTargets/humanConfirmed`。
`expectedTargets` 为各仓库当前目标分支的 `{repositoryPath,revision}`，外部目标前进即拒绝。
跨仓库部分成功会留下逐仓库结果，不自动回滚任何仓库。

`storage.backup` 需要明确人工确认，输出包含一致性 SQLite 快照和引用产物的 zip。
备份不含源码仓库；Git 仓库需另行保存。`storage.check` 检查库、外键及产物。
恢复前停止服务，运行 `workflow restore --file backup.zip --confirm`；现有执行仍在施工或有活动运行器时拒绝覆盖。
可恢复到单独的空 `--workflow-home`，恢复结果默认暂停，原操作待核对，不自动重发旧任务。
旧数据库保留在 `pre-restore`。`legacy.preview` 只读 Markdown，`legacy.import` 需实际路径、
`projectId/expectedDigest/humanConfirmed`，只保存原始记录及来源；需要人工映射新路线，旧验证不算当前通过。

通用宿主零插件也能运行。插件调用只经 `workflow plugin --name <已启用插件> --command <已声明命令ID> --file <JSON>`，
使用既有 `AgentWatcherPluginStdio/1`。可选 roadmap `pluginName` 会在派发和执行轮询中检查启用状态。
具体 AES 规划技能、业务规则和插件源码应保存在独立插件目录/仓库。
