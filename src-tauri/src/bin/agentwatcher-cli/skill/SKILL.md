---
name: agentwatcher
description: 查询本机 AI 助手（VS Code Copilot、Copilot CLI、Claude Code、Codex、OpenCode、ZCode、WorkBuddy）的会话状态、用量与正文，导出接续上下文，并按会话所属 agent 生成下一条建议回复。用户问哪些会话在等人、在跑、闲着，要把会话接到新窗口，或想让人代拟下一句话时使用。
version: 2
---

# AgentWatcher 会话查询与建议

先确认命令可用：执行 `agentwatcher-cli --version`。提示找不到命令时，向用户询问 AgentWatcher 的安装目录，可执行文件是目录里的 `agentwatcher-cli.exe`（与 AgentWatcher.exe 同目录），后续命令都带完整路径执行。

## 什么时候用

- 想知道本机有哪些 AI 会话、哪些在等人回复（waiting）、哪些在跑（running）、哪些闲着（idle）。
- 想找更早或已归档的历史会话（默认列表只看最近 7 天、隐藏归档）。
- 想看某个会话聊了什么、花了多少 token、调了多少次工具。
- 要把某个会话的上下文导出来，交给新的会话接续。
- 想替某个会话拟一条下一句话（建议回复），由用户决定发不发送。

## 命令

| 命令 | 做什么 |
| --- | --- |
| `agentwatcher-cli session list [--provider <名>...] [--status waiting\|running\|idle] [--workspace <路径>] [--limit <n>] [--active-days <n>] [--include-archived] [--exclude-playground]` | 会话元数据列表，不含对话文本 |
| `agentwatcher-cli session show <id> [--content]` | 单会话详情（预览级）；`--content` 取正文视图 |
| `agentwatcher-cli session usage <id>` | 用量明细：token、轮次、工具调用、成本、topTools |
| `agentwatcher-cli handoff export <id>` | 导出接续上下文 Markdown |
| `agentwatcher-cli suggest prompt <id>` | 只组装上下文与提示词，不调模型、不写盘，用来核对建议依据 |
| `agentwatcher-cli suggest generate <id> [--avoid <上一条>] [--model <模型名>] [--effort <档>] [--timeout <秒>] [--engine auto\|sample\|exec] [--via session\|codex] [--session-path <路径>]` | 真实调模型生成一条建议，会产生 token 用量 |
| `agentwatcher-cli skill list [--dir <路径>]` | 看各宿主技能目录的安装状态 |
| `agentwatcher-cli skill install [--dir <路径>]` | 把这份技能装进宿主目录，重复执行即覆盖升级 |

会话 ID 形如 `opencode:abc123`，从 `session list` 的输出里拿，跨次调用稳定。建议命令额外接受 Codex 的深链形态 `codex://threads/<uuid>`。

`session list` 的 `data` 形如 `{ "sessions": [...], "total": <条数> }`。每个条目的时间有两份口径：`timeLabel` 是相对天数（如 `140d`，给人看的），`updatedMs` 是最后更新的毫秒时间戳（给程序用的）。要按日期范围搜会话（比如某个月），用 `updatedMs` 过滤，别拿 `timeLabel` 换算。

## 状态口径

- `waiting`：在等用户审批或选选项，会话本身停在一个问题上。
- `running`：近期有内容活动，还在干活。
- `idle`：跑完一轮在等下一条指令。

建议回复针对的是「跑完一轮」的场景，也就是 `idle`。`waiting` 的会话通常应该直接回答它提出的问题，而不是推测用户下一句想问什么。

## 生成建议

1. 先 `suggest prompt <id>` 看依据：返回 `prompt`（完整提示词）、`turns`（进上下文的角色序列）、`fingerprint`、`workspacePath`。这一步不花钱，怀疑建议跑偏时先看它。
2. 再 `suggest generate <id>`。它会按会话所属 agent 选生成通道：Codex 会话优先走 app-server 的 `thread/sample`（直接继承真实会话历史，小会话更快），会话文件超过 1MB 或采样不可用时自动回落 `codex exec`；ZCode 走无头 `zcode.cjs`；WorkBuddy 复用 `codex exec` 但上下文取自它的转录。一次大约 5 到 15 秒。要指定模型或推理档就加 `--model gpt-6.1-sol --effort high`；不加则沿用本机 Codex 配置（当前是 `gpt-6.1-sol` / `medium`）。`high` 档本地实测约 30 秒以上，超过默认 45 秒上限时用 `--timeout 180` 放宽。
   - `--engine sample` 强制走采样、忽略体积门控；长会话会把整段历史送进模型，30 万 token 的会话要 25 秒以上，一般不要用。**采样的模型与档位继承会话本身，`--model` / `--effort` 对它无效**；要指定档位请用 `--engine exec`。
   - `--engine exec` 强制走单轮 `codex exec`（只带最近 4 轮上下文），长会话或不希望产生 fork 副本时用。
   - `--via codex` 是**跨引擎**：让 Codex 给非 Codex 会话出建议，用于 copilot / copilot-cli / claude 这类自身没接生成引擎的会话，也可让 ZCode / WorkBuddy 临时换脑。它会读该会话转写的尾部几轮拼成 prompt，再走 `codex exec`（永远不走 `thread/sample`，因为源会话不是 Codex thread）。建议仍标记为原会话的 provider，但 `engineLabel` 是 `codex/cross`。转写读不到就明确失败——opencode 的会话在 sqlite 里，暂不支持。
   - `--session-path <路径>`：跨引擎时手动指定转写文件。后端会按会话 id 里的 uuid 自行找（`~/.claude/projects/*/<uuid>.jsonl`、`~/.copilot/session-state/<uuid>/events.jsonl`），找不到时再用这个参数兜底；界面会直接把扫描到的 sessionPath 带过来。
   - 采样默认用 `service_tier=priority`（与官方预测同档，更快但费率更高）。想退回默认档就设环境变量 `AGENTWATCHER_SERVICE_TIER=default`。
   - 采样通道会为每次生成 fork 一份只读副本并在结束后归档，不会留在会话列表里；它要求该会话存在且至少有 1 轮对话。
