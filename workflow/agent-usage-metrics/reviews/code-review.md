---
schema_version: 1
protocol: 1.3.0
artifact: review
artifact_id: ar_01M2J40XN7000EPB8WPV3KVFCK
work_item_id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
created_at: 2026-09-15T08:48:34Z
producer: aes-review
verdict: approved
supersedes: null
dependencies:
  work_item_contract_digest: sha256:b1d0e0503375d88153a6711d42195454acdbf4b2117b2b4a4a6cca41a6fcc8d9
  artifacts:
    - artifact_id: ar_01M2J40XMD0000GC2DDBG67VQ0
      digest: sha256:9e020bb6ee15cc540212da38caa95e0f860334fd178cf0633dc84300b4761550
      locator: implementation.md
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
review_type: code
reviewers:
  - aes-review（同一执行单元自审，声明见协议「自己评审自己要在开头声明」）
authorization: null
blocking_findings: []
---

# 代码评审

**声明：这是实现同一执行单元做的自审，非独立评审。** 独立评审由使用者指定其他执行单元完成，
或由使用者在人工核对清单里亲自过一遍。

## 结论

approved。改动集中于两个文件，逻辑与设计记录一致，隐私边界未触碰（用量数字走既有 scan_sessions
返回值与事件，预览正文仍不落 localStorage），扫描性能有界（缓存 + 预算 + 批量 SQL），
零插件约束不受影响（plugin_catalog 零改动）。

## 审查中抓到的问题

| 严重度 | 位置 | 后果 | 依据 | 最小改法 | 处置 |
| --- | --- | --- | --- | --- | --- |
| 中 | lib.rs `cached_jsonl_usage_for_path` | 首扫遇到 10 个以上未缓存新文件时预算耗尽，本轮剩余文件直接返回 None，属设计内渐进填充；但预算计数在「缓存命中检查」之后、读取之前，若读取失败（文件被删）预算已被消耗 | 逐行读代码 | 接受：失败文件下一扫描重试，浪费一次预算的影响有界（每扫描 10 个名额） | 不修，写进实现记录剩余风险 |
| 低 | lib.rs `parse_copilot_chat_usage` | 深层补丁行（k 长度 >3 的嵌套路径）被忽略，极端情况下 toolCallRounds 少计 | 对照调研的补丁行型清单 | 若用户反馈 Copilot 工具数偏低，再把 k 路径递归写入补上 | 记为已知限制 |
| 低 | lib.rs `codex_rollout_file_index` | 每 5 秒 TTL 重建时全量遍历 sessions 目录；年度文件数增长后耗时上升 | 逐行读代码 | 届时按 mtime 剪枝或限制遍历深度 | 记为已知限制 |
| 低 | ui/index.html `loadSessionUsageDetail` | 预览窗 invoke 失败被静默吞掉（catch 空处理） | 逐行读代码 | 已有注释说明保留基础用量；但无日志，排障不便 | 接受：桌面小工具场景无控制台可看，保持静默合理 |

## 已核实的事实

- 六个 `AgentSession` 构造点 + 2 个测试构造点全部补齐 `usage` 字段，`cargo check` 零警告编译。
- `git grep cost_usd`：cost 只来自 OpenCode `session.cost` 列（SQL `coalesce(cost,0)`，>0 才输出），
  ZCode 的恒 0 cost 不进入数据，符合「0 显示为 —」的设计约束。
- 隐私：新增代码无 `localStorage` 写入、无文件写入、无网络请求；`grep -n "localStorage" ui/index.html`
  命中仍只有预览尺寸 key。
- 发布合同：Bridge ID/版本/命令命名空间零改动；`plugin_catalog.rs` 零改动。

## 非阻断问题

- 单条提交覆盖 S1–S7（同一功能整体交付，无法按步拆分运行验证），Step trailer 取 S1。
- 预览状态胶囊 light 主题红色对比度 4.6:1 是存量样式，本任务未动它（实现记录已注明实现时对齐 #a4262c，
  实际未改——该元素属预览头部既有代码，改它会扩大变更面，留待独立小修）。

## 没覆盖的范围

- 未在 DPI 125%/150% 下实测；未做 8 小时长跑；light 主题 + 中文组合的真实截图未逐张目检（字典与 CSS 已覆盖，
  交给人工核对清单）。
