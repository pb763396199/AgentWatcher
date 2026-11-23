---
schema_version: "1"
protocol: "1.3.0"
artifact: manual-test
artifact_id: "ar_01M83B66AY000EKXHXNNFXXQFD"
work_item_id: "wi_0ZQ8FFAX6K2B4D8F66T96TFDEM"
created_at: "2026-11-23T04:40:00Z"
producer: "aes-validate"
result: "passed"
dependencies:
  work_item_contract_digest: sha256:377edb0dac11131577aa3523826b0b7accc7a012a8b0fef5fe9226ef0433ed54
  artifacts: []
---
# 人工核对清单

零条目。原五条人工项的处置与授权：

- 两条（接续导出落盘、429 会话超窗变闲置）按准入规则移回机器证据——真实 CLI
  handoff export 落盘核验、评审终审黑盒验证、真实 session show 返回 idle，见
  validation/validation-v4.md 的 AC-008 与 AC-001 证据。
- 余下三条（悬浮预览一致性、四组合主题语言、真实 ask_question 场景的等回复判定）
  由用户授权移除，未做人工核对。

用户授权记录：用户 pengbo 于 2026-11-23 在任务会话中两次明确指示跳过人工核对
直接合入（「你帮我勾」「我授权你勾，赶紧合」）。范围：本任务全部人工核对项。
理由：赶交付，风险已知——等回复判定按 protobuf 结构推断上线（设计记录未决
问题 3 已留档）、四组合界面渲染由自动流程部分覆盖（中文/默认主题）。失效条件：
界面上发现预览内容不一致、主题/语言渲染异常或等回复状态判定错误时，回补人工
核对并重开本清单；agent 不得替人补记 [x]。
