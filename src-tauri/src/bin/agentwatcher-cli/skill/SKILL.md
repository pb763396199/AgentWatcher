---
name: agentwatcher
description: 查询本机 AI 助手（VS Code Copilot、Copilot CLI、Claude Code、Codex、OpenCode、ZCode）的会话状态、用量与内容，并导出接续上下文。用户想知道哪些会话在等回复、跑着还是闲着，或要把某个会话接续到新会话时使用。
---

# AgentWatcher 会话查询

先确认命令可用：执行 `agentwatcher-cli --version`。提示找不到命令时，向用户询问 AgentWatcher 的安装目录，可执行文件是目录里的 `agentwatcher-cli.exe`（与 AgentWatcher.exe 同目录），后续命令都带完整路径执行。

## 什么时候用

- 想知道本机有哪些 AI 会话、哪些在等人回复（waiting）、哪些在跑（running）、哪些闲着（idle）。
- 想看某个会话聊了什么、花了多少 token、调了多少次工具。
- 要把某个会话的上下文导出来，交给新的会话接续。

## 命令

| 命令 | 做什么 |
| --- | --- |
| `agentwatcher-cli session list [--provider <名>...] [--status waiting\|running\|idle] [--workspace <路径>] [--limit <n>]` | 会话元数据列表，不含对话文本 |
| `agentwatcher-cli session show <id> [--content]` | 单会话详情（预览级）；`--content` 取正文视图 |
| `agentwatcher-cli session usage <id>` | 用量明细：token、轮次、工具调用、成本、topTools |
| `agentwatcher-cli handoff export <id>` | 导出接续上下文 Markdown |

会话 ID 形如 `opencode:abc123`，从 `session list` 的输出里拿，跨次调用稳定。

## 输出格式

默认输出 JSON 信封，一条命令只输出一个文档：

```json
{
  "command": "session list",
  "ok": true,
  "data": {},
  "error": null,
  "messages": []
}
```

- 退出码：0 成功；1 命令失败（含 `session_not_found`）；2 用法错误。
- 空列表、某个 provider 未安装都不是失败。
- 只解析 json 模式的 stdout；`--format human` 是给人看的，不要解析它。

## 内容分级

`session list` 只有元数据（状态、标题、工作区、时间、用量）。`session show` 默认到预览级（标题、状态、最后用户输入、AI 摘要）。`--content` 才有正文：OpenCode / ZCode 给完整 transcript 的导出文件路径（Markdown，可直接读文件），其他 provider 给原始会话文件路径（JSONL，需要自己解析）。

## 守则

- 全部查询命令只读；唯一写盘动作是 `handoff export`，写到系统临时目录的 `AgentWatcher\handoff-sources\` 下，每次导出生成新文件。
- 要最新状态就重新执行 `session list`，命令不缓存结果。
- 命令不启动任何后台服务，也不依赖 AgentWatcher 桌面程序在运行。
