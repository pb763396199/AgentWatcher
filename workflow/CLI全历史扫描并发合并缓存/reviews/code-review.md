---
schema_version: "1"
protocol: "1.3.0"
artifact: "review"
artifact_id: "ar_07W6ZZKZZE95HXMMP8VT2W474P"
work_item_id: "wi_3NM7QSRDWZHE0QCAXWTDJ2VNPX"
created_at: "2026-11-06T03:31:04Z"
producer: "aes-review"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:94c4459d059aef000a97f13dc3c6265f90517a921ef86a9ee550227bacefb717"
  artifacts:
    - artifact_id: "ar_09NZZFFZQZKBDX6XQ8A864ZMCS"
      digest: "sha256:e54814df0b6c75b77bdd32b56938bb36e045d7f729dbf399a5c7ef9b1a895cdc"
      locator: "implementation.md"
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
verdict: "approved"
review_type: "code"
reviewers: []
---
# 代码评审（code）

## 看了什么

提交 c67331b 全部 9 个文件，重点 `scan_cache.rs`（新）、`commands.rs::session_list` 重排、
`cli.rs`/`main.rs` 接线、两个测试文件、文档口径。

## 逐项结论

- 正确性：锁拿放配对（Drop 释放，句柄关闭兜底）；`.tmp` + rename 原子替换，Windows 下
  `fs::rename` 带 REPLACE_EXISTING；损坏/版本不符/目录建不出来全部降级直扫，退出码不受影响。
- 契约：信封、LIST_KEYS、human 列、退出码零变化；`workspaceFallback` 内部键在输出前剥掉，
  已由契约测试与 human 冒烟覆盖。
- 并发：后到进程阻塞在 `LockFileEx`（100ms 轮询 + 15 分钟上限），不会双写坏文件
  （原子替换保证最坏只是重复扫描一次）。
- 依赖：未新增 crate，只启用 windows-sys 的 `Win32_System_IO` feature。

## 已识别的不修项（记录在案）

1. **prune 可能删掉其他在飞扫描持有的锁文件**（>1 天的 .lock，且持锁扫描通常分钟级结束，
   实际不可能触发）。即使发生：双方各持各的文件锁、各自扫描、原子写同一缓存文件，
   最坏浪费一次 I/O，无数据损坏。不修。
2. **锁竞争之外的真实错误会轮询到 15 分钟上限**（如句柄异常时 LockFileEx 持续失败）。
   触发条件是 OS 级故障，届时进程还有 TTL/直扫兜底之外的退出路径吗——有：
   超时后降级直扫。可接受。
3. f64 末位 1 ulp 表示差（见验证记录补充说明），语义等价，不修。

## 结论

approved。无阻断问题；三条不修项均已给出触发条件与最坏后果。
