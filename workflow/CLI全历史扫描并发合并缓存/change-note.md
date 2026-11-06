---
schema_version: "1"
protocol: "1.3.0"
artifact: "change-note"
artifact_id: "ar_45N6ZFYYYZ9GZX2CG67QYDEPJQ"
work_item_id: "wi_3NM7QSRDWZHE0QCAXWTDJ2VNPX"
created_at: "2026-11-06T03:29:28Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:94c4459d059aef000a97f13dc3c6265f90517a921ef86a9ee550227bacefb717"
  artifacts:
    - artifact_id: "ar_45YYZSVXXZGH9914TG4RMKNNQF"
      digest: "sha256:e4ed49d9519f2a6a7755a10fd8f7be64e7c137b7109e391d81e981c21a5b0c75"
      locator: "plan.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "c67331b3d5476031cca6dd16eafa5f3f82a04d1b"
    revision: "c67331b3d5476031cca6dd16eafa5f3f82a04d1b"
    tree: "af6f12dda9bea2a61e1541ed4a575dc1012e023e"
    content_digest: "sha256:6bb97c863678ec86cdf1a28b17d454e30753058f603b7c4caf759ea694f41396"
    branch_or_pr: "feature/CLI全历史扫描并发合并缓存"
    workflow_excluded: true
result: "complete"
---

# 变更理由

为什么改：2026-11-06 会话 sess_d3a5e9ee（adt-scene-agent）4 个并行 subagent 各跑一次
`session list --active-days 0 --include-archived --limit 20000`，同一分钟级重扫描并发 4 份，
磁盘 I/O 线性叠加，叠加当天约 100GB 文件删除的滞后负载，机器卡死（任务管理器无法打开）。

改了什么（提交 c67331b）：
- 新增 `src-tauri/src/bin/agentwatcher-cli/scan_cache.rs`：跨进程缓存 + 独占文件锁（LockFileEx）。
- `commands.rs::session_list` 全历史模式走缓存；status/workspace 过滤移到投影后内存执行。
- `cli.rs` 新增 `--no-cache`；`main.rs` 接线；Cargo.toml 启用 `Win32_System_IO` feature（OVERLAPPED）。
- 测试：scan_cache 单元 4 条、taxonomy 补 `--no-cache` 解析、contract 补「默认窗口不落缓存」。
- 文档：AGENTS.md CLI 一节、skill/SKILL.md 守则同步缓存口径。

不变：lib 三个 pub 扫描入口、信封契约、LIST_KEYS、human 格式、退出码。
