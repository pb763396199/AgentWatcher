---
schema_version: "1"
protocol: "1.3.0"
artifact: "validation"
artifact_id: "ar_21WQZFHWQYKHGR3PQXJT7NXN9Z"
work_item_id: "wi_3NM7QSRDWZHE0QCAXWTDJ2VNPX"
created_at: "2026-11-06T03:30:52Z"
producer: "aes-validate"
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
outcome: "passed"
acceptance:
  - id: "AC-001"
    outcome: "passed"
    method: "case:test_concurrent_full_history_scan_merges + 真实二进制并发冒烟"
    evidence: "单元：4 线程计数闭包只执行 1 次、结果一致。真实：1 预热 + 3 并发进程总耗时 66s（单次冷扫描 59.5s，四倍叠加应 >180s），四进程 total 均 17297、id 序列一致，缓存目录仅 1 个 json"
    executed_at: "2026-11-06"
    environment: "Windows 11 26200，本机 17297 会话数据源"
  - id: "AC-002"
    outcome: "passed"
    method: "case:test_cache_ttl_expiry_and_no_cache_flag + 真实二进制计时"
    evidence: "单元：TTL 内命中不扫描、TTL 0 过期重扫。真实：命中 0.76s，--no-cache 64.5s（等价冷扫描）"
    executed_at: "2026-11-06"
    environment: "同上"
  - id: "AC-003"
    outcome: "passed"
    method: "case:test_corrupt_cache_falls_back"
    evidence: "写入非法 JSON 缓存后调用，直接扫描返回正确结果（退出码 0），随后正确结果覆盖缓存、下次命中"
    executed_at: "2026-11-06"
    environment: "单元测试"
  - id: "AC-004"
    outcome: "passed"
    method: "case:test_default_scan_uncached_and_contract_tests + case:default_window_session_list_leaves_no_scan_cache"
    evidence: "默认窗口 list 前后缓存目录文件数不增；cargo test 全绿（lib 146 / bin 4 / cli_contract 9 / cli_taxonomy 10），含『列表不得含 lastUserMessage/lastAiMessage』既有断言"
    executed_at: "2026-11-06"
    environment: "本机"
---
# 验收

四条验收标准逐条验证，全部通过。方法、证据、环境见头部 `acceptance` 机器字段。

补充说明（对应实现回执「哪里没按计划走」第 2 条）：命中缓存与真实扫描的输出存在
f64 字段末位 1 ulp 的十进制表示差（costUsd，实测 0.056862974999999996 与
0.056862975，相邻两个 f64），字段集合与其余字段完全一致。这是 JSON 字符串往返的
固有特性，消费方按浮点数读取时数值语义相同（美元成本到小数点后 10 位）。
