---
schema_version: "1"
protocol: "1.3.0"
artifact: "design"
artifact_id: "ar_7KQYQVDZFB4P0TY6T8K70B38QK"
work_item_id: "wi_5VYQZTXYZD0H5M6CN9TKTRFJC3"
created_at: "2026-10-29T08:07:17Z"
producer: "aes-brainstorm"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:6848ab9fb6f89246f58decd1503a13d728d7aa614109393c23af3b6f2ffae203
  artifacts:
    - artifact_id: "ar_51YQVBEYQZ5KTHC6V0BP3BT5B9"
      digest: sha256:b273c0fa3a5000025905e06a766fd207b60fdc3aabe28b1685326f6ace0fd697
      locator: "research/provider-audit-signals-research.md"
result: "superseded"
---

# 逐轮降智审计器设计

## 要解决什么

用户判断「模型变笨了」现在只能靠体感。调查（research/provider-audit-signals-research.md）确认六个数据源里四个有完整的逐轮痕迹、一个部分有、一个没有。这个设计把痕迹统一提出来，做成 AgentWatcher 里每会话可查看的逐轮记录：每一轮是谁在回答、什么档位、花了多少、有没有换模型或重试。

## 背景

AgentSession 现有的 `usage` 字段是全会话聚合值（AGENTS.md 口径），轮与轮之间的差异被平均掉了；模型切换、档位变化、重试这些事件在聚合值里不可见。前一轮对 ChatGPT 桌面端的逆向（会话外调查，记录在本任务之外）证明逐轮信号存在且可用，本设计的证据以本机六个数据源的实测为准。

## 非目标

- 不抓原始思维链：所有源都拿不到（Codex 只有密文），前一轮已确认。
- 不做任何发送或干预动作。
- 不新增对外网络请求，不新建常驻服务或数据库。
- 不搬运对话正文：审计结构里只有元数据，内容分级不动。

## 方案对比

| 方案 | 怎么做 | 代价 | 为什么没选 |
| --- | --- | --- | --- |
| A 扫描时即时提取（同 usage 管道） | 不建库；懒加载命令现读现算，会话文件本身就是历史账本 | 大会话每次详情都要重读源文件；Codex 需要读全文件而不只是尾部采样 | ——（选定） |
| B 本地审计库累积 | 新建 SQLite 逐轮存档，可画跨会话趋势 | 引入新状态与迁移；和「扫描无副作用」的现有口径冲突；插件宿主硬规则风险 | 第一版不需要历史累积就能回答「这轮是谁答的」；趋势视图等有真实需求再做 |
| C 什么都不做 | 继续靠体感 | 降智永远不可核对 | 调查已证明数据就在本机，不做没有理由 |

选定 A：与 `get_session_usage_detail` 同型，改动面最小，不新增任何持久状态。

## 选定方案

### 统一结构

每轮一条 `TurnAuditRecord`：

| 字段 | 来源（有则填，无则 null） |
| --- | --- |
| `turnIndex`、`startedAtMs`、`durationMs` | 各源轮次与时间字段 |
| `modelRequested` | Claude `requestedModel`；Copilot Chat `modelId` |
| `modelServed` | Claude `message.model`；Codex `turn_context.model`；OpenCode `data.modelID`；ZCode `model_usage.model_id`；Copilot Chat `resolvedModel` |
| `effortOrVariant` | Claude `effort`/`perTurnEffort`；Codex `turn_context.effort`；OpenCode/ZCode `variant` |
| `tokens{input,output,reasoning,cacheRead,cacheWrite}` | 各源 token 列；Copilot Chat 只有 input/output |
| `costUsd` | OpenCode `data.cost` |
| `timeToFirstTokenMs` | ZCode 专属 |
| `retries`、`errorType`、`contextExceeded` | ZCode 专属列；其它源 null |
| `rawNotes` | Claude 的 `service_tier`/`speed`/`fallback_credit` 原样存放，只展示不判定 |

### 四类信号，各自独立成列，不合成分数

1. 模型变化：`modelRequested ≠ modelServed`（两个源可判），或轮间 `modelServed` 改变（五个源可判）。
2. 档位变化：轮间 `effortOrVariant` 改变。
3. 用量异常：该轮输出 token 低于本会话中位数三成。规则写死、阈值常量，先不做配置。
4. 可靠性：`retries > 0`、有 `errorType`、`contextExceeded`。

