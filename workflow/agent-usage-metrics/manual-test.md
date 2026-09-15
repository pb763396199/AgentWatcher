---
schema_version: 1
protocol: 1.3.0
artifact: manual-test
artifact_id: ar_01M2J40XPS000EAEWSJQ0YX8F7
work_item_id: wi_01M2HX0EAATCGJZXB8KG6VNAXB
created_at: 2026-09-15T08:48:34Z
producer: aes-validate
result: passed
supersedes: null
dependencies:
  work_item_contract_digest: sha256:b1d0e0503375d88153a6711d42195454acdbf4b2117b2b4a4a6cca41a6fcc8d9
  artifacts:
    - artifact_id: ar_01M2J40XMD0000GC2DDBG67VQ0
      digest: sha256:534f79f43e09f1c41f80a8eb05c71157e0e8680823f69b4df0fbb7b89a5f165a
      locator: implementation.md
---
# 人工核对清单

写给人看的验收清单。在 `F:\AiProject\AgentWatcher\.aes-workflow\worktrees\agent-usage-metrics`
目录跑 `npm run dev`，等卡片出来后按顺序操作。改 `[ ]` 为 `[x]` 或 `[!]` 就算反馈，
`[!]` 请在下面缩进写现象。

- [X] M1 主窗口每张卡片左下角出现一行紧凑用量（如「8 轮 · 243 工具 · 42.9M」），数字随会话活动每 15 秒左右增长；没有用量的会话这一行整体不出现
- [X] M2 鼠标悬停任意 ZCode 或 Claude 卡片，预览窗出现「用量」区块：顶部大数字是累计输入，旁边有「缓存 N%」；下方两列小格有累计输出、当前上下文、对话轮次、工具调用（下方有一行小字工具排行）、模型调用、模型、时长、成本
- [X] M3 悬停一张 OpenCode 卡片（如有），成本一格显示美元金额；悬停 ZCode 卡片，成本一格显示「—」而不是 0；Copilot CLI 卡片的预览用量区显示「该数据源不提供用量数据」
- [X] M4 设置里把语言切到中文再切回英文、主题切 light 再切 dark：用量区块的标签和单位跟随语言，配色在两套主题下都清晰可读，布局不破
- [X] M5 拖动「每排数量」滑杆把卡片缩到很小：用量行和分支名一起消失，标题仍在；再缩到最小只剩徽章；调回默认后用量行恢复

## 现象记录

（人改文件就是反馈：把 `[ ]` 改成 `[x]` 或 `[!]`，`[!]` 在对应条目下缩进写现象。）
