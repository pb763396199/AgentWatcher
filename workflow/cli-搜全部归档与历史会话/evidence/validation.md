---
schema_version: 1
protocol: 1.3.0
artifact: validation
artifact_id: ar_01M5C8HPRW000681XEPNG7W9HV
work_item_id: wi_01M5C61SH5000FME6B0YSBBTDE
created_at: 2026-10-20T09:15:00Z
producer: aes-validate
outcome: passed
supersedes: null
contract_revision: 1
verified_at_commit: 39962ef838af0abb0f1e502d36490995ccca908c
executed_at: 2026-10-20
environment: Windows 10.0.26200；真实用户数据（ZCode 291 / OpenCode 15,872 / Claude 543 文件 / Codex sessions+archived_sessions 1,385 归档文件）；worktree 内 debug 与 release 双构建
dependencies:
  work_item_contract_digest: sha256:66c5eb385a5e74388b6cd2c4a87f41f8d92e69603379b6bb23da5196597a30de
  artifacts: []
acceptance:
  - acceptance_id: AC-001
    outcome: passed
    method: 对 OpenCode 库取 time_archived 非空的全量 ID（63 个），与 `session list --provider opencode --include-archived --active-days 0 --limit 20000` 的输出求差集
    evidence: 归档可见 63/63，缺失 0；Codex 归档目录主线程 178 个全部入列（含 .jsonl 与 .jsonl.zst）
  - acceptance_id: AC-002
    outcome: passed
    method: `session list --active-days 0 --include-archived --limit 20000` 与各数据源总量对照；对 Codex 归档目录按文件名解析唯一 origin thread、与列表差集再逐个解压验 subagent 标记；对 Claude 把缺失会话按项目目录分类
    evidence: OpenCode 15,872/15,872（含 63 归档）；ZCode 171 = 291 − 119 个 subagent_child − 1（临时目录黑名单）；Codex 303 = 125 活跃 + 178 归档主线程，1,194 个缺失全部带 source.subagent.thread_spawn 标记（抽样 61 个文件 42 个 subagent 印证）；Claude 350/350 非临时目录会话，317 个缺失全部位于 %TEMP% 工作区项目（默认黑名单）；137 天前的 zst 会话在列
  - acceptance_id: AC-003
    outcome: passed
    method: 取 137 天前的 codex zst 会话与 12 天前的 zcode 会话，依次跑 session show / session usage / handoff export
    evidence: codex:019e83a4…（137d）三连成功，show 带完整 lastUser/lastAi 预览（定向全量解压）；zcode:sess_da8cc039…（12d，默认 7 天窗口外）show/usage 成功，usage 返回真实 token 数（毛输入 1,080,352）
  - acceptance_id: AC-004
    outcome: passed
    method: 对归档目录 zst 会话跑 `session show --content` 与 `handoff export`，检查路径可读性
    evidence: content.rawSessionPath 指向解压副本（%TEMP%\AgentWatcher\handoff-sources\codex\…jsonl），head -c 验证为可解析 JSONL（首行 session_meta）；rawSessionArchive 保留原始 zst 路径；handoff export 输出同一解压副本路径
  - acceptance_id: AC-005
    outcome: passed
    method: 机器锚点 `cargo test --lib test_resolve_scan_options_defaults_preserved`；新旧 release 二进制默认 `session list` 对照（总数与耗时）
    evidence: 单测通过（默认 80/7 天/隐藏归档/240 上限逐字段断言）；旧版（0.1.7）与新版默认列表同为 129 条，5.3s vs 5.6s；全量测试 102+8+9 全绿
---

# 验收：CLI 全历史与归档会话搜索

五条验收全部通过。补充事实：release 版最重路径（全历史+归档 16,698 条）10.9s，定向 show（42MB zst 全解压）5.9s，默认路径与旧版无差异。已知边界（不算失败）：zst 会话在列表中无预览文本与精确 messageCount（定向 show 补全）；老 rollout 采样段无 token 事件时 usage 为 null；Codex 归档 usage 聚合未做（范围外）。