3. 返回 `data` 形如 `{ "sessionId", "provider", "text", "question", "options", "fingerprint", "engineLabel", "createdAtMs" }`。`text` 就是那条建议，最多 120 字；`engineLabel` 标出实际生成引擎（`codex/sample` / `codex/exec` / `codex/cross` / `zcode`）。
4. 建议只生成文本，**不会自动发送**。发不发由用户决定；本命令没有发送能力。

失败码：

- `unsupported_provider`：这个 agent 还没接生成通道；改用 `--via codex` 或只能用查询命令。
- `suggestion_generation_failed`：跨引擎时读不到会话转写（文件不存在、格式不认、没有有效的 user 轮）。加 `--session-path` 指定文件再试。
- `generation_failed`：会话文件缺失、资格校验未过（plan 模式 / 非 openai provider / 模型在黑名单）、引擎超时或输出无法解析。真实原因在 stderr，stdout 只有信封。

生成前后会各取一次会话指纹做比对：如果生成期间源会话又前进了，这条建议会作废（`generation_failed`），重跑一次即可。

## 时间窗口与归档

默认只列最近 7 天的活跃会话，归档会话不出现。要搜全历史：

- `--active-days 0` 不限时间（非 0 值夹在 1 到 30）。全历史模式下 `--limit` 上限 20000，默认仍是 80。
- `--include-archived` 连归档会话一起列：Codex 的 `archived_sessions` 目录（含 `.jsonl.zst` 压缩档）、OpenCode/ZCode 的 `time_archived`、Claude/Copilot 的归档标记、WorkBuddy 的 `deleted_at` 软删行（WorkBuddy 没有归档目录，软删就是它的归档口径）。
- 找归档或窗口外的会话推荐组合：`session list --active-days 0 --include-archived --limit 20000`。万级会话库（如 OpenCode）这一条命令数秒返回。
- WorkBuddy 的 playground 会话（`is_playground=1`）默认纳入列表；只要普通会话时加 `--exclude-playground`。没有工作区的 playground 行仍照默认规则排除。
- `session show` / `session usage` / `handoff export` 对归档、窗口外的会话 ID 同样可用（内部会做定向兜底查找），不用先改 list 参数。
- 子代理会话（Codex subagent 线程、ZCode `subagent_child`）和临时目录工作区的会话照默认规则排除。

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

- 退出码：0 成功；1 命令失败（含 `session_not_found`、`generation_failed`）；2 用法错误。
- 空列表、某个 provider 未安装都不是失败。
- 只解析 json 模式的 stdout；`--format human` 是给人看的，不要解析它。

## 内容分级

`session list` 只有元数据（状态、标题、工作区、时间、用量）。`session show` 默认到预览级（标题、状态、最后用户输入、AI 摘要）。`--content` 才有正文：OpenCode / ZCode 给完整 transcript 的导出文件路径（Markdown，可直接读文件），其他 provider 给原始会话文件路径（JSONL，需要自己解析）；Codex 的 `.jsonl.zst` 归档会先解压出一份 JSONL 副本再给路径（同时保留 `rawSessionArchive` 指向原始压缩档）。

## 守则

- 只读命令：`session list` / `session show` / `session usage` / `suggest prompt` / `skill list`。
- 写盘命令：`handoff export` 写系统临时目录的 `AgentWatcher\handoff-sources\`（每次导出生成新文件），对 `.jsonl.zst` 归档取正文时在同目录写解压副本；全历史 `session list`（`--active-days 0` 或 `--include-archived`）写 `AgentWatcher\cli-scan-cache\` 下的跨进程扫描缓存（120 秒有效）；`skill install` / `skill remove` 只动宿主的 `agentwatcher` 技能目录。
- 计费命令：`suggest generate` 不写盘，但会真实调用外部模型、消耗该 agent 账号的用量。不要在一个循环里反复调用；同一条会话要换一条建议时加 `--avoid <上一条>`，而不是重试。两条通道的 token 口径不同：采样送整段历史，exec 只送尾部 4 轮，长会话用 exec 更省。`--via codex` 走的是 Codex 账号，即使源会话属于别的 agent。
- 要最新状态就重新执行 `session list`。全历史模式的同参数查询 120 秒内可能读到缓存快照，需要确保重新扫描时加 `--no-cache`；默认 7 天窗口的查询不缓存。
- 命令不启动任何后台服务，也不依赖 AgentWatcher 桌面程序在运行。
- 宿主里装的技能副本可能落后于当前程序版本：`skill list` 报 `outdated` 时用 `skill install` 覆盖升级，装完再按新版命令用。
