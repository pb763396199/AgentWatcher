---
schema_version: "1"
protocol: "1.3.0"
artifact: "summary"
artifact_id: "ar_5QMVXKFENY8J7Z4AWTTG48KKPM"
work_item_id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
created_at: "2026-09-16T07:17:17Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:32abb9b77345e1c4412d7442f9ae353d1bca154ec66037eeaf403b276872b2ea
  artifacts:
    - artifact_id: "ar_17QYMV7XZVJT2YMZ97TDYEPB8T"
      digest: sha256:eaf59cad40d573697ce8a9b1016ce324caca359c8f482ca58cce821b1de91ab5
      locator: "implementation.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "de5a9813378a676dde2ffd2ad67c8ad5d995eda4"
    revision: "de5a9813378a676dde2ffd2ad67c8ad5d995eda4"
    tree: "598fe1d1a4da0bb3667ef0b1a50ad98502bb91bb"
    content_digest: "sha256:71cae4bb63af7db87e355b64ce3d0fa1f62d65a7781abd21f5d487949f26a920"
    branch_or_pr: "feature/agent-cli"
    workflow_excluded: true
result: "complete"
scope: "round"
---
# 本轮成果

## 结论

AgentWatcher 现在有一个给 AI 助手用的命令行：`agentwatcher-cli`。不打开桌面窗口就能查全部六个 provider 的会话列表、详情和用量，导出接续上下文；还能一键把用法技能装进本机 AI 宿主，装完后宿主里的 AI 自己就会用这套命令。输出是稳定的 JSON 信封，AI 用管道捕获不会挂死（为此修掉了一个扫描拉起常驻服务的坑）。

## 你现在能做什么

- 终端里跑 `agentwatcher-cli session list --status waiting`，看哪些会话在等人回复。
- 用列表里的 ID 查详情和用量：`session show <id>`、`session usage <id>`，加 `--content` 取正文视图。
- `handoff export <id>` 导出接续上下文，落盘位置和右键接续面板相同。
- `skill install` 把用法技能装进本机 ZCode / Claude Code 等 AI 宿主，之后宿主里的 AI 直接就会查会话；升级后重跑一次即更新。
- 发布 zip 里多了 `agentwatcher-cli.exe`，AI 助手可直接调用。

## 改了什么

| 变更种类 | 位置 | 锚点 |
| --- | --- | --- |
| 新二进制：命令定义、编排、信封输出 | src-tauri/src/bin/agentwatcher-cli/ | S3/S4 |
| 宿主开放三个查询入口（纯可见性） | src-tauri/src/lib.rs | S2 |
| 修复：CLI 进程不再拉起常驻 Codex 服务，管道读者不再挂死 | src-tauri/src/lib.rs + main.rs | S4 |
| 技能分发：内嵌 SKILL.md + install/list/remove | skill.rs + skill/SKILL.md | S8 |
| 测试：分类学 8 项 + 真实二进制契约 8 项 | src-tauri/tests/cli_*.rs | S3/S4/S8 |
| 发布打包与文档口径 | package-exe.mjs、README、AGENTS | S5/S6 |

## 还欠什么

- 桌面发版流程（package:exe 全量跑、CHANGELOG、SHA256）属于发版任务，本轮只保证脚本路径正确。
- 接续「派发」、运维工具是访谈拍板的后续版本范围；skill 候选宿主表硬编码，新宿主要改代码。
