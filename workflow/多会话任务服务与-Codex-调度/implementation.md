---
schema_version: "1"
protocol: "1.3.0"
artifact: "implementation"
artifact_id: "ar_01M7A945SS000AHDABHTACX4A4"
work_item_id: "wi_1QYFZ7SPZ2TGAMZAMK448JP45C"
created_at: "2026-11-13T11:01:46Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:807ccdf792d9da9509eddb8e1cc706954c00ea16bf8f9c44bb645b13b854af56"
  artifacts:
    - artifact_id: "ar_01M7A945S1000809QW2T004P35"
      digest: "sha256:91af54e852d2d3d0965e8f21b1b9acd5f25ea51429f2c4076bd7541ce41ce7d7"
      locator: "change-note.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "34493f3b35552dad87cdbe860d8b0341caf35e6d"
    revision: "34493f3b35552dad87cdbe860d8b0341caf35e6d"
    tree: "caca832a448a9e09a44cad3402650b7430e8fa0c"
    content_digest: "sha256:65871a827481ffc6dbfb249e25d5f9938826706c91f3d4b0067dbbdf3b60185c"
    branch_or_pr: "dev"
    workflow_excluded: true
result: "complete"
---
# 施工回执

## 做了什么

完成通用持久任务服务的 Windows 首版，数据库、CLI、独立 Codex 运行器、Git 候选整合和桌面人工确认使用同一套状态。

## 逐步结果

| 步骤 | 结果 |
| --- | --- |
| P1–P2 | SQLite WAL、请求幂等、唯一归属、输入来源、版本与依赖失效；11 条真实 CLI 工作流回归通过。 |
| P3–P4 | 私有本地服务名片、单例、按需唤起、暂停/改线/原会话修复，2 次反思、3 次转人。 |
| P5 | 临时 Git 仓库验证准备、独立评审、候选、CAS 发布、主分支里程碑。 |
| P6 | 真实 Codex 作者与另一只读 thread 完成交付和评审；活动锁与明确恢复有专门契约测试。 |
| P7 | 库与引用产物 zip 备份，恢复暂停、拒绝覆盖活动施工、旧文档显式导入和里程碑逐仓库结果。 |
| P8 | 工作流总控、接受/退回/选定下游、审批和恢复入口；真实 Tauri 接受 fixture 交付与 smoke 通过。 |
| P9 | README、AGENTS、内嵌技能和服务指南更新；代码与证据保持未提交供审阅。 |

## 改了哪些代码

主要模块位于 `src-tauri/src/workflow/`；CLI 新编排在 `workflow_commands.rs`；GUI 入口在 `lib.rs` 与 `ui/index.html`。完整列表见 change-note.md 和 assets/working-changes.json。

## 自审发现的问题

| 问题 | 发现与处理 |
| --- | --- |
| 上游变化却复用下游旧结论 | 补传递失效、旧授权取消、全局约束计入合同；回归通过。 |
| 服务崩溃误以为旧执行已消失 | 持久归属不超时释放，核对运行器锁；恢复只接原 thread。 |
| Codex CLI 是 JS、Windows 直接执行失败 | 实际路径检查后以 Node 启动；真实运行验证通过。 |
| 备份恢复覆盖尚在施工的任务 | 恢复前检查活动执行与锁；拒绝覆盖测试通过。 |
| 会话别名绕过归属或自审 | Codex ID 归一化；深链和大小写回归通过。 |

## 哪里没按计划走

| 做了什么 | 与计划的差别 | 原因 |
| --- | --- | --- |
| 运行器真实测试处理一次 Git 审批 | 临时脚本等待明确审批后继续 | worktree 的 Git 元数据在沙箱写目录外；只批准隔离 fixture 的指定提交。 |
| 手工保存工作区证据记录 | 旧 assemble 拒绝未提交树，record verify 缺任务 Verify 映射 | 不为绕过旧工具自动提交；保留真实直接测试输出，不宣称旧发布门禁通过。 |
| 外部具体 AES 插件未新增 | 此轮聚焦宿主通用服务 | 遵守用户插件边界；已有规划技能可生成合同，通用 stdio 调用已接通。 |

## 跑过的检查

CLI contract 13、taxonomy 20、工作流 11、Codex 恢复 1、Git 1 均通过；库测试 178 通过、2 个既有 live 项忽略。headless check、GUI build/check、UI build、Bridge/harness 语法检查通过。真实桌面工作流与 smoke 通过，证据保存在 assets。真实 Codex 回执在 assets/real-codex-code.json。

## 剩余风险

没有 macOS 实机验收。writeScope 依靠合同、隔离 worktree 与评审，不是文件系统级逐路径限制。未知 thread 的启动结果只能人工核对，不能盲目补建。旧记录导入不自动赋予新路线验收资格；已有 worktree 的重做基线需要明确核对。当前未出发布包、未迁移用户真实任务。


## 第二轮：CLI 主闭环

根据用户批准的方向，`workflow watch` 提供事件游标增量和最长 30 秒等待，`workflow next` 提供只读动作收件箱。Tauri 工作流窗保留为只读诊断，业务状态变化统一走 `workflow apply`。本轮契约测试新增 watch/next 回放，工作流测试共 12 项通过；真实 Tauri 流程只验证只读诊断窗。
