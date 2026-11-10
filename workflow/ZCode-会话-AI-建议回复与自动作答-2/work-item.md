---
schema_version: "1"
protocol: "1.3.0"
id: "wi_11ZFQFVVKZZ1YM09XRNARWNPBQ"
short_id: "77c1x356"
title: "ZCode 会话 AI 建议回复与自动作答"
status: "active"
kind: "feature"
created_at: "2026-10-30T03:43:53.470901600+00:00"
home_repository: "file://F:\\AiProject\\AgentWatcher"
branch_or_pr: "feature/ZCode-会话-AI-建议回复与自动作答-2"
---
# ZCode 会话 AI 建议回复与自动作答

## 原始请求

用户原话（2026-10-30）："先拿 AgentWatcher 跑通给 ZCode 实现 AI 建议回复，并跑通到能全自动化回复的程度，是否可以"，随后要求"按流程给设计、施工计划再施工"。

前置调查（2026-10-30，本机已实测）：ZCode 桌面 3.16.3 自带 CLI（`C:\Program Files\ZCode\resources\glm\zcode.cjs`，版本 0.17.0，`ELECTRON_RUN_AS_NODE=1` 经 ZCode.exe 以纯 node 运行），支持 `--resume <sess_…> -p "<文本>" --cwd <目录> --json`；已用临时会话验证 headless 建会话与续话（同一 sessionId，上下文连续）。AgentWatcher 与桌面版共用 `~/.zcode/cli/db/db.sqlite`，扫描拿到的会话 ID 即 CLI 的 sessionId。旧结论"ZCode 无 CLI、跳转不实现"基于 3.11/3.12 时代，已过时。

## 目标

AgentWatcher 对 ZCode 会话提供"AI 建议下一句回复"：会话状态转换触发建议生成，卡片以幽灵行与 AskUserQuestion 选项芯片展示，一键或全局热键经 zcode.cjs headless 通道发送到原会话；并提供默认关闭的全自动选项作答（带预算、熔断与审计护栏）。

## 范围

做：

- 建议生成管线（`src-tauri/src/lib.rs` 通用层）：ZCode 会话进入 waiting 或 running→idle 转换时触发；从 db.sqlite 提取该会话最后一轮上下文与 AskUserQuestion 选项；调用可配置的模型 API，用改造自 ChatGPT Composer Predictions v5 的提示词生成一条不超过 120 字的中文建议或选项预测；建议绑定会话指纹（最后消息 id），会话前进即作废。
- 发送通道：新增 Tauri 命令，从注册表 `HKCR\zcode\shell\open\command` 反解 ZCode.exe 路径，以 `ELECTRON_RUN_AS_NODE=1` 拉起 `zcode.cjs --resume <会话ID> --cwd <工作区> -p "<建议>" --json`，`CREATE_NO_WINDOW`、带超时与明确报错。
- 卡片 UI（`ui/index.html`）：waiting 卡片显示建议幽灵行与操作（复制/发送/换一个）；检测到结构化 AskUserQuestion 时升级为选项芯片；中英文、dark/light 四象限。
- 全局热键：接受最近一次变化会话的建议，默认动作为复制到剪贴板并 toast，可在设置中改为直接发送。
- 全自动档（默认关）：仅对 waiting 且结构化 AskUserQuestion 的会话自动作答；预算（每会话每小时上限）、同一答案连续两次即熔断该会话、全局开关、审计日志（JSONL 落 `%TEMP%\AgentWatcher\` 下）。
- 设置区块：建议开关、模型配置、热键与全自动档设置，中英文齐备。
- AGENTS.md 同步 ZCode 分支的新能力口径。

不做：

- 其他 provider 的建议生成与发送（管线留通用接口，本任务只接 ZCode）。
- 自由文本的全自动发送，任何情况下都不做。
- ZCode 点击跳转的恢复（独立任务）。
- 不做插件化封装；生成器放宿主通用层，ZCode.exe 路径注册表反解，不硬编码。

## 强约束

- 建议与选项正文只经 Tauri 事件传递，不写 localStorage（沿用 Session Preview 隐私规则）。
- 全自动档默认关闭；自动作答的动作空间只能是会话里 AskUserQuestion 给出的选项集。
- 发送子进程必须 `CREATE_NO_WINDOW` 且有超时；失败给用户明确报错，不静默。
- 中英文、dark/light 同步；现有 ZCode 扫描、悬浮预览、接续导出行为不回退。
- 模型 API 调用失败或未配置时降级为"不出建议"，不影响扫描主流程。

## 验收条件

- AC-001: 建议生成管线有单元测试锁定：状态转换触发判定、会话指纹防陈旧、输出 JSON 契约（suggestion 可为 null）、提示词组装含长度上限与"点名具体对象"的反泛化要求。Verify: case:zcode_suggestion_pipeline_contract
- AC-002: waiting 的 ZCode 卡片显示建议幽灵行；结构化 AskUserQuestion 场景显示选项芯片；中英文、dark/light 四象限样式正常。Verify: manual:npm run test:tauri:flow -- zcode-session 新增建议展示步骤通过（真实 Tauri 壳，window.__TAURI_INTERNALS__ === true）
- AC-003: 点「发送」后目标 ZCode 会话出现该建议的新用户轮，卡片状态随之流转；ZCode 未安装或发送失败时 toast 明确报错。Verify: manual:对真实 ZCode 临时会话完成一次一键发送往返
- AC-004: 全局热键在 AgentWatcher 未聚焦时可用，接受最近一次变化会话的建议并按设置执行复制或发送。Verify: manual:热键在聚焦其他应用时按一次验证复制动作
- AC-005: 全自动档默认关闭；开启后仅对结构化 AskUserQuestion 自动作答，预算、同答案连续两次熔断、审计日志行为有单元测试。Verify: case:zcode_auto_answer_guardrails
- AC-006: 现有 ZCode 扫描、悬浮预览、接续导出全部行为不回退（现有测试全绿）；建议正文不写 localStorage 有代码级断言。Verify: case:cargo_test_full_suite
- AC-007: 设置区块含建议开关、模型配置、热键、全自动档四项，中英文齐备并持久化，重启后生效。Verify: manual:设置页中英文各改一项并重启验证生效
