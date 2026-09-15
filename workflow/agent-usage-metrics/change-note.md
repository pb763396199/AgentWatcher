---
schema_version: 1
protocol: 1.3.0
artifact: change-note
artifact_id: ar_01M2J40XKJ0004SY6GMZW2RKW6
work_item_id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
created_at: 2026-09-15T08:48:34Z
producer: aes-execute
result: complete
supersedes: null
dependencies:
  work_item_contract_digest: sha256:b1d0e0503375d88153a6711d42195454acdbf4b2117b2b4a4a6cca41a6fcc8d9
  artifacts:
    - artifact_id: ar_01M2J1HY7000034P57XZ03CQ8W
      digest: sha256:19eb2e8ec3d41f479085aae6ce8ee724b5831ee8ad840ddb52a589e64bccf62f
      locator: plan.md
  subject:
    kind: change_set
    digest: sha256:15ddace05bae1e680465fa93adb7d941ce6d9db06fa276ac27b6725893c3a280
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: bc915a208539f73abc4964919eabcfb69e66bbf6
    revision: bc915a208539f73abc4964919eabcfb69e66bbf6
    tree: d752bf95fbe0cca563743eb717c1563c9c38acc2
    content_digest: sha256:15ddace05bae1e680465fa93adb7d941ce6d9db06fa276ac27b6725893c3a280
    branch_or_pr: feature/agent-usage-metrics
    workflow_excluded: true
---

# 修改说明

覆盖账本：S1–S4（src-tauri/src/lib.rs）与 S5–S6（ui/index.html）的全部改动见下表，
S7 为验收门无代码改动，S8 为记录与提交。

| 改动 | 计划步 | 理由与依据 |
| --- | --- | --- |
| 新增 `SessionUsage` 结构体（13 字段）与 `AgentSession.usage`，None 跳过序列化 | S1 | 设计「数据结构」节；缺数据显示 — 而不是 0，是拍板结论 |
| 新增 Claude/Codex/Copilot Chat 三个纯解析聚合器与（大小、mtime）增量缓存，每扫描预算 10 个未缓存文件 | S2 | 调研实测三个 JSONL 源累计值头尾采样拿不到，必须全读；预算兑现拍板 5（渐进填充） |
| Claude 按 distinct `message.id` 去重求和；Codex 的 `input_tokens` 按毛口径（cached ⊆ input，仅加 cache_write）取最后一条 total，上下文跳过全零 input 事件 | S2 | 调研语义验证；Codex 毛口径另经本机 db 对照 message 侧求和确认（14,156,150 vs 13,946,408 同口径），测试期望曾因算错被纠正 |
| 六源接线：claude/copilot/codex(文件+app-server rollout 索引) 走聚合器，copilot_cli 用既有 SQL 行，opencode 扩 session 汇总列，zcode 三条 IN 批量 SQL（model_usage/tool_usage/message JSON） | S3 | 调研「仓库挂点」表；ZCode 旧库缺观测表时整体降级为无用量 |
| 新增 `get_session_usage_detail` 命令：opencode/zcode 走 per-session SQL，JSONL 源从聚合缓存取 topTools | S4 | 设计「展示形式」：工具明细与 OpenCode 明细懒加载，避免 4.46GB db 每次 scan 聚合 92.7ms/会话 |
| 卡片 `.card-usage` 行（轮次+工具+token），签名纳入用量关键字段，CSS 三档密度隐藏 + light 变体 | S5 | 拍板 2（方案 A）；compact 起隐藏与 card-foot 现有规则一致 |
| 预览「用量」区块（hero=累计毛输入+缓存占比，网格八格），payload 加 sessionId/usage，预览窗懒加载 detail 合并 topTools，i18n en/zh 各 20 键 | S6 | 拍板 1/3/4；Copilot CLI 显示无数据说明而不是一排 — |

例外与未解释改动：

- `Cargo.toml` 未改动；扫描过程中 git 曾因行尾归一化把它标记为修改，已还原。
- `vscode-agentwatcher-bridge/` 零改动（语法检查照常通过）。
- 计划外改动：无。
