---
schema_version: 1
protocol: 1.3.0
artifact: validation
artifact_id: ar_01M5C9MDDM0008ENGAJH9KEX5T
work_item_id: wi_01M5C61SH5000FME6B0YSBBTDE
created_at: 2026-10-20T10:15:00Z
producer: aes-validate
outcome: passed
supersedes: [ar_01M5C8HPRW000681XEPNG7W9HV]
contract_revision: 1
verified_at_commit: 141fb7659875fa25e8fd4bc6aa8e6b437ff8c005
executed_at: 2026-10-20
environment: Windows 10.0.26200；真实用户数据（ZCode 291 / OpenCode 15,872 / Claude 543 文件 / Codex 归档 1,385 文件）；worktree 内 debug 与 release 双构建；第二轮锚点起部署于 PATH（~/.local/bin/agentwatcher-cli）
dependencies:
  work_item_contract_digest: sha256:66c5eb385a5e74388b6cd2c4a87f41f8d92e69603379b6bb23da5196597a30de
  artifacts: []
acceptance:
  - acceptance_id: AC-001
    outcome: passed
    method: 对 OpenCode 库取 time_archived 非空的全量 ID（63 个），与 `session list --provider opencode --include-archived --active-days 0 --limit 20000` 输出求差集
    evidence: 归档可见 63/63，缺失 0；Codex 归档目录主线程 178 个全部入列（含 .jsonl 与 .jsonl.zst）
  - acceptance_id: AC-002
    outcome: passed
    method: 全历史+归档列表与各数据源总量对照；Codex 归档按文件名解析唯一 origin thread 后逐个解压验 subagent 标记；Claude 缺失会话按项目目录分类
    evidence: OpenCode 15,872/15,872；ZCode 171 = 291 − 119 subagent_child − 1（临时目录黑名单）；Codex 303 = 125 活跃 + 178 归档主线程，1,194 缺失全部带 source.subagent.thread_spawn；Claude 350/350 非临时目录会话，317 缺失全在 %TEMP% 项目；137 天前的 zst 会话在列。第二轮锚点复验 total 16,698、10.6s
  - acceptance_id: AC-003
    outcome: passed
    method: 取 137 天前的 codex zst 会话与 12 天前的 zcode 会话，依次跑 session show / session usage / handoff export
    evidence: codex:019e83a4…（137d）三连成功，show 带完整预览；zcode:sess_da8cc039…（12d）show/usage 成功（毛输入 1,080,352）
  - acceptance_id: AC-004
    outcome: passed
    method: 对归档目录 zst 会话跑 `session show --content` 与 `handoff export`，检查路径可读性
    evidence: content.rawSessionPath 指向解压副本且首行为可解析 session_meta JSONL；rawSessionArchive 保留原始 zst；export 输出同一解压副本
  - acceptance_id: AC-005
    outcome: passed
    method: 机器锚点 `test_resolve_scan_options_defaults_preserved`；新旧 release 二进制默认 `session list` 对照；第二轮增量后 cli_taxonomy / cli_contract 重跑
    evidence: 单测通过；旧版（0.1.7）与新版默认列表同为 129 条（5.3s vs 5.6s）；第二轮后 9 + 8 合同测试全绿，默认列表 129 条且每条带 updatedMs
---

# 验收：CLI 全历史与归档会话搜索（覆盖第二轮增量）

五条验收全部通过。本版锚点刷到第二轮提交（列表补 `updatedMs` + SKILL.md 时间口径），在部署于 PATH 的 release 二进制上复验关键路径：默认 129 条、全历史+归档 16,698 条 10.6s、codex 六月会话用 `updatedMs` 直接过滤 92 条。

委托人测（用户授权 subagent、不带实现上下文、仅凭技能文档）结果：按"搜六月 codex 会话"的真实请求，独立找到 93 条（天数近似口径），选中会话经 sessionPath（`...\codex\sessions\2026\06\01\rollout-2026-06-01T...jsonl.zst`）硬证据确认，show 预览与 handoff 导出成功（解压副本 876,885 字节），最优路径约 20s、含踩坑 31.5s，判据结论"可接受"及以上。踩坑点（timeLabel 相对天数、列表无绝对时间戳）已由第二轮修复并复验。

已知边界（不算失败）：zst 会话在列表中无预览文本与精确 messageCount（定向 show 补全）；老 rollout 采样段无 token 事件时 usage 为 null；Codex 归档 usage 聚合未做（范围外）。
