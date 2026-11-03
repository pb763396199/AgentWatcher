---
schema_version: "1"
protocol: "1.3.0"
id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
short_id: "38rcwjdf"
title: "ZCode 会话 AI 建议回复与自动作答"
status: "active"
kind: "feature"
created_at: "2026-10-30T03:43:53.300419300+00:00"
home_repository: "file://F:\\AiProject\\AgentWatcher"
branch_or_pr: "feature/ZCode-会话-AI-建议回复与自动作答"
---
# ZCode 会话 AI 建议回复与自动作答

## 原始请求

用户原话（2026-10-30）："先拿 AgentWatcher 跑通给 ZCode 实现 AI 建议回复，并跑通到能全自动化回复的程度，是否可以"，随后要求"按流程给设计、施工计划再施工"。

前置调查（2026-10-30，本机已实测）：ZCode 桌面 3.16.3 自带 CLI（`C:\Program Files\ZCode\resources\glm\zcode.cjs`，版本 0.17.0，`ELECTRON_RUN_AS_NODE=1` 经 ZCode.exe 以纯 node 运行），支持 `--resume <sess_…> -p "<文本>" --cwd <目录> --json`；已用临时会话验证 headless 建会话与续话（同一 sessionId，上下文连续）。AgentWatcher 与桌面版共用 `~/.zcode/cli/db/db.sqlite`，扫描拿到的会话 ID 即 CLI 的 sessionId。旧结论"ZCode 无 CLI、跳转不实现"基于 3.11/3.12 时代，已过时。

## 目标

AgentWatcher 对 ZCode 会话提供"AI 建议下一句回复"：会话状态转换触发建议生成，卡片以幽灵行与 AskUserQuestion 选项芯片展示，一键或全局热键经 zcode.cjs headless 通道发送到原会话；并提供默认关闭的全自动选项作答（带预算、熔断与审计护栏）。

## 范围

做（2026-10-30 第二版设计拍板：零配置，复用会话自己的引擎）：

- 生成走 ZCode 本身：`ELECTRON_RUN_AS_NODE=1` 运行 `zcode.cjs -p "<提示词+上下文>" --cwd %TEMP%\\AgentWatcher\\suggestion-scratch --json`，模型与账号沿用用户 ZCode 现有配置，AgentWatcher 不存任何模型接口配置。
- ZCode 会话新进入 waiting 自动生成建议（改造自 Composer Predictions v5 的提示词，一条不超过 120 字）；卡片右键可「换一条建议」。
- 建议经事件点亮卡片幽灵行；结构化 AskUserQuestion 场景为选项芯片；动作 复制/发送/换一个；发送沿用 zcode.cjs --resume 通道。
- 全自动作答按会话开关（右键「自动作答此会话的选项」），仅选项域，护栏常量内置：每小时 6 次、同答案连续两次熔断、审计落 %TEMP%\\AgentWatcher\\auto-answers\\audit.jsonl。

不做：

- 设置面板不做任何 AI 建议相关参数（第一版的外部模型 API、ureq、全局热键、set_suggestion_settings 全部移除）。
- 不给其他 provider 出建议；不做 ZCode 点击跳转；不清理 ZCode 会话库里的 scratch 会话（等上游能力）。

## 强约束

- 零配置：功能不引入任何需要用户填写的参数；模型、账号全部来自 ZCode 自身。
- 建议与选项正文只经 Tauri 事件传递，不写 localStorage。
- 自动作答的动作空间只能是会话里 AskUserQuestion 给出的选项集；默认不开。
- 发送与生成子进程必须 CREATE_NO_WINDOW 且有超时；失败给用户明确报错。
- 中英文、dark/light 同步；现有 ZCode 扫描、悬浮预览、接续导出行为不回退。
- scratch 会话目录固定在 %TEMP%\\AgentWatcher\\suggestion-scratch（默认工作区黑名单已挡）。

## 验收条件

- AC-001: 建议管线契约有单元测试：状态触发判定、指纹防陈旧、提示词组装（反泛化与 120 字上限、待选项约束）、输出 JSON 解析（含宽容围栏）、无头生成参数组装（-p 与 --cwd 指向 scratch 目录）、护栏判定。
  Verify: case:zcode_suggestion_pipeline_contract
- AC-002: ZCode waiting 卡片右键出现「换一条建议」与「自动作答此会话的选项」；建议生成后卡片显示幽灵行或选项芯片；中英文、dark/light 四象限正常；设置面板没有任何 AI 建议相关分组。
  Verify: manual:npm run test:tauri:flow -- zcode-session 右键与建议步骤通过后，人工四象限确认
- AC-003: 点「发送」后目标 ZCode 会话出现该建议的新用户轮，卡片状态流转；ZCode 未安装或发送失败时 toast 明确报错。
  Verify: manual:对真实 ZCode 临时会话完成一次一键发送往返
- AC-004: 全自动护栏有单元测试：每小时预算、连续同答案熔断、等待时长、选项命中、按会话开关状态、审计行格式。
  Verify: case:zcode_auto_answer_guardrails
- AC-005: 现有 ZCode 扫描、悬浮预览、接续导出全部行为不回退（全量测试绿）；建议正文不写 localStorage。
  Verify: case:cargo_test_full_suite
- AC-006: 右键对某会话打开自动作答后，该会话带选项的等待被自动作答一次，审计文件出现对应记录；关闭开关后不再自动作答。
  Verify: manual:制造一个带选项的等待会话验证自动作答与审计
