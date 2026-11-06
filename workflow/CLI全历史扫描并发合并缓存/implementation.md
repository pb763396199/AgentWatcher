---
schema_version: "1"
protocol: "1.3.0"
artifact: "implementation"
artifact_id: "ar_09NZZFFZQZKBDX6XQ8A864ZMCS"
work_item_id: "wi_3NM7QSRDWZHE0QCAXWTDJ2VNPX"
created_at: "2026-11-06T03:29:28Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:94c4459d059aef000a97f13dc3c6265f90517a921ef86a9ee550227bacefb717"
  artifacts:
    - artifact_id: "ar_45N6ZFYYYZ9GZX2CG67QYDEPJQ"
      digest: "sha256:a24da4414f6a1a909370e7055d8033158675466583f1d90e194847f1249ac559"
      locator: "change-note.md"
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

# 实现回执

## 做了什么

按计划六步全部完成，提交 c67331b（9 文件，+501/-34）。核心：`scan_cache.rs` 提供
`cached_full_history_scan(key, scan)`——拿锁 → 缓存新鲜则读 → 否则扫描并原子写 → 放锁；
锁等待超 15 分钟降级直扫；损坏/版本不符静默弃用；写后清理超 1 天旧文件。

## 哪里没按计划走

1. **缓存键不含 status/workspace**（设计原文含）。实现时改为缓存未过滤的全量投影、
   status/workspace 过滤在命中后内存执行——命中率更高（同参数不同过滤的组合共享一次扫描），
   且过滤逻辑只有一份。投影项带内部键 `workspaceFallback` 保持原过滤语义（workspacePath
   缺省回退 session.workspace），输出前剥掉。
2. **命中缓存与真实扫描的输出不是逐字节相等**：costUsd 等 f64 字段经 JSON 字符串往返后
   末位十进制表示可能漂 1 ulp（serde_json 解析快速路径的精度特性，实测
   0.056862974999999996 → 0.056862975，相邻两个 f64）。语义等价，消费方按浮点读无影响。
   强约束「逐字段等价」按此放宽为「字段集合与语义等价、浮点末位允许 1 ulp 表示差」。
3. human 渲染从 `AgentSession` 结构改为读投影 JSON 字段（缓存路径下没有结构体可用），
   输出列与格式不变；顺带删除了不再使用的 `workspace_matches` 与 `session_time_label`。

## 跑过的检查

- `cargo test --manifest-path src-tauri/Cargo.toml`：lib 146 过、bin 单元 4 过、cli_contract 9 过、cli_taxonomy 10 过。
- 真实二进制冒烟（本机，17297 个会话）：
  - 冷扫描 59.5s；缓存命中 0.76s（78 倍）；`--no-cache` 64.5s。
  - 并发实测：1 个预热进程扫描中 + 3 个并发进程，总耗时 66s（四倍叠加应 >180s），
    四进程 total 均 17297、id 序列完全一致、缓存目录只 1 个 json 文件。
  - `--format human` 缓存路径渲染正常。
- 全部证据数字来自 2026-11-06 本机实跑。
