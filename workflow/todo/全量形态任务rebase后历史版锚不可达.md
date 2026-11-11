---
schema_version: 1
protocol: 1.3.0
record: todo
id: td_01M74B0Q2K0008XKM4FTQ9MBQX
status: open
created_at: 2026-11-11T11:50:00+08:00
repository: https://github.com/pb763396199/AgentWatcher.git
title: 全量形态任务 rebase 后历史版证据锚永远证不出
---

# 全量形态任务 rebase 后历史版证据锚永远证不出

任务 `Codex-生图-CLI-命令`（mbvtfbzr，已 done）合入 dev 后，`merge-check` / `validate` 对它持续报三份记录"证据锚点不可达，等价与机械重锚都证不出"：两份是被 `superseded` 的历史版（reviews/code-review-v2.md、validation/validation-v2.md），一份是终版 delivery-v3（锚定合入提交 69e674c，其后的 closeout 提交只改 workflow/，代码未动）。

根因是组合边界：全量形态的 `change_set` 锚走逐提交三方重放等价判定，rebase 洗掉原提交链后，历史版锚在数学上无法证明；closeout 提交推进 HEAD 后终版锚又会被下一轮判定判旧。契约形态的两锚（可达 + 到 HEAD 代码零改动）没有这个问题——GUI 退出后 codex app-server 任务（契约形态）合入后检查一直干净。

现状不影响功能与合入事实：代码已落地 dev（69e674c），全部测试绿。处置方向二选一：awf 工具对"superseded 历史版 + closeout-only 提交"放宽判定（工具仓库的事）；或本仓库全量形态任务在 rebase 前先转契约形态。
