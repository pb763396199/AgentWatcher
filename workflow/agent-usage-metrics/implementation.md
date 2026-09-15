---
schema_version: 1
protocol: 1.3.0
artifact: implementation
artifact_id: ar_01M2J40XMD0000GC2DDBG67VQ0
work_item_id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
created_at: 2026-09-15T08:48:34Z
producer: aes-execute
result: complete
supersedes: null
dependencies:
  work_item_contract_digest: sha256:b1d0e0503375d88153a6711d42195454acdbf4b2117b2b4a4a6cca41a6fcc8d9
  artifacts:
    - artifact_id: ar_01M2J40XKJ0004SY6GMZW2RKW6
      digest: sha256:b6056b09c2465fc6738527c509ef7c18a81f8aa617478672cd9cc80f986ff942
      locator: change-note.md
    - artifact_id: ar_01M2J1HY7000034P57XZ03CQ8W
      digest: sha256:19eb2e8ec3d41f479085aae6ce8ee724b5831ee8ad840ddb52a589e64bccf62f
      locator: plan.md
  subject:
    kind: change_set
    digest: sha256:15ddace05bae1e680465fa93adb7d941ce6d9db06fa276ac27b6725893c3a280
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: bc915a208539f73abc4964919eabcfb69e66bbf6
    revision: bc915a208539f73abc4964919eabcfb69e66bbf6
    tree: d752bf95fbe0cca563743eb717c1563c9c38acc2
    content_digest: sha256:15ddace05bae1e680465fa93adb7d941ce6d9db06fa276ac27b6725893c3a280
    branch_or_pr: feature/agent-usage-metrics
    workflow_excluded: true
---

# 实现记录

## 做了什么

按计划 S1–S8 完成：Rust 端 `SessionUsage` 数据模型、三个 JSONL 聚合器与增量缓存、六源接线、
`get_session_usage_detail` 懒加载命令；前端卡片用量行、预览「用量」区块、i18n；
测试与真实壳验证全部通过。

## 逐步结果

| 步 | 结果 | 证据 |
| --- | --- | --- |
| S1 数据模型 | 完成 | `cargo test agent_session_usage_fields_serialize` 通过 |
| S2 聚合器与缓存 | 完成 | 新增 8 个单元测试全部通过（去重/公式/补丁日志/缓存命中/预算耗尽） |
| S3 六源接线 | 完成 | zcode 观测表聚合、opencode 汇总列、copilot_cli 行内构造各有 fixture 测试 |
| S4 detail 命令 | 完成 | `get_session_usage_detail_serves_jsonl_and_missing_sessions` 通过；真实壳对 sess_eeec0860 返回 topTools |
| S5 卡片用量行 | 完成 | 真实壳 DOM 出现 8 条 `.card-usage`（claude/zcode/codex） |
| S6 预览用量区块 | 完成 | 真实事件驱动预览渲染：hero「Total input 43.8M cached 98%」+ 八格网格含懒加载 topTools |
| S7 验收门 | 完成（见验证） | cargo test 91 通过；build:ui 通过；smoke 通过；zcode flow 通过；git diff --check、bridge 语法检查通过 |
| S8 记录与提交 | 完成 | 代码单独提交到 feature/agent-usage-metrics |

## 改了哪些代码

- `src-tauri/src/lib.rs`：+约 1,400 行（SessionUsage/聚合器/接线/detail 命令/8 个新测试）。
- `ui/index.html`：+约 340 行（CSS、卡片行、预览区块、i18n、签名）。
- 完整改动理由逐条见 `change-note.md`（ar_01M2J40XKJ0004SY6GMZW2RKW6）。

## 自审发现的问题

| 问题 | 怎么发现的 | 修了没有 |
| --- | --- | --- |
| Codex `input_tokens` 最初按「input+cached」相加，毛口径理解错误 | 单元测试期望值对不上，回查来源会话样本（total=input+output，cached ⊆ input） | 已修：只加 cache_write，缓存读取作为子集单列 |
| Claude 用户轮次漏计 `message.content` 形态的真实记录 | 单元测试 user_turns 差 1 | 已修：兼容顶层 content / message.content / message 字符串三种形态 |
| 卡片用量行在 141px 宽度下切半字（原型稿阶段） | 原型视觉复核 + DOM 断言 | 已修：text-overflow: ellipsis，bottom 间距 17px→20px |

## 哪里没按计划走

| 做了什么 | 计划里有吗 | 不做它验收标准能达成吗 |
| --- | --- | --- |
| 修复主检出残留 vite 进程占用 1420 端口，导致 worktree dev 复用旧 UI | 没有 | 能达成，但不修就无法在真实壳验证新 UI（served 页面来自主检出旧代码），属测试基建排障 |
| 计划步 ID 从 P 系列改为 S 系列 | 没有 | 能。依据 commit-style 对 Step trailer 的 `S<数字>` 形态约定，提交前对齐 |

## 跑过的检查

- `cargo test --manifest-path src-tauri/Cargo.toml`：91 passed / 0 failed / 1 ignored。
- `npm run build:ui`：构建成功（288ms）。
- `npm run test:tauri:smoke`：通过（真实 Tauri 壳 + WebView2 CDP，主窗口/AgentTask/性能/接续面板）。
- `npm run test:tauri:flow -- zcode-session`：通过。
- `npm run test:tauri:flow -- opencode-session`：失败——**环境性**，本机 7 天活跃窗口内没有 OpenCode 会话
  （最近一次 2026-08-25，SQL 直查 7 天窗口命中 0 行），扫描端本来就不该出卡片；与本次改动无关，
  同基线同样失败。scan SQL 本身已对真实 4.46GB db 直查验证无误。
- 真实壳端到端（CDP）：scan_sessions 68/68 会话带 usage，数字与调研记录交叉核对一致
  （Claude c25596b2：808 次调用/1302 工具/1,875,661 输出；ZCode sess_743d9fde：221.5M 毛输入/693 工具）。
- `git diff --check`、`node --check vscode-agentwatcher-bridge/extension.js`：通过。

## 剩余风险

- 首次扫描（缓存冷启）每扫描最多全量聚合 10 个文件，68 会话约需 7 轮扫描渐进补齐（15 秒轮询下约 2 分钟），
  期间部分会话用量显示 —。属拍板 5 的预期行为。
- `opencode-session` flow 在本机因活跃窗口内无会话而无法通过；换一台有活跃 OpenCode 会话的机器应可恢复。
- 扫描延迟实测 1.4–1.6s（真实壳 68 会话），未做新旧代码同机 A/B；增量部分按组件实测（ZCode 批量 SQL 与
  OpenCode 汇总列均为毫秒级，JSONL 命中缓存后为 0），主耗时仍是既有 Codex app-server RPC。
