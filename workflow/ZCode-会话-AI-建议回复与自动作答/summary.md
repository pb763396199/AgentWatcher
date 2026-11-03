---
schema_version: "1"
protocol: "1.3.0"
artifact: "summary"
artifact_id: "ar_1KQ77VFQEFTC417YF4H9V0N7NW"
work_item_id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
created_at: "2026-10-30T04:35:42Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:2b1c17e9e8d949086387fef35bc752d834b0026f368ec9f17c071b583ecf4d0a
  artifacts:
    - artifact_id: "ar_4QY67ZCXZ7YPS1Z73K7P90PVTR"
      digest: sha256:f12d8fecf62a32153d4ac169630c9f2729075bb0be0ba186d07b8ca3b96711c4
      locator: "implementation.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "012fe5b3f744a7e7e20286cc2c7875e106bc1dee"
    revision: "012fe5b3f744a7e7e20286cc2c7875e106bc1dee"
    tree: "a83bb06437d48123ec138074367482c68e8e0a34"
    content_digest: "sha256:13a1e2ffdfdc5460da25da7aed8b051b5345ece01cbf1321110d9558c24fb44e"
    branch_or_pr: "feature/ZCode-会话-AI-建议回复与自动作答"
    workflow_excluded: true
result: "complete"
scope: "round"
---
# 本轮施工总结

## 结论

ZCode 会话的 AI 建议回复从生成到全自动作答整条打通：会话一停下等人，卡片上就出现一行灰色建议（有选择题时直接列成选项按钮），点一下就送回原会话；愿意的话可以让它在你不在时自动替你选，但只在 agent 自己给的选项里选，默认关。

## 你现在能做什么

- 设置面板新增「AI 建议」分组：填一个模型接口（兼容 OpenAI 格式）即可启用，热键默认 Ctrl+Alt+K。
- 等待中的 ZCode 卡片直接点「发送」或选项按钮回复，不用切窗口。
- 打开「自动作答提问选项」后，同一个会话每小时最多自动答六次、连续两次选同一个答案就停，每次自动作答都记在 %TEMP%\AgentWatcheruto-answersudit.jsonl。

## 改了什么

| 变更 | 位置 | 锚点 |
| --- | --- | --- |
| 建议管线（触发、上下文、提示词、防陈旧） | src-tauri/src/lib.rs | S3、S4 |
| headless 发送与错误分类 | src-tauri/src/lib.rs | S5 |
| 全自动护栏与审计 | src-tauri/src/lib.rs | S6 |
| 卡片幽灵行、选项芯片、设置分组九项 | ui/index.html | S8 |
| 流程测试新增 AI 建议断言 | tools/tauri-realtest/cli.mjs | S9 |
| 文档口径 | AGENTS.md | S10 |

## 还欠什么

- 四条人工核对（建议行显示、真实发送往返、热键、设置重启生效）在 manual-test.md，等人勾完才能收尾合入。
- 桌面端开着的会话被命令行续话后 ZCode 窗口是否实时刷新，随 M2 一起看。
