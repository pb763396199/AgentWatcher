---
schema_version: "1"
protocol: "1.3.0"
artifact: "validation"
artifact_id: "ar_01M7A945WS0003ZXEY7FCA2NSV"
work_item_id: "wi_1QYFZ7SPZ2TGAMZAMK448JP45C"
created_at: "2026-11-13T11:01:46Z"
producer: "aes-validate"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:807ccdf792d9da9509eddb8e1cc706954c00ea16bf8f9c44bb645b13b854af56"
  artifacts:
    - artifact_id: "ar_01M7A945SS000AHDABHTACX4A4"
      digest: "sha256:049898ca03ae147b2c8317745da9261fbbafea499715fc7503005b23341ae6f0"
      locator: "implementation.md"
    - artifact_id: "ar_01M7A945W2000FVRYVEPK4MB2N"
      digest: "sha256:1138800e26b9c766c909332a0dfdd0cad61d6818754baf4b8c6dceeab57c3a7d"
      locator: "reviews/code-review.md"
    - artifact_id: "ar_01M7AG8TY3000DG3NNJSY9D33Q"
      digest: "sha256:7bca5f7fcc026d5e94d3b905b49d21337b44922010188ad72eab664b54ec372f"
      locator: "manual-test.md"
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
outcome: "partial"
executed_at: "2026-11-13T11:01:46Z"
environment: "Windows; PowerShell; dev working tree; isolated SQLite/Git/Codex fixtures"
acceptance: [{"acceptance_id": "AC-001", "outcome": "passed", "method": "SQLite、项目隔离、持久状态", "evidence": "workflow contract 12 项通过，含 watch/next 回放"}, {"acceptance_id": "AC-002", "outcome": "passed", "method": "单例、read-only status、服务退出再唤起", "evidence": "state-tests.txt；real-codex-code.json"}, {"acceptance_id": "AC-003", "outcome": "passed", "method": "并发认领、幂等、版本、会话别名", "evidence": "state-tests.txt"}, {"acceptance_id": "AC-004", "outcome": "passed", "method": "原作者 handoff、独立只读 thread、人接受", "evidence": "real-codex-code.json"}, {"acceptance_id": "AC-005", "outcome": "passed", "method": "两次反思、三次转人、暂停重启", "evidence": "state-tests.txt"}, {"acceptance_id": "AC-006", "outcome": "passed", "method": "改线冻结闭包、停止回执、旧授权失效", "evidence": "state-tests.txt"}, {"acceptance_id": "AC-007", "outcome": "passed", "method": "真实 Codex 创建 thread、退出服务保留施工；锁恢复合同", "evidence": "real-codex-code.json；state-tests.txt"}, {"acceptance_id": "AC-008", "outcome": "passed", "method": "真实 Git 候选、CAS、里程碑与下游代码门禁", "evidence": "state-tests.txt；real-codex-code.json"}, {"acceptance_id": "AC-009", "outcome": "passed", "method": "数据库及产物备份恢复、旧源预览与显式导入", "evidence": "state-tests.txt"}, {"acceptance_id": "AC-010", "outcome": "passed", "method": "CLI taxonomy/contract、headless 构建", "evidence": "cli-contract-tests.txt；headless-check.txt"}, {"acceptance_id": "AC-011", "outcome": "passed", "method": "真实 Tauri 只读诊断流程与原有 smoke", "evidence": "native-workflow.json；native-smoke.json；workflow-console.png"}, {"acceptance_id": "AC-012", "outcome": "passed", "method": "检查计划、实现、指南和当前代码摘要", "evidence": "working-changes.json；change-note.md；implementation.md；../../../src-tauri/workflow-guide.md"}]
---
# 验证回执

本次机器验证适用于 Windows 开发工作区；人工验收仍未完成，当前总体验收为 partial。请按 [人工验收测试表](F:/AiProject/AgentWatcher/workflow/多会话任务服务与-Codex-调度/manual-test.md) 逐项核对。第二轮已把控制面收敛到 CLI/服务，Tauri 工作流窗只读。逐条方法和证据见 acceptance 字段；所有证据均来自实际执行，并非从模型宣称完成推断。

命令日志保存在 assets。CLI contract 首次因技能文档升级到 v4 后断言仍为 v3 失败，修正版本断言后 13 项全过。第二轮新增 `workflow watch` 游标和 `workflow next` 动作收件箱，工作流状态测试共 12 项通过。真实模型代码任务经过一次隔离 Git 提交审批，作者和评审 thread 不同，最后目标分支确实指向交付 commit。

旧 awf record verify 因本任务没有旧协议 Verify 映射而拒绝，原始结果保存在 assets/legacy-verifier.json；它不代替本回执，也没有被标成通过。旧 assemble 还要求干净提交树；当前工作区未提交，因此未宣称旧提交/发布门禁已闭合。记录结构另行运行 record validate 检查。

macOS 实机、发布包和人工测试表尚未闭合；云端断线后 thread/resume 只完成本地合同验证。没有对用户实际任务做迁移或自动授权。
