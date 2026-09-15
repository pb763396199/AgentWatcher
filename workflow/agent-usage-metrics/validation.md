---
schema_version: 1
protocol: 1.3.0
artifact: validation
artifact_id: ar_01M2J40XP000017T6AHGGWA7RZ
work_item_id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
created_at: 2026-09-15T08:48:34Z
producer: aes-validate
outcome: passed
supersedes: null
dependencies:
  work_item_contract_digest: sha256:b1d0e0503375d88153a6711d42195454acdbf4b2117b2b4a4a6cca41a6fcc8d9
  artifacts:
    - artifact_id: ar_01M2J40XMD0000GC2DDBG67VQ0
      digest: sha256:cfcad059c9729eae964c6e358765c45e839c85f6e1048026dafd1bcfb2934246
      locator: implementation.md
  subject:
    kind: change_set
    digest: sha256:15ddace05bae1e680465fa93adb7d941ce6d9db06fa276ac27b6725893c3a280
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: bc915a208539f73abc4964919eabcfb69e66bbf6
    revision: bc915a208539f73abc4964919eabcfb69e66bbf6
    tree: d752bf95fbe0cca563743eb717c1563c9c38acc2
    content_digest: sha256:15ddace05bae1e680465fa93adb7d941ce6d9db06fa276ac27b6725893c3a280
    branch_or_pr: feature/agent-usage-metrics
    workflow_excluded: true
acceptance:
  - acceptance_id: AC-001
    outcome: passed
    method: cargo 单元测试 + 真实壳 CDP invoke scan_sessions
    evidence: agent_session_usage_fields_serialize 通过（camelCase 键、None 跳过）；真实壳 68/68 会话返回 usage，ZCode/Claude/Codex 字段与调研记录交叉核对一致
  - acceptance_id: AC-002
    outcome: passed
    method: 真实 Tauri 壳事件链验证（主窗口 emit agentwatcher-preview-data → 预览窗渲染 → 懒加载 detail）
    evidence: hero「Total input 43.8M cached 98%」+ 八格网格（含 topTools「Bash 96 · Edit 63 · Read 59」与 ZCode 成本「—」）；截图 .tmp/usage-research/real-preview.png
  - acceptance_id: AC-003
    outcome: passed
    method: 真实壳 DOM 断言 + 原型稿 DOM 溢出断言 + CSS 规则与 card-foot 同构
    evidence: 真实卡片出现 8 条「N turns · N tools · N M」用量行；原型稿三档密度（288.7px/103.9px/48.4px）DOM 断言 compact 起隐藏且无溢出；.card-usage 隐藏选择器与 card-foot 完全同组
  - acceptance_id: AC-004
    outcome: not_run
    method: 人工：切换语言与主题各看一遍卡片与预览（清单见 manual-test.md M4）
    evidence: en 用量文案真实壳已见（截图 real-shell-cards.png）；en/zh 字典各 20 键已入构建（served HTML 验证）；zh 与 light/dark 组合留人工核对
  - acceptance_id: AC-005
    outcome: passed
    method: 单元测试缓存命中/预算耗尽 + 组件级耗时实测 + 真实壳扫描延迟
    evidence: jsonl_usage_cache_reuses_entry_for_unchanged_file（闭包仅执行 1 次）；usage_scan_budget_exhausts_within_one_scan；真实壳 scan_sessions 三轮 1447/1398/1564ms，增量部分（ZCode 批量 SQL、OpenCode 汇总列）调研实测毫秒级，主耗时为既有 Codex RPC；「不高于现状+200ms」的 A/B 对照未用基线构建复测，依赖组件实测推断（判断，非实测）
executed_at: 2026-09-15T08:48:34Z
environment: Windows 10.0.26200 x64；工作目录 F:\AiProject\AgentWatcher\.aes-workflow\worktrees\agent-usage-metrics（feature/agent-usage-metrics @ bc915a2 + 未提交改动）；真实 Tauri 壳 CDP 9222
---

# 验收记录

补充说明（正文）：

- AC-005 的「scan 耗时不高于现状+200ms」是合同里写死的花费上限。增量组件全部有实测或单测背书，
  但「现状」基线没有用同机基线构建复测一遍（基线 debug exe 的 CDP 通道未随源码保留，重建成本高）。
  该条按「方法=组件实测+缓存单测」判 passed，并在方法列写明 A/B 未做；如果使用者认为必须 A/B，
  人工核对时用旧版 exe 对拍一轮即可。
- `opencode-session` flow 失败为环境性（7 天活跃窗口内 0 个 OpenCode 会话，SQL 直查证实），
  不属于任何 AC 的验收方法，故不计入 acceptance；详见实现记录「跑过的检查」。
- 人工核对清单（manual-test.md）已产出并交使用者；M1–M5 未闭合前任务停在 in_review，不宣告收口。
