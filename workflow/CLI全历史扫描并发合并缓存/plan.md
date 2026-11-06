---
schema_version: "1"
protocol: "1.3.0"
artifact: "plan"
artifact_id: "ar_45YYZSVXXZGH9914TG4RMKNNQF"
work_item_id: "wi_3NM7QSRDWZHE0QCAXWTDJ2VNPX"
created_at: "2026-11-06T02:03:34Z"
producer: "aes-plan"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:94c4459d059aef000a97f13dc3c6265f90517a921ef86a9ee550227bacefb717"
  artifacts:
    - artifact_id: "ar_3SYEQBXY7P82ZZD4ME8XKV3311"
      digest: "sha256:02b59559bcde2fe36031c289fe57cd116c6097820082e61e665b37bd2a9d38dd"
      locator: "design.md"
result: "ready"
---
# 计划

每步指回设计的决定（设计见同目录 design.md）。

1. **新模块 `scan_cache.rs`**（设计「决定的方案」1–5 条）：缓存键（sha256，含 CLI 版本与全部扫描参数）、缓存文件读写（`version`+`generatedMs`+`data`，`.tmp` 原子替换）、`LockFileEx` 独占锁、TTL 120 秒、锁等待 15 分钟降级、损坏回退、写后清理超 1 天旧文件。纯函数与 I/O 分离，锁拿放单独封装。
2. **接线 `commands.rs::session_list`**：全历史模式（`active_days==Some(0)` 或 `include_archived`）走缓存包裹；其余模式直扫不碰缓存目录（AC-004 前半）。human 输出走同一份 `data`，保持逐字段等价。
3. **`cli.rs` 加 `--no-cache`**：`session list` 子命令级旗标，直扫不读不写不拿锁。
4. **测试**：
   - 单元：键对参数敏感、TTL 过期判定、损坏文件回退、`--no-cache` 不落缓存文件（AC-002/003）。
   - 并发集成测试（AC-001）：同参数起 3 个真实二进制进程，断言全部退出码 0、输出一致、真实扫描只发生一次（以扫描计数探针或缓存 generatedMs 不变证明）。
   - taxonomy 补 `--no-cache` 解析断言；contract 补「默认窗口不产生缓存文件」断言。
5. **文档口径**（AC-004 后半）：AGENTS.md「AI 操作 CLI」一节、`skill.rs` 内嵌 SKILL.md 同步缓存行为说明。
6. **验证**：`cargo test --manifest-path src-tauri/Cargo.toml`、`--test cli_taxonomy --test cli_contract`、`node --check` 无涉及；构建 `agentwatcher-cli` 二进制跑一次真实全历史 + 一次并发冒烟。
