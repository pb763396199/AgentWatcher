---
schema_version: "1"
protocol: "1.3.0"
artifact: "wayfinder"
artifact_id: "ar_01M2F27YF1BYC844M1BSJXVR8K"
work_item_id: "wi_3QQPFJCPXD24P1CCXMXAZ730TQ"
created_at: "2026-09-14T04:19:12Z"
producer: "aes-wayfinder"
state: "handed_off"
dependencies:
  work_item_contract_digest: sha256:efa5a5233be0f6e70b70442ff1fb5cb63dad810669e2356c04643038eb83e195
  artifacts: []
navigation:
  destination: "AgentWatcher 新增第 6 个会话 provider `zcode`（徽标 ZC）：从 ZCode 的本地会话库"
  route: "路线已按用户确认交接：跳转下线，数据面交付并发布 v0.1.5"
  domains: []
  nodes: []
  edges: []
  frontier: []
  tasks: []
  resume:
    session: "ws_pending"
    position: "handoff"
    next: "已交接"
    claimed_tasks: []
  handoff:
    route: "aes-brainstorm"
    confirmation: "confirmed"
    summary: "用户 2026-09-14 确认：提交并收尾、合到 dev、发布 v0.1.5，不提 issue"
---

# WayFinder

## 目的地

AgentWatcher 新增第 6 个会话 provider `zcode`（徽标 ZC）：从 ZCode 的本地会话库

## 说明

- 待补

## 已有决定

- [ZCode TUI 运行时在桌面发行版上的可用性](wayfinder-tickets/ZCode-TUI-运行时在桌面发行版上的可用性.md)：AgentWatcher 落地：open_zcode_session 先探 PATH zcode（SEA 内嵌 TUI），再用 zcode.cjs+node 并预检 node_modules/@zcode/tui/dist/index.js；不可用即报错不 spawn。handoff zcode 模式同路径同边界。flow 测试接受双分支（恢复成功/明确报错）。上游就绪（官方独立 CLI 或深链）后零改动升级
- [ZCode 卡片点击跳转的可行路线](wayfinder-tickets/ZCode-卡片点击跳转的可行路线.md)：四路线评估完毕：桌面深链/远控 relay/协议宿主 rejected，CLI TUI resume adopted。证据：app.asar 静态分析（路由枚举+execute-desktop-command 枚举无 open-session）、netstat 零监听、zcode.cjs 逆向（--resume 与桌面同一 app 工厂与 session store）、node zcode.cjs --version 冒烟 0.16.5、社区佐证（Multica/zcode-open-bridge 均走 CLI 运行时）。已知限制：CLI 会话不出现在桌面侧边栏（feedback#617）；与桌面并行开同一会话无互斥，AgentWatcher 需在 running 状态点击时警示
- [ZCode 卡片跳转是否本期实现](wayfinder-tickets/ZCode-卡片跳转是否本期实现.md)：已落地：Rust 删除 launch_zcode_tui/ZcodeTuiRuntime/find_zcode_* 全套与 handoff zcode 模式，open_zcode_session 返回 not supported yet；前端移除 ZCode TUI 目标选项、并行恢复警示与死键；zcode-session flow 断言该提示；AGENTS/README 口径同步。悬浮预览真机验证通过（工作区/标题/状态/两段正文），上下文导出逐项审计通过

## 尚未明确

## 范围外

- 暂无
