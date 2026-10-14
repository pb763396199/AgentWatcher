---
schema_version: 1
protocol: 1.3.0
artifact: review
artifact_id: ar_01M4WQWT8Y000F351BN8R7PXAG
work_item_id: wi_5SYZXNKVTFYTV99EVBBBMJK0S6
created_at: 2026-10-14T16:36:00Z
producer: aes-review
verdict: approved
supersedes: null
contract_revision: 2
verified_at_commit: 600df7183d583146885548753268fa38bac740f7
review_type: code
reviewers: []
dependencies:
  work_item_contract_digest: sha256:2bf0546a812236ff05d377823c4cea07a5a43bdabe190a850ba43a1afb2433e4
  artifacts: []
---

# 代码评审：常驻子服务 kill-on-close 作业

**本评审由写这段代码的同一个 agent 做（自审），按流程在此声明。** 改动 274 行（lib.rs 为主），逐块读过：作业辅助、两处 spawn 接入、退出钩子、回归测试。

## 结论

通过。生命周期绑定用了 Windows 的标准机制，两条退出路径都有覆盖，扫描口径未动。

## 审查中抓到的问题

| 位置 | 问题 | 严重度 | 处理 |
| --- | --- | --- | --- |
| 无 | 未发现阻断问题 | - | - |

## 已核实的事实

- CREATE_SUSPENDED 与 AssignProcessToJobObject 之间进程不可能派生子进程，孙进程（node、codex.exe）全部继承作业；恢复用线程快照，此时进程只有主线程一个，不存在恢复错线程的问题（代码注释已写明依据）。
- 作业句柄的 `unsafe impl Send/Sync` 有依据：句柄仅经服务互斥锁串行使用，CloseHandle 线程安全，静态 OnceLock 存储要求跨线程。
- 失败路径全部收敛：CreateJobObject/SetInformation/Assign 任一失败都会 kill+wait 子进程并关句柄，不留挂起进程；`wait_for_codex_app_server_ready` 超时路径 kill 后显式 drop 作业。
- 正常退出 `TerminateJobObject` 后仍 `child.kill()+wait()` 收尸直接子进程句柄，不留僵尸。
- 扫描口径未变：app-server 的拉起条件、重用判断（ready 探测）、thread/list 调用一字未动；CLI 的 `set_codex_app_server_disabled` 语义未动。
- 回归测试三段式：孙进程出现 → 杀直接子进程证明孙进程存活（旧机制）→ drop 作业句柄证明整树消失（新机制），断言双向。

## 非阻断问题

- 恢复主线程用快照遍历全系统线程，O(进程数)；只在拉起服务时调用一次，可接受。
- `codex-windows-sandbox-ser`（Codex 自带的 Session 0 服务）能否被作业带走取决于 Codex 自身的派生方式，未纳入断言；两轮真实壳实验后未观察到残留，风险记入施工回执。

## 没覆盖的范围

- 崩溃退出（宿主 panic）未单独实验；机制上与强杀同一路径（系统关句柄），由作业保证。
- 非 Windows 平台未测（仓库数据源与发布面本就仅 Windows）。
