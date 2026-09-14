---
schema_version: "1"
protocol: "1.4.0"
record: "wayfinder-ticket"
id: "wt_01M2FCG3WW0MQV4ZAG6N2GF96V"
work_item_id: "wi_3QQPFJCPXD24P1CCXMXAZ730TQ"
map_artifact_id: "ar_01M2F27YF1BYC844M1BSJXVR8K"
title: "ZCode 卡片跳转是否本期实现"
type: "research"
interaction: "afk"
executor: "aes-research"
status: "resolved"
created_at: "2026-09-14T07:18:26Z"
frontier_order: "100"
question: "官方渠道确认无法获得可用的 TUI 运行时（3.12.1 解包无 @zcode/tui、npm/镜像 404、CDN 无 CLI、SEA 缓存不存在）——本期是否保留 TUI 跳转实现？"
options:
  - title: "保留 TUI 跳转 + 运行时预检 + 明确报错"
    state: "rejected"
    reason: "上一轮实现；用户复查后拍板：既然做不了就先不实现，避免半吊子能力"
  - title: "跳转整体下线，ZCode 定位为只读监控 provider"
    state: "adopted"
    reason: "用户拍板（2026-09-14）：点击返回 not supported yet 提示；扫描/悬浮预览/接续上下文导出必须可用（已逐项验证）；handoff 移除 ZCode 目标模式避免死路选项；TUI 实现保留在 git 历史待上游就绪"
decision: "跳转整体下线，ZCode 本期定位为只读监控 provider"
decision_reason: "用户拍板：先把跳转不实现，但上下文检索与悬浮预览必须可用。官方渠道（深链/独立 CLI/TUI 运行时）短期无望，保留报错能力不如直接明确提示"
resolution_summary: "已落地：Rust 删除 launch_zcode_tui/ZcodeTuiRuntime/find_zcode_* 全套与 handoff zcode 模式，open_zcode_session 返回 not supported yet；前端移除 ZCode TUI 目标选项、并行恢复警示与死键；zcode-session flow 断言该提示；AGENTS/README 口径同步。悬浮预览真机验证通过（工作区/标题/状态/两段正文），上下文导出逐项审计通过"
resolution_artifact_ids: []
evidence_artifact_ids: []
source_fog_id: null
human_confirmation_artifact_id: null
blocked_by: []
reopened_from: null
assignee: "zcode-main"
claim_session: "sess_493c0cf7-8dda-4a39-9b91-53c9b3de5cc1"
claim_issued_by_session: null
claim_token_digest: "401f915d3d394233d1535485e9437f4fb91845caace58d94abf15bee4bcb8b04"
claimed_at: "2026-09-14T07:18:36Z"
resolved_session: "sess_493c0cf7-8dda-4a39-9b91-53c9b3de5cc1"
resolved_at: "2026-09-14T07:18:36Z"
out_of_scope_reason: null
withdrawn_reason: null
invalidated_by: null
domain_id: null
---

# ZCode 卡片跳转是否本期实现

## 问题

官方渠道确认无法获得可用的 TUI 运行时（3.12.1 解包无 @zcode/tui、npm/镜像 404、CDN 无 CLI、SEA 缓存不存在）——本期是否保留 TUI 跳转实现？
