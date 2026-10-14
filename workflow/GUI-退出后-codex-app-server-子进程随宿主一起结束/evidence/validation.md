---
schema_version: 1
protocol: 1.3.0
artifact: validation
artifact_id: ar_01M4WQWTB400096JX0NYV0R27N
work_item_id: wi_5SYZXNKVTFYTV99EVBBBMJK0S6
created_at: 2026-10-14T16:40:00Z
producer: aes-validate
outcome: passed
supersedes: null
contract_revision: 2
verified_at_commit: 600df7183d583146885548753268fa38bac740f7
executed_at: "2026-10-14T16:20:00Z"
environment: "Windows 11 10.0.26200；worktree 隔离真实 Tauri 壳（devUrl 1421，主检出 dev 常驻 1420 构成双实例）；进程核对用 PEB 读当前目录脚本"
dependencies:
  work_item_contract_digest: sha256:2bf0546a812236ff05d377823c4cea07a5a43bdabe190a850ba43a1afb2433e4
  artifacts: []
acceptance:
  - acceptance_id: AC-001
    outcome: passed
    method: 读代码定位 spawn 链与两条退出路径；结合 2026-10-14 两台测试壳的孤儿进程证据（PEB 读 CWD 命中 node/codex.exe）核实机制
    evidence: builds/1.md 调试记录节：假设表五条各带验证手段，两条泄漏路径证实（只杀直接子进程；强杀无清理），引入提交 28a8a65
  - acceptance_id: AC-002
    outcome: passed
    method: worktree 起真实壳（扫描自动拉起 app-server，PEB 扫描确认 cmd→node→codex.exe 三层在场）后分别做温和关闭（taskkill 不带 /F）与强杀（taskkill /F），每次退出后按 CWD 全量扫描
    evidence: 实验 A：温和关闭 4 秒后 src-tauri 下零存活进程；实验 B：强杀 5 秒后同样零存活（作业 kill-on-close 兜底），两次实验间隔重新起壳重新拉起 app-server
  - acceptance_id: AC-003
    outcome: passed
    method: cargo test 全量加 CLI 两组；两轮隔离真实壳跑冒烟流程
    evidence: cargo test 99 过 1 忽略（含新增回归测试 job_handle_drop_terminates_whole_descendant_tree）；cli_taxonomy 与 cli_contract 各 8 过；两轮 smoke 均通过：冒烟流程，结果文件在 worktree .tmp/tauri-realtest/20261014-155543 与 20261014-16xx 目录
---

# 验收：常驻子服务 kill-on-close 作业

三条验收全部通过。AC-002 的两条退出路径都在真实壳上以「先证明服务树在场、再退出、再证明清零」的方式验证；扫描行为无回退由两轮冒烟与 Codex 会话加载共同佐证。
