---
schema_version: 1
protocol: 1.3.0
artifact: design
artifact_id: ar_01M71QRR9M000EGGH4EF73F7DT
work_item_id: wi_01M71Q3R0W0004MNNQHHS0ZSZ1
created_at: 2026-11-10T11:26:30+08:00
producer: aes-brainstorm
result: accepted
supersedes: null
dependencies:
  work_item_contract_digest: sha256:08218fcaad2d9ec21a92a9335bb0cea91f075df699314c2971d7d17dfd158939
  artifacts: []
---

# 设计：Codex 生图 CLI 命令

## 要解决什么

用户要一条所有 AI 助手都能调的生图命令，且生图模型不随 CLI 版本绑死。两个事实性疑问已由实测回答：

1. **生图与编排模型无关。** `codex exec` 的编排模型（gpt 6/6.1 系列）只负责读 imagegen skill、组织提示词、调用内置 `image_gen` 工具；实际生图在 Codex 云端完成，生图模型由云端自动选择。任何能跑 `agentwatcher-cli` 的 agent 都能用这条命令，不需要自己有生图工具。
2. **不绑版本的达成方式是通道选择，不是版本协商。** 走 codex exec 原生通道，CLI 从头到尾不出现生图模型名；云端升级生图模型时命令行为自动跟随。"永远用最新的"由 Codex 云端保证。

## 实测依据（2026-11-10，codex-cli 0.167.0）

| 验证过的事实 | 证据 |
| --- | --- |
| `codex exec --json --ephemeral --skip-git-repo-check -s workspace-write -C <scratch> -` 可触发内置 `image_gen` 工具 | 实跑一次生图成功，无需 `OPENAI_API_KEY` |
| 事件流首条是 `{"type":"thread.started","thread_id":"…"}` | stdout 实录 |
| 产物落 `%USERPROFILE%\.codex\generated_images\<thread_id>\exec-<uuid>.png` | 目录与文件实测（923 KB PNG） |
| 编排模型自己会读 `%USERPROFILE%\.codex\skills\.system\imagegen\SKILL.md` | 事件流里第一条 `command_execution` 就是 Get-Content 该文件 |

第四条决定双来源的分工：系统 skill 在场时 CLI 不注入任何说明，Codex 自己读最新的；skill 缺失时 CLI 才把内嵌固化副本附进 prompt。

## 方案对照

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| codex exec 原生通道 | 组装提示词后经 codex exec 触发内置 image_gen 工具，产物从 generated_images 复制到 --out | 依赖本机 codex 登录态；单次生图走完整 turn，比直连 API 慢 | 采用：用户拍板全用原生 codex 能力；生图模型自动跟随云端最新，天然不绑版本 |
| Images API 直连（image_gen.py 模式） | CLI 内置脚本或 http 调 /v1/images/generations | 需要 `OPENAI_API_KEY` 且要写死模型名，违背"不绑版本" | 不采用 |
| 复用建议专用常驻 app-server 的 thread 通道 | 走 thread 让模型调工具 | sample 是只读采样档，工具调用行为不受控；常驻 server 是建议专用档不宜混用 | 不采用 |
| 提示词模板只内嵌固化副本 | 二进制里写死一份 | skill 更新后 CLI 行为不跟随 | 不采用；双来源：系统 skill 优先、内嵌兜底 |

## 关键机制

提示词组装与产物搬运的流程：

1. `system_imagegen_skill_path()`：`CODEX_HOME` 优先，否则 `%USERPROFILE%\.codex`，拼 `skills\.system\imagegen\SKILL.md`；存在 → 来源 `system-skill`，prompt 只含用户提示词与一段固定短指令。
2. 不存在 → 来源 `embedded`，prompt 前置 `include_str!` 内嵌的固化副本（自 2026-11-10 本机版逐字节复制）。
3. `imagegen_exec_args`：沿用建议管线的 flag 集（`--json --ephemeral --skip-git-repo-check -C <scratch>` + stdin `-`），不传 `-m`、不传 effort。沙箱档在施工第一步实测 read-only 可用性后定。
4. exec 退出后从事件流提 `thread_id`，在 `<codex_home>\generated_images\<thread_id>\` 取 mtime 最新的 PNG，复制（非移动）到 `--out`（默认 `%TEMP%\AgentWatcher\imagegen\`）。

错误分类：`codex_unavailable`（CLI/node 缺失）、`codex_timeout`、`generation_failed`（退出码非 0 / 无产物 / 未登录态，detail 带区分线索）。未登录能否独立成 `codex_auth_failed` 看施工第一步实测的输出形态。

## 放弃了什么

- 放弃 `--model`、`--quality`、`--size` 之类生图参数：这些是 gpt-image 系 API 的参数面，走 codex 通道时编排模型+skill 自己决定；暴露它们就是把版本绑回 CLI。
- 放弃 edit/batch：单一 generate 先跑通，命令面留扩展空间。
- 放弃 GUI 集成：本任务只做 CLI；卡片上"生成图片"入口是后续任务的事。
