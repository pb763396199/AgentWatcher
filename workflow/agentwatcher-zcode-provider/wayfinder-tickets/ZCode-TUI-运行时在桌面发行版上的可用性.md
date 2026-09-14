---
schema_version: "1"
protocol: "1.4.0"
record: "wayfinder-ticket"
id: "wt_01M2F47PRW03KE6W9NGYHZ63A3"
work_item_id: "wi_3QQPFJCPXD24P1CCXMXAZ730TQ"
map_artifact_id: "ar_01M2F27YF1BYC844M1BSJXVR8K"
title: "ZCode TUI 运行时在桌面发行版上的可用性"
type: "research"
interaction: "afk"
executor: "aes-research"
status: "resolved"
created_at: "2026-09-14T04:54:02Z"
frontier_order: "100"
question: "桌面版附带的 zcode.cjs 能否直接跑 TUI（--resume 恢复会话）？@zcode/tui 运行时从哪里来？"
options:
  - title: "桌面 bundle 直接跑 TUI"
    state: "rejected"
    reason: "实测 node zcode.cjs tui / --resume 均报 Cannot find package @zcode/tui：官方桌面发行版（3.11.2 / CLI 0.16.5）不带该包；zcode-tui-runtime 资产只在 SEA 单文件里，bundle 是普通 CJS"
  - title: "npm 安装 @zcode/tui"
    state: "rejected"
    reason: "实测 npm view @zcode/tui 与 @zcode/cli 均 404；官方无 npm 分发（zcode-cli-stream 3.7.5-12 为社区 fork）"
  - title: "PATH 独立 zcode CLI 优先 + bundle TUI 预检 + 明确报错"
    state: "adopted"
    reason: "AgentWatcher 实现：find_zcode_cli_on_path 优先；bundle 路线预检 node_modules/@zcode/tui/dist/index.js，缺失时返回 ZCode TUI runtime not available，不弹注定失败的终端；独立 CLI（SEA，TUI 内嵌）可用时完整恢复"
  - title: "等官方深链/独立 CLI 分发"
    state: "open"
    reason: "官方 install 文档只发桌面版；feedback#465（会话深链）无响应。上游就绪后本路线可升级"
decision: "PATH 独立 zcode CLI 优先 + bundle TUI 预检 + 明确报错（ZCode TUI runtime not available）"
decision_reason: "修正上一张票的隐含假设：--resume 参数存在≠桌面 bundle 能跑 TUI。实测桌面发行版无 @zcode/tui（npm 404、官方只发桌面版）；SEA 资产只在独立 CLI。保留 TUI 会话级恢复为产品路线，运行时可用性作为机器边界处理"
resolution_summary: "AgentWatcher 落地：open_zcode_session 先探 PATH zcode（SEA 内嵌 TUI），再用 zcode.cjs+node 并预检 node_modules/@zcode/tui/dist/index.js；不可用即报错不 spawn。handoff zcode 模式同路径同边界。flow 测试接受双分支（恢复成功/明确报错）。上游就绪（官方独立 CLI 或深链）后零改动升级"
resolution_artifact_ids: []
evidence_artifact_ids: []
source_fog_id: null
human_confirmation_artifact_id: null
blocked_by: []
reopened_from: null
assignee: "zcode-main"
claim_session: "sess_493c0cf7-8dda-4a39-9b91-53c9b3de5cc1"
claim_issued_by_session: null
claim_token_digest: "43db9f2aa90f3ccfcf48d93830994d13f452b9c338af997504823c826493dbaf"
claimed_at: "2026-09-14T04:54:07Z"
resolved_session: "sess_493c0cf7-8dda-4a39-9b91-53c9b3de5cc1"
resolved_at: "2026-09-14T04:54:19Z"
out_of_scope_reason: null
withdrawn_reason: null
invalidated_by: null
domain_id: null
---

# ZCode TUI 运行时在桌面发行版上的可用性

## 问题

桌面版附带的 zcode.cjs 能否直接跑 TUI（--resume 恢复会话）？@zcode/tui 运行时从哪里来？
