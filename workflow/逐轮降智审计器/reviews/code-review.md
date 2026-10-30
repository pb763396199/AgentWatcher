---
schema_version: "1"
protocol: "1.3.0"
artifact: "review"
artifact_id: "ar_79XFQVPPEZ69XS2MEC34B8B8X4"
work_item_id: "wi_5VYQZTXYZD0H5M6CN9TKTRFJC3"
created_at: "2026-10-30T02:54:52Z"
producer: "aes-review"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:6848ab9fb6f89246f58decd1503a13d728d7aa614109393c23af3b6f2ffae203"
  artifacts:
    - artifact_id: "ar_35QQ7HZD7QA793KM6KP7NWRYTB"
      digest: "sha256:b7fb0886c5c81d4db8f1c3cc6cb0748b638a46516d11c292e61a4ab62792b8b1"
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
verdict: "approved"
review_type: "code"
reviewers: []
---
# 代码评审

## 声明

自己评审自己：本记录由本任务的执行者（aes-execute 同一执行单元）产出，评审对象是 feature/逐轮降智审计器 上的四个提交（3f42689、c6ac6a7、5f2db9a、e8dbd94）。

## 结论

通过。四个提交聚焦同一件事，序列化形状向后兼容（新字段全部可空省略，旧快照测试未改断言全过），隐私边界未被扩大（审计行只有元数据无正文，预览摘要沿事件通道），无插件宿主规则触碰。

## 审查中抓到的问题

| 问题 | 位置 | 后果 | 判定 | 处置 |
| --- | --- | --- | --- | --- |
| 预览窗口自行 invoke 被能力面挡住，摘要行永远空白 | ui/index.html 第一版 appendPreviewAuditLine | 功能在真实预览窗不可见 | 阻断（已在 e8dbd94 修复并真机复验） | 已修 |
| 摘要请求经 analyticsTargetFromSession 拿到空 id，静默空手而归 | 同上 enrichPreviewAuditSummary | 同上 | 阻断（同上修复） | 已修 |
| 离屏构建时 isConnected 卫语句吞掉同步填充 | fillPreviewAuditLine | 同上 | 阻断（同上修复） | 已修 |

三处问题都在实现记录「自审发现的问题」表里留了发现过程，修复提交 e8dbd94 一次盖住。

## 已核实的事实

- `LifecycleCallPoint`/`SessionLifecycle` 新字段全部 `Option` + `skip_serializing_if`，`cargo test` 126 条既有断言零改动通过；CLI 两个契约测试（8+9）通过，命令面未扩。
- 交接包新条目 `audit/turns.ndjson` 走既有 manifest SHA256 清单路径，无逐轮数据时整文件省略，旧读方不受影响；真实打包解包核对过首行 JSON。
- 信号判定集中在 `build_turn_audit` 纯函数，阈值（中位数三成、三轮下限）是具名常量；四类信号不合成分数。
- UI 文案中英文成对、颜色全部走主题变量；审计数据不进 localStorage（预览摘要由主窗口经 `agentwatcher-preview-data` 事件补发）。

## 非阻断问题

- Claude 的 `service_tier`/`speed`/`fallback_credit` 目前只随原始字段收集进调查记录，未上屏；后续想看再说，不阻塞。
- 降采样（超 5000 点）会话的信号基于已采点位，返回带 `degraded`、UI 注明，精度损失可接受。

## 没覆盖的范围

- Copilot Chat 分支只过了代码级 fixture（本机无活跃会话文件），真机核对在人工清单。
- 未评审打包脚本与 Bridge（本次未动它们）。
