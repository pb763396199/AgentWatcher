---
schema_version: "1"
protocol: "1.3.0"
id: "wi_01M71Q3R0W0004MNNQHHS0ZSZ1"
short_id: "mbvtfbzr"
title: "Codex 生图 CLI 命令"
status: "in_review"
kind: "feature"
created_at: "2026-11-10T11:12:58+08:00"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
branch_or_pr: "feature/Codex-生图-CLI-命令"
---

# Codex 生图 CLI 命令

## 原始请求

用户原话（2026-11-10）："将codex的image gen能力包装成AgentWatcher的cli的命令吧，你看正在开发的其他会话，在做利用codex的ai自动回复能力，image gen是否也能包成所有agent都能调的能力，全都用原生codex能力来做。但我不太确定是，image gen的调用是否跟gpt模型有关（我指的不是gpt-image系列模型，而是gpt 6 或6.1系列模型），还是说可以完全由各自agent组织prompt后调image gen技能就行了。而且我希望的是，这个cli命令不跟image gen具体版本绑定，永远用最新的chatgpt桌面端的那个最新的image gen，没有的话才使用cli里默认带的，也就是先从当前电脑的最新版固化一份"

事实核查（2026-11-10，已实测）：`codex exec`（0.167.0，账号登录态）可触发内置 `image_gen` 工具生图，不需要 `OPENAI_API_KEY`；产物落在 `%USERPROFILE%\.codex\generated_images\<thread_id>\exec-<uuid>.png`。生图与编排模型（gpt 6/6.1 系列）无关：编排模型只负责读 skill、组织提示词、调工具；实际生图由 Codex 云端 image_gen 工具完成，生图模型由云端自动选择，客户端不指定、也不需要指定。

## 目标

agentwatcher-cli 新增 `imagegen generate` 命令：输入提示词，经 codex exec 原生通道生图，把产物落到调用方指定路径。任何 AI 助手（ZCode / Claude Code / Codex / WorkBuddy / Copilot 等）都能通过这条命令获得生图能力，不依赖各自宿主有没有生图工具。

## 范围

做：

- `imagegen generate <提示词>` 子命令：prompt 走参数，`--out` 控制产物路径，`--timeout` 控制超时；复用建议管线已验证的 codex exec 封装模式（stdin 传 prompt、`--json` 事件流解析）。
- 提示词模板双来源：优先读当前电脑 `%USERPROFILE%\.codex\skills\.system\imagegen\SKILL.md`（尊重 `CODEX_HOME`）的最新版；该文件不存在时，用 CLI 二进制内嵌的固化副本（自当前 2026-11 最新版 skill 固化，include_str! 内嵌，同 `skill install` 的既有做法）。
- 产物定位与搬运：从 `~/.codex/generated_images/<thread_id>/` 找到本次 exec 产出的图片，复制（非移动）到 `--out`。
- 信封输出与既有契约一致，data 含最终落盘路径、engine 标签（`codex/imagegen`）、提示词模板来源（`system-skill` / `embedded`）。

不做：

- 不做图片编辑（edit）、批量（batch）、透明通道特控——后续任务再扩。
- 不做 `OPENAI_API_KEY` 的 Images API fallback：用户拍板"全都用原生 codex 能力"，codex 不可用时分类报错并提示，不静默换通道。
- 不动 GUI：本任务只动 `src-tauri/src/bin/agentwatcher-cli/` 与必要的 lib 侧 pub 辅助函数。

## 强约束

- 命令面与实现代码不出现具体生图模型版本号（gpt-image-1/1.5/2 等），不提供 `--model` 类参数；生图模型由 Codex 云端自动选择。固化副本内 skill 原文照抄不改写（其中出现的版本号属于引用原文，不违规）。
- `imagegen generate` 是真实产生模型用量的 CLI 命令（与 `suggest generate` 同级），帮助文本与 README 口径写明。
- 信封契约不破：一条命令一个信封、退出码 0/1/2、失败也走信封。
- codex CLI 缺失 / 未登录时不得挂起：分类报错，退出码 1。

## 验收条件

- AC-001: `agentwatcher-cli imagegen generate` 真实生图成功：信封 ok=true，data 含最终落盘路径且文件存在（PNG 可读），engine 标注 codex 通道。
  Verify: manual:在本机跑一次真实生图，核对落盘文件与信封字段
- AC-002: 提示词模板双来源闭环：系统 imagegen skill 存在时信封标注 `system-skill`；把 `%USERPROFILE%\.codex\skills\.system\imagegen\SKILL.md` 临时改名后重跑同命令，信封标注 `embedded` 且仍能生图成功。
  Verify: manual:改名 skill 后重跑命令，核对 data 的模板来源字段与产物
- AC-003: 实现代码（cli/commands/output 与新增模块）中除内嵌固化副本原文外不出现生图模型版本号；命令面不含 --model 类参数。
  Verify: case:imagegen_exec_args_flags_and_no_model_pinning
- AC-004: 失败路径分类明确：codex 不在 PATH、未登录（`CODEX_HOME` 指向空目录模拟）、超时三种场景信封 ok=false 且 error 可区分，退出码 1，stderr 有分类日志。
  Verify: manual:三种失败场景各跑一次（PATH 缺 codex 模拟见 validation、CODEX_HOME 指空目录、--timeout 5 逼超时），核对信封 error 可区分与退出码 1
- AC-005: `cargo test --manifest-path src-tauri/Cargo.toml --test cli_taxonomy --test cli_contract` 全过，帮助文本含 imagegen 条目，信封契约断言覆盖新命令。
  Verify: case:json_imagegen_empty_prompt_fails_in_envelope
