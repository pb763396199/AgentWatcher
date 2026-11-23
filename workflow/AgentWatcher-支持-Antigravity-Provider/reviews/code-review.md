---
schema_version: "1"
protocol: "1.3.0"
artifact: "review"
artifact_id: "ar_5ZNZKFXGWX9T2KP57Z8R7Y0MZ1"
work_item_id: "wi_0ZQ8FFAX6K2B4D8F66T96TFDEM"
created_at: "2026-11-23T04:47:12Z"
producer: "aes-review"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:377edb0dac11131577aa3523826b0b7accc7a012a8b0fef5fe9226ef0433ed54
  artifacts:
    - artifact_id: "ar_1KQFQBQTDZ59ZS14AYZ7AZJ9B5"
      digest: sha256:53a75aa42cd37bb7a90265cd542a3f64480a6bfb7c5eb4dacc63e79914caa7b6
      locator: "implementation.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "1b81016f80326705a98e600da89baa4c8b9b9de1"
    revision: "1b81016f80326705a98e600da89baa4c8b9b9de1"
    tree: "f7dde8baab2eb35e4472b65f25e8e5cdcd56f5d2"
    content_digest: "sha256:90994b90b2b68feb47236bc125b15a975e4f6c309833abbd5c99ea82226c6b9b"
    branch_or_pr: "feature/AgentWatcher-支持-Antigravity-Provider"
    workflow_excluded: true
verdict: "changes_requested"
review_type: "code"
reviewers: []
---
# 代码评审：Antigravity Provider（变更集 0a15bfa + 1b81016）

由独立评审代理只读审查（未改任何源码；仅在临时目录做过一次溢出探针复现后删除）。
审查范围：git diff 6d0aca0..1b81016 全量 + 全量测试复跑。

## 阻断（2 条）

1. **WorkBuddy 会话从性能面板计数中消失**（lib.rs `update_performance_scan_snapshot`）：
   `"workbuddy" => provider_counts.workbuddy += 1,` 匹配臂被 antigravity 臂**顶掉**而非追加，
   结构体字段仍在但全仓无递增点；UI 仍显示 `WB <n>` 会恒为 0。最小改法：补回该行。
2. **WorkBuddy WAL 监听根被误删**（lib.rs `session_watch_roots`）：
   `push_existing_path(&mut roots, workbuddy_db.with_extension("db-wal"))` 被 antigravity
   三行顶掉，只写 WAL 的变更不再触发重扫；紧邻注释仍声称挂 -wal，自相矛盾。最小改法：
   恢复该行。

两条都是补丁脚本按行锚点插入时顶掉了既有行，静默回归、无测试兜底。

## 建议（5 条）

3. CLI `suggest generate/prompt` 的帮助文本写了 `antigravity:<id>`，实现未接线：命令会以
   `unsupported_provider` 拒绝且错误文案没提 antigravity。最小改法：从两处帮助文本删掉
   `、antigravity:<id>`（真接线后再加回）。
4. 全历史模式未解除 240 行候选上限：兄弟 provider 都走 `sqlite_session_scan_limit(options,…)`
   在全历史放宽，AGENTS.md 也承诺「解除 SQLite 240 行扫描上限」。最小改法：改用该函数。
5. `antigravity_summarize_conversation` 单行读错会清空整个会话摘要（`.ok()?` 把行级错误当
   整体失败），违背「按行降级」。最小改法：坏行 `continue` 跳过。
6. wire 解析器 `bytes.get(cursor..cursor + length)` 在 debug 构建下 length 接近 u64::MAX
   时加法溢出 panic（评审代理用探针复现）；release 回绕后安全。最小改法：`checked_add`。
7. 扫描热路径无缓存预算：250ms 防抖每次触发对最多 240 个 conversations 库全表读；WorkBuddy
   同类聚合有 mtime 失效预算缓存，antigravity 没有。最小改法：按 mtime 失效的进程内缓存，
   或全量计数挪进悬停懒加载。

## 提示（7 条）

8. `lookup_session_via_wide_scan` 漏补 `include_antigravity` 赋值（其他 provider 兜底宽扫会
   顺带扫 antigravity，纯浪费）。
9. 新增行缩进丢失（lib.rs 818/835/849/13094、commands.rs 46/62/254 顶格）。
10. `AntigravitySummaryRow.killed`/`step_count` 死字段（仅测试读）。
11. 候选预过滤单向依赖清单时间戳（daemon 滞后于正文时漏报新会话）；order by 是 TEXT 排序。
12. waiting 闭合只认 type-14 用户轮；若答案写成 132 工具结果会卡在 waiting；ask 落出 24 行
    尾窗后不再检出。设计已标注「结构推断，真实样本待人工核对」。
13. `session show --content` 对 antigravity 只给二进制 .db 路径，没有节选；可复用接续导出。
14. 清单库 schema 缺列时整个 provider 静默为空（未沿用 opencode 的 pragma 探测范式）。

## 查过，没发现

SQL 注入（全参数化、路径拼接有字符白名单）、时间戳换算与 usage 饱和加法、varint 读取器、
percent 解码健壮性、ScanOptions 字面量补字段（编译器兜底）、ZCode 建议段搬移（逐字节一致）、
UI i18n 双语与双主题配色、隐私边界（预览正文只走事件、无 localStorage）、监听与防抖、
文档口径（除第 4 条外无打架）。

## 结论

changes_requested。阻断 2 条必须先修；建议 3、4 与提示 8 应随修复一并处理。antigravity
本体实现（wire 解析、状态机、过滤、导出、UI/测试接线）质量扎实，修完复审。
