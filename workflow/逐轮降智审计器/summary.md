---
schema_version: "1"
protocol: "1.3.0"
artifact: "summary"
artifact_id: "ar_2VY7KQSZYA2828K3VCWEH0TN1H"
work_item_id: "wi_5VYQZTXYZD0H5M6CN9TKTRFJC3"
created_at: "2026-10-29T09:50:00Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:6848ab9fb6f89246f58decd1503a13d728d7aa614109393c23af3b6f2ffae203
  artifacts:
    - artifact_id: "ar_35QQ7HZD7QA793KM6KP7NWRYTB"
      digest: sha256:b7fb0886c5c81d4db8f1c3cc6cb0748b638a46516d11c292e61a4ab62792b8b1
      locator: "implementation.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "e8dbd949b77d217d5e7bdf7773d5d12e4016a437"
    revision: "e8dbd949b77d217d5e7bdf7773d5d12e4016a437"
    tree: "a2335216ba100fdaf0bd0c46b47716c986e13c10"
    content_digest: "sha256:d17e421222a8a15f1aca54b1872097d7907aa31031f0a21dd0e26af6321ddec4"
    branch_or_pr: "feature/逐轮降智审计器"
    workflow_excluded: true
result: "complete"
scope: "round"
---
# 成果总结

## 结论

逐轮降智审计已经做完并全部验证通过：每个会话每一轮是谁在回答、什么档位、花了多少、有没有换模型或重试，现在有数据可查。等人工核对两 language × 两主题的界面细节。

## 你现在能做什么

- 悬停任意会话卡片，预览的用量区下多一行「模型链 · N 个信号」，点它直接打开会话分析。
- 会话分析里多一张可折叠的「逐轮审计」表：轮次、模型、档位、输出、首 token、信号徽标；用量时间轴上模型切换画红虚线、异常轮画红点，点红点跳到对应行。
- 按模型的分解现在带档位标签（有档位数据的服务）。
- 右键打包的交接包里多一份 `audit/turns.ndjson`，接手的人不用 AgentWatcher 也能核对当时的模型与用量。
- Copilot CLI 会话会明确显示「该数据源不支持逐轮审计」，不再猜。

## 改了什么

| 变更 | 落点 | 为什么 | 锚点 |
| --- | --- | --- | --- |
| 逐轮审计结构与四类信号判定 | `src-tauri/src/lib.rs` | 降级痕迹原来被全会话聚合平均掉 | S2 |
| 六个数据源填充审计字段 | 同上（六个时间轴解析器） | 每轮模型/档位/用量/重试来自各自落盘 | S3 |
| 分析面板审计表、时间轴标记、模型×档位分解、预览摘要行 | `ui/index.html` | 审计要看得见、点得着 | S4 |
| 交接包导出 `audit/turns.ndjson` | `src-tauri/src/lib.rs`（export_session_package） | 跨电脑交接后仍可核对 | S5 |

## 还欠什么

主动不做：跨会话趋势视图、CLI `session turns` 子命令、可配置的低输出阈值（设计未决问题，等真实使用反馈）。没做完：Copilot Chat 字段映射的真机会话核对（本机当前没有活跃文件），已列进人工核对清单。
