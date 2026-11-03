---
schema_version: "1"
protocol: "1.3.0"
artifact: "review"
artifact_id: "ar_01M65V4PXE000FM4HZVJ9YVKXK"
work_item_id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
created_at: "2026-10-30T07:45:00Z"
producer: "aes-review"
review_type: "code"
verdict: "approved"
supersedes: "ar_01M65H7BTZ000FT540VJ0T6W0Z"
dependencies:
  work_item_contract_digest: sha256:2b1c17e9e8d949086387fef35bc752d834b0026f368ec9f17c071b583ecf4d0a
  artifacts: []
  subject:
    kind: "change_set"
    digest: "sha256:PENDING"
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "d4b38a6e9c8b7a68cd27fe25164e8244f6914a94"
    revision: "PENDING"
    tree: "PENDING"
    content_digest: "sha256:PENDING"
    branch_or_pr: "feature/ZCode-会话-AI-建议回复与自动作答"
    workflow_excluded: true
blocking_findings: []
---
# 代码评审（第三版：零配置改造的变更集）

取代第二版（ar_01M65H7BTZ000FT540VJ0T6W0Z，覆盖第一轮）。审查范围：第二轮两个提交（b299342、020e6d8）加对第一轮保留代码的连带影响，只读。

## 核对过的关键点

- 死锁修复未被回退：生成线程仍先 drop 锁再进全自动判定。
- 补时逻辑：评审中发现 020e6d8 之前的两行重算是错的（waiting_ms 加了 now_ms，数值失真但被 max 常量掩盖），已在 020e6d8 删掉，改为睡满后直接传常量满时长。复核修改处：睡眠发生在 auto_on 判定之后，未开自动作答的会话不会白睡。
- 无设置残留：grep 确认 ureq、global-shortcut、set_suggestion_settings、SuggestionSettings 全部移除，capabilities 无遗留声明（第一轮就没加，正确）。
- 生成入口收敛：扫描触发与「换一条」共用 spawn_suggestion_generation；regenerate_suggestion 在无现存建议时也能当"立即生成"用。
- run_child_with_timeout 返回 stdout 的改动对发送路径无影响（map 丢弃）；stdout 在进程退出后统一读取的缓冲限制与第一版相同，zcode --json 信封远小于管道上限。

## 非阻断问题

1. 「换一条」两次上限只在发送成功后重置（regen_counts 在 send 成功时清），一条建议一直不采纳时换满两次就到顶。后续可改为按指纹重置，本轮不动。
2. 每个 waiting 事件都会起一个 scratch 会话进 ZCode 会话库，长期会积累；AgentWatcher 侧被默认黑名单挡住，ZCode 桌面端列表会看到。设计 v2 已声明为接受的代价。

## 结论

approved：无阻断问题，两处非阻断已记录处置。
