---
schema_version: "1"
protocol: "1.3.0"
artifact: "implementation"
artifact_id: "ar_35QQ7HZD7QA793KM6KP7NWRYTB"
work_item_id: "wi_5VYQZTXYZD0H5M6CN9TKTRFJC3"
created_at: "2026-10-29T09:48:35Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:6848ab9fb6f89246f58decd1503a13d728d7aa614109393c23af3b6f2ffae203
  artifacts:
    - artifact_id: "ar_3HX7BYZFZDKZJXHEXVV6K51TW5"
      digest: sha256:60c79a84973cd1b23a1b6d935bb14f3e78acc05cfebdb18d408e4c45a9862ae6
      locator: "change-note.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "e8dbd949b77d217d5e7bdf7773d5d12e4016a437"
    revision: "e8dbd949b77d217d5e7bdf7773d5d12e4016a437"
    tree: "a2335216ba100fdaf0bd0c46b47716c986e13c10"
    content_digest: "sha256:d17e421222a8a15f1aca54b1872097d7907aa31031f0a21dd0e26af6321ddec4"
    branch_or_pr: "feature/逐轮降智审计器"
    workflow_excluded: true
result: "complete"
---
# 实现

## 做了什么

按计划 S1–S5 做完逐轮降智审计器：`session_lifecycle` 返回新增 `turnAudit` 逐轮聚合行（四类信号在 Rust 侧判定），六个数据源的时间轴解析填充请求模型、档位、思考 token、成本、首 token 时延、重试与错误列；会话分析面板新增可折叠审计表、时间轴信号标记与「模型 × 档位」分解；悬浮预览加模型链与信号数摘要行（主窗口计算随事件补发）；会话交接包新增 `audit/turns.ndjson`。

## 逐步结果

| 计划步 | 结果 |
| --- | --- |
| S1 测试先红 | 16 个编译错误（`build_turn_audit` 与新字段不存在），符合预期红灯 |
| S2 核心结构与信号判定 | `turn_audit` 系列 9 个测试全绿 |
| S3 六源填充 | Claude/Copilot Chat/Codex/ZCode/OpenCode 各构造点填齐；ZCode 测试夹具补新列并断言审计字段 |
| S4 分析面板与预览 | 真实壳验证：分析面板 4 行审计、1 个信号徽标；预览摘要行显示 `GLM-5.3 · 1 signals` |
| S5 打包导出 | 真实打包流程通过；zip 内 `audit/turns.ndjson` 首行为真实数据（gpt-6-astra/low/1493239 输入） |
| S6 全量验证 | `cargo test` 126+8+9 全过；`test:tauri:smoke` 通过 |

## 改了哪些代码

- `src-tauri/src/lib.rs`（4 次提交：3f42689、c6ac6a7、5f2db9a、e8dbd94）
- `ui/index.html`（2 次提交：c6ac6a7、e8dbd94）

## 自审发现的问题

| 问题 | 怎么发现的 | 修了没有 |
| --- | --- | --- |
| 预览窗口能力面不允许自己 invoke 命令，摘要行第一版拉不到数据 | 真实壳 CDP 检查：摘要行元素存在但空 | 修了：改由主窗口算好随 `agentwatcher-preview-data` 事件补发（e8dbd94） |
| 摘要请求误用 `analyticsTargetFromSession`（读 `session.id`，预览数据是 `sessionId`），拿到空 id 静默空手而归 | 同上，逐层排查后主窗口直连 invoke 探针证明数据在 | 修了：按预览字段直接构造请求（e8dbd94） |
| 预览 body 离屏构建，同步填充被 `isConnected` 卫语句吞掉，行永远空白 | 同上，导出被服务前端的实际函数逐行比对 | 修了：摘掉卫语句（e8dbd94） |
| Copilot Chat 点位模型从 `modelId` 改为优先 `resolvedModel` | 实现时的口径决定，写进了提交 3f42689 正文 | 属计划内对齐，无需再修 |

## 哪里没按计划走

- 计划 S2/S3 的边界移动了：`LifecycleCallPoint` 的字段扩展从 S3 提前进了 S2（S1 的测试引用新字段，不一起实现编译不过）。行为不变，只影响提交归组：S1–S3 合成一次提交（3f42689，Step 标 S2）。
- S4 验证方式从「只跑 smoke」扩成三段：smoke 附着了主检出的旧 Vite，UI 改动实际是另起 worktree 专属 Vite（1421 端口）加隔离 CDP 脚本验证的（`.tmp/ui-audit-check.mjs`），另跑了一次真实打包 flow 解包核对 ndjson。
- 计划里「opencode-session flow」没跑成：本机没装 OpenCode CLI，流程在环境处失败（非本次改动导致）；ZCode 会话的真实交互由上述 CDP 脚本覆盖。

## 计划外做的事

| 做了什么 | 计划里有吗 | 不做它验收标准能达成吗 |
| --- | --- | --- |
| worktree 的 tauri.conf.json 临时改 devUrl 到 1421 再改回 | 没有 | 能达成，但改回前 UI 验证会测到主检出旧前端。测试脚手架行为，已还原 |
| node_modules 以 Junction 连进 worktree 供 Vite/Playwright 使用 | 没有 | 能达成，但真实交互测试起不来。已删除 |

## 跑过的检查

| 检查 | 结果 |
| --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 126 通过 0 失败（1 忽略，与基线一致） |
| `cargo test --manifest-path src-tauri/Cargo.toml --test cli_contract --test cli_taxonomy` | 8 + 9 全过（命令面未动） |
| `npm run test:tauri:smoke` | 通过（`.tmp/tauri-realtest/20261029-171837/result.json`） |
| `npm run test:tauri:flow -- session-package --isolated-runtime` | 通过（`20261029-172210/result.json`） |
| worktree 前端 CDP 逐项核对（预览行、审计表、标记点） | 全部命中（`.tmp/ui-audit-check.mjs` 输出） |
| 打包产物解包核对 | `audit/turns.ndjson` 在包内、manifest 含 SHA256 条目 |

## 剩余风险

- Copilot Chat 的字段映射仍只有代码级 fixture 佐证（本机无活跃会话文件），真机核对留给人工清单（AC-003 对应条目）。
- Claude `usage.speed`、`service_tier` 只原样展示未参与判定，取值集合只见过 `standard`。
- 时间轴信号标记在降采样（degraded）会话上基于已采点位，标记可能稀疏；返回里已带 `degraded`，UI 注明。