### 提取与展示

- 新增一个懒加载命令 `get_session_turn_audit({ id })`，与 `get_session_usage_detail` 同型（每 provider 一个提取分支，Copilot CLI 分支返回明确的 `notSupported`）。
- 会话详情（悬浮预览面板）在用量段下面新增可折叠「逐轮审计」表：轮次、模型、档位、输出 token、四类信号徽标。中文/英文、dark/light 同步。可点开的样子见原型稿 [mockups/turn-audit-panel.html](mockups/turn-audit-panel.html)（含正常表、换模型/低输出/重试三种信号标记、Copilot CLI 不支持提示，带主题与语言切换；数据为示意）。
- 数据只走 Tauri 事件与命令通道，不进 localStorage（隐私强约束）；表里没有对话正文。
- 右键「打包会话」的 zip 里加 `audit/turns.ndjson`，进 manifest 的 SHA256 清单。
- 思考的痕迹只展示元数据：Claude 显示思考块数与 `thinkingDurationMs`，Codex 显示 `summary` 是否开启，ZCode/OpenCode 显示 `reasoning` token 数。不搬正文。

### 场景走查

- 顺利：用户觉得今天回复变差，打开会话详情，逐轮表第 12 轮起 `modelServed` 变了、徽标点亮「模型变化」——有据可查。
- 出错：老文件缺字段（如 Claude 旧记录没有 `requestedModel`）→ 该格空白，不点亮任何信号。
- 中断回来：会话文件追加后再扫描，详情重新懒加载即可，没有需要恢复的状态。

## 影响面

复算命令在仓库根跑：`grep -c "fn scan_.*_sessions" src-tauri/src/lib.rs`（=8，含辅助函数）、`grep -c "#\[tauri::command\]" src-tauri/src/lib.rs`（=21）。

| 落点 | 现状 | 本次改动 |
| --- | --- | --- |
| `src-tauri/src/lib.rs`（18068 行） | 8 个 scan 函数、21 个命令、`get_session_usage_detail` 懒加载模式已存在 | 新增 turn audit 提取模块与 1 个命令，预计 400–600 行 |
| `ui/index.html`（18366 行） | 预览面板已有用量段 | 用量段后追加可折叠审计表，预计 100–150 行 |
| `export_session_package` | manifest 已带逐文件 SHA256 | 包内加 `audit/turns.ndjson` 一项 |
| 测试 | lib.rs 末尾内嵌测试；`test:tauri:flow -- zcode-session` 等流程 | 新增各源提取的单元测试（用本任务调查里实测过的字段构造 fixture）；UI 改动跑对应 flow |

## 边界与失败

- OpenCode 库 10.6 GB：按 `session_id` 过滤读 `message`，每会话行数上限 2000，沿用现有采样口径。
- Codex 现有「尾读 256 KB」采样对逐轮审计不够（`turn_context` 分布在全文件）：审计路径对该文件单独全量读，单文件几 MB 以内可接受，超过上限就截断并在结果里标记 `truncated`。
- 数据库繁忙：沿用只读打开加 `busy_timeout` 的现有模式。
- Copilot Chat 字段口径来自 AgentWatcher 自己的 fixture（本机暂无活跃文件）：实现时先拿真实文件核对，对不上就修映射，不猜。

## 怎么算做对

- AC-001：本设计「统一结构」表 + 调查记录的信号矩阵即交付；每条结论带字段名与文件位置。
- AC-002/003：单元测试用实测字段构造 fixture；在真实本机会话上逐源验证（Claude、Codex、OpenCode、ZCode 至少四个过，Copilot CLI 返回明确不支持）。
- AC-004：`test:tauri:flow` 覆盖预览面板中英文与两套主题；审计数据不写 localStorage 由代码评审与既有隐私测试口径保证。
- AC-005：四类信号各自独立列，无合成分数——评审按这条检查。
- AC-006：`cargo test` 与对应 flow 全过。

## 未决问题

1. CLI 要不要加 `session turns <id>` 子命令。改命令面要同步两个契约测试；建议第一版只做 GUI，CLI 等用户开口。
2. 用量异常阈值（中位数三成）要不要做成可配置。建议先写死，有真实误报反馈再配。
3. 跨会话趋势视图（同一 provider 多会话的模型分布随时间变化）。方案 B 的价值所在，等第一版数据被用起来再评估。
