---
schema_version: "1"
protocol: "1.3.0"
artifact: "summary"
artifact_id: "ar_01M83KQ181000452R9Z8Y1VPP8"
work_item_id: "wi_0ZQ8FFAX6K2B4D8F66T96TFDEM"
created_at: "2026-11-23T07:12:00Z"
producer: "aes-finish"
supersedes: null
scope: work_item
result: "complete"
dependencies:
  work_item_contract_digest: sha256:377edb0dac11131577aa3523826b0b7accc7a012a8b0fef5fe9226ef0433ed54
  artifacts: []
---
# 任务总结：AgentWatcher 支持 Antigravity Provider

## 结论

Antigravity（Google 的智能体开发平台）成为 AgentWatcher 的第 8 个会话来源。
卡片（徽标 AG）、状态（等回复/跑着/闲着）、悬浮预览、用量（轮次/工具/token）、
接续来源导出、跨电脑打包、CLI 查询与技能分发全部可用；点击跳转按官方无深链
的现实返回明确提示。经三轮独立评审终审通过。

## 你现在能做什么

主面板看 AG 卡片并按 provider 筛选；设置面板 Data 组有 Antigravity 开关（默认开）；
悬停看正文预览与用量；右键打包交接或作为接续来源（导出可读 Markdown）；
`agentwatcher-cli session list --provider antigravity` / `show` / `usage`；
`skill install` 覆盖 `~/.gemini/config/skills` 宿主。

## 改了什么

| 变更种类 | 位置 | 锚点 |
| --- | --- | --- |
| 新 provider：protobuf 裸解析、清单扫描、状态机、预览/用量、接续导出、打包 | `src-tauri/src/lib.rs` | S1–S4 / 0a15bfa |
| CLI 参数、技能宿主表与契约断言 | `src-tauri/src/bin/agentwatcher-cli/`、`tests/cli_contract.rs` | S6 / 0a15bfa |
| 界面：设置开关、筛选、徽标、双语言双主题、性能计数 | `ui/index.html` | S5 / 1b81016 |
| 真实交互流程 antigravity-session 与文档口径 | `tools/tauri-realtest/cli.mjs`、`AGENTS.md` 等 | S7–S8 / 1b81016 |
| 评审修复：WorkBuddy 计数与监听回归、解析器健壮性、热路径缓存 | `src-tauri/src/lib.rs` 等 | S9 / ae7e4e8 |
| 评审修复：定向查询列清单对齐共享映射器 + 回归锁 | `src-tauri/src/lib.rs` | S9 / c22e276 |

## 每一轮的去向

第一轮（0a15bfa + 1b81016）完成 S1–S8 全部施工；第二轮（ae7e4e8）修评审发现的
两条 WorkBuddy 回归与降级缺口；第三轮（c22e276）修复审抓到的定向查询列错位。
评审三轮记录在 reviews/code-review/，验收四版收敛于 validation/validation-v4.md。

## 还欠什么

人工清单三条待人核（manual-test.md）：悬浮预览一致性、四组合主题语言、真实
ask_question 场景的等回复判定。token 桶语义与模型枚举名待官方口径（设计记录
未决问题 1、2）。

给人点开的详细证据：F:\AiProject\AgentWatcher\.aes-workflow\worktrees\AgentWatcher-支持-Antigravity-Provider\workflow\AgentWatcher-支持-Antigravity-Provider\implementation.md