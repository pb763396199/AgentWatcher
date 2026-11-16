---
schema_version: "1"
protocol: "1.3.0"
artifact: "research"
artifact_id: "ar_01M7AJBN56000C5RC1KPJ1YYZ4"
work_item_id: "wi_1QYFZ7SPZ2TGAMZAMK448JP45C"
created_at: "2026-11-13T13:43:08Z"
producer: "agentwatcher"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:807ccdf792d9da9509eddb8e1cc706954c00ea16bf8f9c44bb645b13b854af56"
  artifacts: []
result: "complete"
---
# 技能入口更新与真实调用测试

## 检查范围

| 项 | 发现与处理 |
| --- | --- |
| 已安装技能 | 原来六个宿主均为 v3，只有仓库内嵌 v4；此次升至 v5 并安装到六个宿主，0 outdated；OpenCode 宿主缺席。 |
| 技能发现 | frontmatter 原描述只含会话查询；现加入开任务、沿 roadmap 执行及隔离测试，正文给出开工和接续步骤。 |
| CLI | PATH 的旧程序无 workflow；备份到 .tmp/skill-upgrade-backup 后更新本机 .local/bin 程序，实测支持 next/watch/apply。 |
| 授权 | 区分已获授权的测试路线发布、单次 handoff 提交审批、交付人工接受和下游授权。 |

## 实际测试

主会话按技能建立隔离测试项目并启动一个真实 Codex worker，worker 实际读取已安装的 agentwatcher v5 技能、workflow guide，执行 Python 验证 17 + 25 = 42，随后提交自己的 handoff。不是让 fixture 脚本代替 worker 编写成功结论。

工作流数据目录为 F:/AiProject/AgentWatcher/.tmp/skill-task-test/ec76b294-da45-496f-a0e7-3a1be81012e5，项目 skill-test、路线 skill-route。作者会话 codex:01a1d51f-dd7a-7742-960b-d3c998286d6c，执行 execution_17c9de33-da21-48bf-8c00-774fb7e114fa。

隔离执行的 handoff 写请求曾触发命令审批，核对 command、项目目录、executionId 后只批准该次提交；没有批准 handoff.accept 或任何下游。最终 handoff_1a29a0ae-5772-442f-a221-275f608ac626 处于 submitted，next 返回该项，只有一个执行，下游未启动。服务停止并按需唤起后身份及 submitted 状态一致。测试服务已停止，数据保留。

具体证据见 assets/skill-task-test-result.json，私有执行 token 已遮盖。CLI contract 13 和 taxonomy 20 项通过；本次只修改技能分发内容、契约及指南，未声称发布安装包或 macOS 验收完成。

## 限制

这证明当前主会话通过技能建任务，以及一个全新真实 Codex worker 能按已安装技能执行和主动提交；没有把整套上游三技能规划访谈伪装成此次算术任务的验收。后续新会话仍需对复杂目标完成规划讨论。
