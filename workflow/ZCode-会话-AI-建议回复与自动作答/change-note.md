---
schema_version: "1"
protocol: "1.3.0"
artifact: "change-note"
artifact_id: "ar_67MYZZEZZQBVK4CXBYSV07YYR7"
work_item_id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
created_at: "2026-10-30T04:28:58Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:2b1c17e9e8d949086387fef35bc752d834b0026f368ec9f17c071b583ecf4d0a
  artifacts:
    - artifact_id: "ar_03QZ5EWXZXV89C2WW64GEZ7501"
      digest: sha256:6f320afb452f37b18a991983c2319e92cdf42999334d362987917a173f01ba1f
      locator: "plan.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "d0e0d7ab880c95fb8bdd3d525813057fe2e74c32"
    revision: "d0e0d7ab880c95fb8bdd3d525813057fe2e74c32"
    tree: "0b9851b68f33b1ef0a92d7ade6e09528c8ab0fcd"
    content_digest: "sha256:66e68d9e461b950bf4d3ba1765215aa70d2cd2b2c5fd2db4c7ddeac2e7f57d91"
    branch_or_pr: "feature/ZCode-会话-AI-建议回复与自动作答"
    workflow_excluded: true
result: "complete"
---
# 修改说明

## 覆盖账本

本任务的施工提交（feature/ZCode-会话-AI-建议回复与自动作答，基线 d4b38a6 起）：

- 0b3b65e build(zcode-suggest): 引入 ureq、winreg 与全局热键插件依赖
- 0b7dd5f test(zcode-suggest): 先写建议管线与全自动护栏的契约测试
- 31e2a2a 之前的实现提交 feat(zcode-suggest): 实现建议管线核心判定与提示词组装
- feat(zcode-suggest): 扫描后对新进入等待的 ZCode 会话生成建议
- feat(zcode-suggest): 新增 send_zcode_message 经 zcode.cjs headless 发送
- feat(zcode-suggest): 全自动选项作答挂护栏与审计落盘
- feat(zcode-suggest): 设置命令、全局热键与换一个重生成
- 31e2a2a feat(zcode-suggest): 卡片建议幽灵行、选项芯片与 AI 建议设置区
- 3c2d51d test(zcode-suggest): zcode-session 流程新增 AI 建议断言与设置推送
- d0e0d7a docs(zcode-suggest): AGENTS.md 补 ZCode 建议与发送口径

## 修改理由

每条对应设计（design.md，ar_0KMYDEZFWFEYQ4A58SPWM0MAV4）与计划（plan.md，S1-S10）的决定：

- 依赖（S1）：ureq 阻塞式跑后台线程不拖扫描；winreg 读注册表反解 ZCode.exe；官方热键插件替代裸 Win32。
- 契约测试先行（S2）：状态触发、提示词五要素、JSON 输出契约、护栏四闸先锁行为再实现。
- 管线核心（S3）：纯函数化判定，网络与扫描解耦。
- 触发挂接（S4）：只挂在 GUI 的 scan_sessions 命令返回后，CLI 共用的 scan_sessions_blocking 保持零副作用。
- 发送通道（S5）：ELECTRON_RUN_AS_NODE 纯 node 模式运行 zcode.cjs --resume -p；CREATE_NO_WINDOW 与 120 秒超时沿用打包 git 锚点子进程的做法。
- 全自动护栏（S6）：动作空间收缩到会话自己的选项集；预算与熔断存内存，审计落 temp。
- 设置与热键（S7）：设置推送时重新注册热键；换一个限制两次。
- 前端（S8）：建议正文只走事件（隐私规则沿用 Session Preview）；设置持久化走既有 agentwatcher.settings。
- 流程测试（S9）：本地假模型服务让真实管线点亮；无等待会话时记录原因跳过，不硬造数据。
- 文档（S10）：AGENTS.md 同步 ZCode 分支新口径。

## 代码范围

- src-tauri/Cargo.toml、Cargo.lock：三个新依赖。
- src-tauri/src/lib.rs：建议管线、发送通道、护栏、热键、四个新命令（send_zcode_message、set_suggestion_settings、regenerate_suggestion 挂 generate_handler）、单元测试五组。
- ui/index.html：设置分组九项、卡片幽灵行与选项芯片、两个事件监听、中英文词条、深浅色样式。
- tools/tauri-realtest/cli.mjs：zcode-session 流程新增 AI 建议子步骤与本地假模型服务。
- AGENTS.md：ZCode 分支口径。

## 未解释或例外改动

- 计划 S1 写了往 capabilities/default.json 加 global-shortcut 权限，实际没加：热键在 Rust 侧注册，不经 webview 命令通道，能力声明没有作用点。
- set_suggestion_settings 会清空后端状态快照（计划外补充）：不清的话，已经等着的会话要等状态再次变化才出建议，开启功能的第一体验是"什么都不发生"。
- 热键动作实现为单个"热键直接发送"开关而不是计划里的多选：两个取值一个开关就够，少一组 UI 与词条。
