---
schema_version: "1"
protocol: "1.3.0"
artifact: "plan"
artifact_id: "ar_01M65T8VW5000BYD06Q1J5E6JD"
work_item_id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
created_at: "2026-10-30T07:05:00Z"
producer: "aes-plan"
supersedes: "ar_03QZ5EWXZXV89C2WW64GEZ7501"
dependencies:
  work_item_contract_digest: sha256:2b1c17e9e8d949086387fef35bc752d834b0026f368ec9f17c071b583ecf4d0a
  artifacts:
    - artifact_id: "ar_01M65T78VY000B46E2GATW3KB5"
      digest: sha256:660683e9466f4ce8ab3677e8ae53acd0caf715e8ee3c0684f1bc7279b2d2f66e
      locator: "design/design-v2.md"
result: "ready"
---
# 施工计划 v2（第二版设计）

对着设计 v2（ar_01M65T78VY000B46E2GATW3KB5）拆，第一轮已实现的部分（状态检测、上下文、提示词、解析、发送通道、护栏、审计）复用。

## S1 移除设置面与外部依赖

- Cargo.toml 删 ureq 与 tauri-plugin-global-shortcut；lib.rs 删插件注册、call_suggestion_model、SuggestionSettings 的接口字段（保留自动作答常量）、set_suggestion_settings 与热键；ui/index.html 删设置分组九项、相关词条与推送逻辑。验证：cargo build 与 npm run build:ui 通过。

## S2 生成改走 ZCode 无头会话

- lib.rs 新增 generate_suggestion_via_zcode：解析 ZCode.exe（复用现有反解），以 `-p <提示词> --cwd <scratch> --json` 运行 zcode.cjs，180 秒超时，从 --json 信封的 response 字段取建议文本；scratch 目录 %TEMP%\\AgentWatcher\\suggestion-scratch。扫描触发与「换一个」都走它。验证：zcode_ 系测试全绿（参数组装断言扩展进契约测试）。

## S3 右键菜单与幽灵行交互

- ui/index.html：ZCode 卡片右键菜单加「换一条建议」「自动作答此会话的选项（勾选态）」；建议事件点亮幽灵行/选项芯片与 复制/发送/换一个（沿用第一轮实现）。验证：build:ui 通过。

## S4 按会话自动作答开关

- lib.rs：runtime 加 per-session auto 开关与 set_zcode_auto_answer 命令注册；自动路径（waiting+选项+开关+护栏）触发生成与发送。验证：护栏测试绿。

## S5 流程测试改造

- tools/tauri-realtest/cli.mjs：zcode-session 的 AI 建议子步骤改为右键断言（菜单两项存在、设置面板无 AI 分组），有等待会话时点「换一条建议」并放宽轮询到 120 秒（无头生成慢），无等待会话记原因跳过。验证：npm run test:tauri:flow -- zcode-session 通过。

## S6 回归与记录

- cargo test 全量、node --check、git diff --check；AGENTS.md 口径改零配置；implementation/implementation-v2、code-review 新版、validation 重做、manual-test 更新、summary-v2。
