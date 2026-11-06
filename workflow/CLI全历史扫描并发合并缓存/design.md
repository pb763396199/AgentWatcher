---
schema_version: "1"
protocol: "1.3.0"
artifact: "design"
artifact_id: "ar_3SYEQBXY7P82ZZD4ME8XKV3311"
work_item_id: "wi_3NM7QSRDWZHE0QCAXWTDJ2VNPX"
created_at: "2026-11-06T02:01:26Z"
producer: "aes-brainstorm"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:94c4459d059aef000a97f13dc3c6265f90517a921ef86a9ee550227bacefb717"
  artifacts: []
result: "accepted"
---
# 设计：CLI 全历史扫描并发合并缓存

## 要解决什么

`agentwatcher-cli session list` 在全历史模式（`--active-days 0` 或 `--include-archived`）下每次调用都做全量扫描：Codex `.jsonl.zst` 头部解压、OpenCode/ZCode/WorkBuddy 的 SQLite 全库、上万个 JSONL 文件。这个操作是分钟级的。多个 agent 进程在几秒到几分钟窗口内用相同参数各跑一遍时，磁盘 I/O 随进程数线性叠加——2026-11-06 会话 sess_d3a5e9ee 四个并行 subagent 实测把机器卡死。默认活跃窗口（7 天）的扫描轻，不需要动。

## 第一性原理

问题的本质：**同一个确定性函数（扫描参数 → 会话列表）在短窗口内被求值多次，而每次求值代价是分钟级 I/O。** 消除重复求值只有两条正路：让后来的求值等待第一次的结果（single-flight），或让后来的求值直接取走存好的结果（缓存）。两者共用一个带锁的缓存文件就能同时拿到：锁保证同一时刻只有一个进程在扫，缓存把「等待」的收益窗口从「扫描进行中」延长到「扫描完成后 TTL 内」。参数一旦不同，就是不同的函数，各用各的缓存键，互不污染。

## 方案对照

| 方案 | 做法 | 为什么没选 |
| --- | --- | --- |
| A. 只写文档，教 agent 串行调用 | SKILL/AGENTS 加一句「别并发跑全历史」 | 挡不住：agent 是每次新生成的，约定不会遗传；机器卡死是宿主该防的事故 |
| B. 缓存做进 lib 的 `scan_sessions_blocking` | GUI 和 CLI 共享缓存 | 波及面大：GUI 有自己的防抖、配额和事件驱动刷新，缓存一致性要跟 250ms 防抖、文件监听联动，收益却只在 CLI 侧 |
| C. 缓存 `Vec<AgentSession>`（加 `Deserialize`） | 缓存原始扫描结果 | 要给 `AgentSession` 及其嵌套 usage 类型补 `Deserialize`，改动落进 lib.rs 主结构；且 list 输出本来就要投影成固定键，缓存投影后的 JSON 更小也更贴契约 |
| D. 后台常驻服务/daemon | 扫一次常驻供查询 | 违反 CLI「无副作用、不常驻」的既定口径，为低频查询养进程不值 |
| E. 锁 + TTL 缓存，缓存最终投影 JSON | 见下 | **选定**：全部改动收在 CLI bin 内，不改 lib 类型签名，不动 GUI |

## 决定的方案

新模块 `src-tauri/src/bin/agentwatcher-cli/scan_cache.rs`，`session_list` 在全历史模式下经它包裹 `scan_sessions_blocking`：

1. **缓存键**：`sha256(CLI 版本 ‖ 规范化后的全部扫描相关参数)`——provider 集合（排序后）、limit、active_days、include_archived、status、workspace 过滤、no_cache 不参与键。取前 24 个十六进制字符做文件名。
2. **缓存值**：投影后的最终 `data` JSON（`{"sessions":[…],"total":N}`，即 `project_list_item` 的产物）。格式带 `version` 字段，读到不认识的版本静默弃用。
3. **位置**：`%TEMP%\AgentWatcher\cli-scan-cache\<key>.json`，写入走 `.tmp` + 原子替换。写完后顺手删掉目录里超过 1 天的旧缓存文件，缓存目录总量因此有自然上限（一天窗口内同参数的不同键有限，单文件典型 1–10 MB）。
4. **锁**：同目录 `<key>.lock`，Windows `LockFileEx` 独占锁（用已有 `windows-sys` 依赖，不加包）。流程：拿锁 → 缓存新鲜（TTL 120 秒，写在缓存文件的 `generatedMs` 里）则直接读返回 → 否则真实扫描 → 原子写缓存 → 放锁。后到的进程阻塞在拿锁上，拿到时缓存必然新鲜，直接读。
5. **降级**：锁等待超过 15 分钟（扫描理论上限）就放弃合并、直接扫描不写缓存；缓存文件损坏、字段缺失、反序列化失败一律当未命中处理。降级路径永不改变退出码。
6. **`--no-cache`**：`session list` 子命令新旗标，不读不写缓存、不拿锁，直接扫描。
7. **口径**：AGENTS.md「AI 操作 CLI」一节补一段缓存行为说明（120 秒内同参数结果可能来自缓存、`--no-cache` 绕过、缓存位置）；`skill.rs` 内嵌的 SKILL.md 同步一句。契约测试补断言：默认窗口模式不产生缓存文件。

### 不变的量

- 信封五键、退出码、`LIST_KEYS` 投影、human 输出格式全部不动；命中缓存与真实扫描的 `data` 逐字段等价。
- TTL 内可能读到最多 120 秒前的快照，这是文档化的行为，不是缺陷；需要最新数据的调用方用 `--no-cache`。

## 走一遍场景

- **顺利**：4 个 subagent 同秒启动同参数 list → 进程 1 拿锁扫描写缓存，进程 2–4 阻塞后读缓存，总 I/O = 1 次扫描。
- **错开 1 分钟**：进程 1 已写完缓存退出，进程 2 无锁竞争直接命中缓存。
- **2 分钟后又来一次**：TTL 过期，重新扫描，属预期。
- **缓存被删/损坏**：未命中 → 直接扫描重写，用户无感。
- **持锁进程崩溃**：Windows 文件锁随句柄关闭自动释放，锁文件残留无害；下一个进程拿锁、发现缓存缺失或过期、照常扫描。
- **中断后回来**：无任何需要恢复的状态，缓存文件自足。

## 放弃了什么

- 不做 GUI 侧合并（方案 B）：GUI 已有防抖和文件监听，事故只发生在 CLI 并发。
- 不做跨参数的部分结果复用（比如按 provider 分片缓存）：复杂度换不来收益，事故场景是同参数重复。
- 不给 `show`/`usage`/`handoff export` 加缓存：它们是按 ID 定向单次 I/O，本来就不重。

## 影响面

- `grep scan_sessions_blocking src-tauri/src/bin/` 现有调用 2 处（list、find_session），只改 list 那处的包裹方式；lib 的三个 pub 入口签名零改动。
- 契约测试 `cli_contract.rs` 现有断言零改动，只新增；`cli_taxonomy.rs` 需要补 `--no-cache` 的解析断言。
- 新增依赖：无（`windows-sys` 已在，`sha2` 已在）。
