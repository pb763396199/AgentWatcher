---
schema_version: "1"
protocol: "1.3.0"
artifact: "manual-test"
artifact_id: "ar_3SYTMKZFXE1PBNKXFCEE2TTV0G"
work_item_id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
created_at: "2026-09-16T07:17:17Z"
producer: "aes-validate"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:32abb9b77345e1c4412d7442f9ae353d1bca154ec66037eeaf403b276872b2ea
  artifacts: []
result: "passed"
---
# 人工核对清单

写给用的人。按顺序做，一条一件事；`[ ]` 没测，`[x]` 过了，`[!]` 没过就在下面缩进写现象。

## 查询命令

- [X] 在终端跑 `agentwatcher-cli --format human session list`，能看到一张表：状态、provider、时间、标题、工作区，表格里没有对话内容。
- [X] 跑 `agentwatcher-cli --format json session list --status waiting`，输出是一个 JSON 对象，里面有 `command`、`ok`、`data.sessions`，每个会话只有元数据字段。
- [X] 从上面输出里挑一个 `id`，跑 `agentwatcher-cli --format json session show <id>`，能看到标题、状态、最后用户输入和 AI 摘要；再加 `--content`，OpenCode/ZCode 会话会多出导出文件路径，其他 provider 多出原始文件路径。
- [X] 跑 `agentwatcher-cli --format json session usage <id>`，能看到 token、轮次、工具调用等用量字段。
- [X] 跑 `agentwatcher-cli --format json handoff export <id>`，输出里的 `primarySourceFile` 指向系统临时目录 `AgentWatcher\handoff-sources\` 下面的 Markdown 文件，文件能打开。
- [X] 故意给一个不存在的 ID（如 `nosuch:000000000000`），输出 `ok:false` 且 `error.code` 是 `session_not_found`，终端退出码是 1（Windows 下看 `$LASTEXITCODE`）。
- [X] 把 GUI 完全关掉再跑一次 `session list`，命令照常出结果（验证不依赖 GUI 在运行）。
- [X] 中文输出正常：错误消息和 human 表格里中文不是乱码。

## 技能安装

- [X] 跑 `agentwatcher-cli skill list`，能看到 zcode / claude-code / agents / codex / opencode 五行状态，本机装过的宿主显示 installed 或 not-installed，没装的显示 host-absent。
- [X] 跑 `agentwatcher-cli skill install`，再跑 `skill list`，装过的宿主变成 installed；去对应目录（如 `.zcode\skills\agentwatcher\SKILL.md`）确认文件存在、开头有 `name: agentwatcher`。
- [X] 在装了技能的宿主里开一个新 AI 会话，问它「本机有哪些 AI 会话在等人回复」，它能直接用 agentwatcher-cli 查出来。
- [X] 跑 `agentwatcher-cli skill remove`，再 `skill list`，状态回到 not-installed，宿主目录里没有残留的 agentwatcher 文件夹。

## 为什么有这些条

信封结构、退出码、列表无正文、skill 生命周期（经临时目录）已由契约测试覆盖；这里只剩「人打开文件看内容对不对、真实宿主目录的安装效果、新会话里 AI 是否真会用、中文显示」这类机器验不了的现象。
