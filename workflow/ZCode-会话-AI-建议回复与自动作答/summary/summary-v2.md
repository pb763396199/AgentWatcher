---
schema_version: "1"
protocol: "1.3.0"
artifact: "summary"
artifact_id: "ar_01M65V7CC00007R9ZSKG2JA0B7"
work_item_id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
created_at: "2026-10-30T07:55:00Z"
producer: "aes-execute"
result: "complete"
scope: round
supersedes: null
dependencies:
  work_item_contract_digest: sha256:2b1c17e9e8d949086387fef35bc752d834b0026f368ec9f17c071b583ecf4d0a
  artifacts: []
---
# 第二轮施工总结

## 结论

按你的纠正对齐了 ChatGPT 的流程：零配置，生成复用 ZCode 自己的引擎与账号，不再有任何设置项和外部模型接口。

## 你现在能做什么

- 不用配置任何东西。ZCode 会话一停下等你，约半分钟后卡片上自动出现灰色建议（选择题时是选项按钮），点「发送」直接送回原会话。
- 卡片右键：「换一条建议」换一条（最多两次）；「自动作答此会话的选项」按会话打开全自动，护栏内置（每小时六次、连续同答案熔断、每次自动作答记审计文件）。

## 改了什么

| 变更 | 位置 | 锚点 |
| --- | --- | --- |
| 移除设置面、外部 API、热键；生成改走 zcode.cjs 无头会话 | src-tauri/src/lib.rs、Cargo.toml | S1、S2 |
| 右键菜单两个入口与按会话自动作答开关 | ui/index.html、lib.rs | S3、S4 |
| 流程测试改右键断言与零配置断言 | tools/tauri-realtest/cli.mjs | S5 |
| 文档口径 | AGENTS.md | S6 |

## 还欠什么

- 四条人工核对在 manual-test.md（自动建议显示、真实发送往返、右键与自动作答、换一条上限），等人勾完才能收尾合入。
- 每次生成会在你的 ZCode 会话列表里留下一个 scratch 会话（AgentWatcher 卡片墙看不到它），这是用自己引擎的代价，已在设计里声明。
