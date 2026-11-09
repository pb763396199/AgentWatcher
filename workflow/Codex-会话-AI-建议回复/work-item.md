---
schema_version: "1"
protocol: "1.3.0"
id: "wi_01M6JVHHKF000D34EX7V9M74VE"
short_id: "598bcdg3"
title: "Codex 会话 AI 建议回复"
status: "active"
kind: "feature"
created_at: "2026-11-04T09:00:00.000000000+00:00"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
branch_or_pr: "feature/Codex-会话-AI-建议回复"
---
# Codex 会话 AI 建议回复

## 原始请求

用户 2026-11-04 拍板："先保 CLI 通道，该提交的提交。然后直接去做 codex 的"。ZCode 建议功能已合入 dev（任务 38rcwjdf），复用同一套架构给 Codex 会话。

## 目标

Codex 会话的悬浮预览窗显示 AI 建议回复输入框，与 ZCode 建议功能同构：悬停出预览窗、输入框常驻、AI 建议可生成并填入、点发送经 Codex CLI 送回原会话。

## 范围

做：

- Codex 会话的预览窗回复输入框（复用 ZCode 的 UI 组件，按 provider 切换显示）
- Codex 发送通道：`codex exec resume <session> "text" --json`（CLI 公开可用）
- Codex 建议：扫描后对**刚跑完一轮（running/waiting → idle）**与**新进入 waiting** 的 Codex 会话生成（v5 改造提示词，上下文从 JSONL 尾部提取，保证含 user 轮）
- 触发语义对齐官方：live（转 idle，采样 1.0）+ catch_up（会话重新出现，采样 0.5）+ waiting（等审批，不采样）；冷启动/陈旧 idle 不触发
- 生成通道：`codex exec --json --ephemeral --skip-git-repo-check -s read-only -C <scratch> -o <out> -`，prompt 走 stdin；解析读 `-o` 文件 + JSONL `agent_message` 兜底
- 会话 ID 归一化：`codex:<uuid>` / `codex://threads/<uuid>` / 裸 uuid 统一成一个 UUID
- 护栏：资格校验（plan / 非 openai provider / 模型黑名单）、45 秒超时、确定性采样、指纹防陈旧
- CLI 可观测入口：`agentwatcher-cli suggest generate|prompt <id>`
- AGENTS.md 补 Codex 建议口径

不做：

- 不改 ZCode 建议功能（已合入）
- 已接 `thread/sample` 官方预测引擎（2026-11-09 落地，见下节）
- 不做自动作答（Codex 的 AskUserQuestion 检测已有，护栏逻辑复用）

### thread/sample 通道（2026-11-09 落地）

- 入口：`suggest generate --engine auto|sample|exec`，默认 `auto`。
- 流程：`thread/read`（只读，绕开桌面端的 writer lock）→ `thread/fork` 持久副本 → `thread/sample {expectedTurnId, prompt, disableMcp}` → `thread/archive` 收尾。
- 传输：同一套四步流程跑在两种通道上——优先连**建议专用常驻 app-server**（`--listen ws://127.0.0.1:<port>`，启动参数带 `-c service_tier=priority` 与 `-c mcp_servers={}`），起不来再退回一次性 stdio 进程。单独开一个 server 是为了不让 priority 档波及 `launch_codex_handoff` 的普通 turn 计费。
- 前置：app-server 的 `initialize` 必须带 `capabilities.experimentalApi: true`，否则报 `requires experimentalApi capability`。
- 选路：rollout ≤ 1MB 走 sample，超过走 exec。sample 要全量 prefill，耗时随会话线性增长（622KB≈12.5s，4.3MB≈27s）。
- 实测端到端（2026-11-09 二次自检，622KB 会话）：同一 server 连续采样，`sample` 阶段 priority 6 次为 8.7/5.2/4.6/8.0/8.1/7.1s（均值 7.0s），default 6 次为 11.7/7.8/7.2/10.6/6.1/6.7s（均值 8.4s），priority 约快 17%、方向一致但样本小；`fork` 阶段 0.2–6.7s 是最大噪声源（要写一份 rollout 副本）；server 启动到 `initialize` 0.3s（关 MCP 后），偶发 4.7s。CLI 单次端到端 11.7–13.5s（每进程都要重付 server 启动与冷 fork）。
- 档位语义：`thread/sample` **不接受** `serviceTier` 字段，模型与推理档完全继承 fork 出的父线程。所以 `--model` / `--effort` 对 sample 无效，CLI 显式传时会提示；审计里档位记 `inherited` 而不是 CLI 入参。
- 取证方法坑：codex 的 serde **不拒绝未知字段**（实测塞 `zzzProbeField` 无任何报错）。因此「传了某字段没报错」不能推断字段存在，只能从二进制里的字段名串确认；`ThreadSampleParams` 的 9 个字段表里没有 `serviceTier` / `effort`。
- 限制：`thread/prediction/request` 仍是 stub（`not implemented yet`）；ephemeral fork 过不了 `persistent root thread` 校验；`thread/resume` 被桌面端 `thread-writer-locks` 独占，必然 `os error 5`。

### 与官方流程的有意偏离（2026-11-09 记录）

| 维度 | 官方 Composer Predictions | 本实现 | 代价 |
|---|---|---|---|
| 生成引擎 | app-server `thread/sample` 临时分叉，继承完整压缩历史 + 同款模型/档位 | Codex 小会话走 `thread/sample`（fork 副本）；大会话与 WorkBuddy 走 `codex exec` | 大会话上下文缩水为尾部 4 轮 |
| 成本 | beta 期不计用量 | 真实计费调用 | 每次生成产生 token 用量；sample 送全量历史，exec 只送尾部 4 轮 |
| 超时 | 30 秒硬超时 | 45 秒 | — |
| 上下文 | 完整线程 + 压缩历史 | 尾部 4 轮 × 300 字，至少含 1 条 user | 长会话建议质量下降 |
| 安全过滤 | mod-policy + 轮次 moderation | 仅本地资格校验 | 无服务端安全过滤 |
| 交互 | 幽灵文本 + Tab 接受为草稿，绝不自动发送 | 填入输入框卡片 | 更主动，需用户自己按发送 |

thread/sample 通道已于 2026-11-09 落地（见上节），上下文与档位两项已补齐；成本与安全过滤仍依赖服务端，未对齐。

## 强约束

- 预览窗 UI 按 provider 路由：ZCode 卡片显示 ZCode 建议，Codex 卡片显示 Codex 建议
- 发送走 `codex exec resume`（公开 CLI，无授权墙）
- 全量测试不回退

## 验收条件

- AC-001: Codex 会话悬停预览窗出现回复输入框，打字+发送可用。
  Verify: manual:Codex 会话悬停预览窗测试
- AC-002: Codex 会话跑完一轮转 idle（或新进入 waiting）时自动生成建议并填入输入框。
  Verify: manual:Codex 会话 idle/waiting 建议测试
- AC-005: `agentwatcher-cli suggest prompt <id>` 输出的提示词与官方 v5 契约一致；`suggest generate <id>` 能真实产出建议。
  Verify: case:suggest_prompt_contract_and_generate
- AC-003: 发送后消息落库，模型回复出现。
  Verify: manual:对真实 Codex 会话完成一次往返
- AC-004: 全量测试绿。
  Verify: case:cargo_test_full_suite
