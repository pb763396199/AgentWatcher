---
schema_version: "1"
protocol: "1.4.0"
record: "wayfinder-ticket"
id: "wt_01M2F2878XN61FYM1E1Y914VSX"
work_item_id: "wi_3QQPFJCPXD24P1CCXMXAZ730TQ"
map_artifact_id: "ar_01M2F27YF1BYC844M1BSJXVR8K"
title: "ZCode 卡片点击跳转的可行路线"
type: "research"
interaction: "afk"
executor: "aes-research"
status: "resolved"
created_at: "2026-09-14T04:19:22Z"
frontier_order: "100"
question: "外部程序能否让 AgentWatcher 点击卡片后恢复 ZCode 的具体会话（sess_<id>）？桌面深链 / 远控 relay / CLI TUI resume / app-server 协议宿主四条路线哪条可行？"
options:
  - title: "桌面深链 zcode://session/*"
    state: "rejected"
    reason: "实测（app.asar handleDeepLink 静态分析 + 运行时 18 进程零监听端口）：路由仅 workspace/open、oauth/callback、payment/callback；官方反馈 zai-org/feedback#465 请求该能力，Open 无响应"
  - title: "远控 relay 伪装客户端"
    state: "rejected"
    reason: "云端 wss relay + 桌面生成配对 token + 单页面限制，无公开 API 文档，只能算 hack，不作产品路径"
  - title: "CLI TUI resume（node zcode.cjs --resume <id> --cwd <ws>）"
    state: "adopted"
    reason: "官方参数；与桌面共用 ~/.zcode/cli/db/db.sqlite（运行时实测）；node v24 满足 node:sqlite>=22.5，zcode.cjs --version 冒烟通过 0.16.5；用户已拍板采用"
  - title: "AgentWatcher 自建 app-server 协议宿主"
    state: "rejected"
    reason: "NDJSON stdio 协议可行（社区 zcode-open-bridge/Multica 佐证）但需自建聊天 UI 并处理 interaction/requestPermission 反向请求，0.16 有信封变更史；留作 Future"
decision: "采用 CLI TUI resume：node zcode.cjs --resume <sessionId> --cwd <workspace>，新终端窗口打开；桌面工作区级跳转不做，无降级"
decision_reason: "桌面无会话深链（实测 handleDeepLink 仅 workspace/oauth/payment 三路由，运行时 18 进程零监听；zai-org/feedback#465 Open 无响应）；远控 relay 无文档仅 hack；协议宿主需自建 UI+反向请求处理留 Future；TUI resume 为官方参数、与桌面共用 db.sqlite、node v24 冒烟通过。用户已拍板：接受 TUI，桌面工作区做不了就不做"
resolution_summary: "四路线评估完毕：桌面深链/远控 relay/协议宿主 rejected，CLI TUI resume adopted。证据：app.asar 静态分析（路由枚举+execute-desktop-command 枚举无 open-session）、netstat 零监听、zcode.cjs 逆向（--resume 与桌面同一 app 工厂与 session store）、node zcode.cjs --version 冒烟 0.16.5、社区佐证（Multica/zcode-open-bridge 均走 CLI 运行时）。已知限制：CLI 会话不出现在桌面侧边栏（feedback#617）；与桌面并行开同一会话无互斥，AgentWatcher 需在 running 状态点击时警示"
resolution_artifact_ids: []
evidence_artifact_ids: []
source_fog_id: "wt_01M2F27YF271NY2FR7ZY2G01KC"
human_confirmation_artifact_id: null
blocked_by: []
reopened_from: null
assignee: "zcode-main"
claim_session: "sess_493c0cf7-8dda-4a39-9b91-53c9b3de5cc1"
claim_issued_by_session: null
claim_token_digest: "b8bc25bad3cbaf30dceed6eb7788d65740d533faeeb68d535b7ebf6c75cf156e"
claimed_at: "2026-09-14T04:19:34Z"
resolved_session: "sess_493c0cf7-8dda-4a39-9b91-53c9b3de5cc1"
resolved_at: "2026-09-14T04:19:44Z"
out_of_scope_reason: null
withdrawn_reason: null
invalidated_by: null
domain_id: null
---

# ZCode 卡片点击跳转的可行路线

## 问题

外部程序能否让 AgentWatcher 点击卡片后恢复 ZCode 的具体会话（sess_<id>）？桌面深链 / 远控 relay / CLI TUI resume / app-server 协议宿主四条路线哪条可行？
