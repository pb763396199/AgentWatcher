---
schema_version: 1
protocol: 1.3.0
artifact: manual-test
artifact_id: ar_01M4W49RND000DVSABWVTVFB7M
work_item_id: wi_19X7WBYXTZK9DH6X1KT55XHK7N
created_at: 2026-10-14T10:48:00Z
producer: aes-validate
result: passed
supersedes: null
dependencies:
  work_item_contract_digest: sha256:98aa641eac6856359c6ac518be14749ff547a9c78327586b62d542ed67347bc4
  artifacts: []
---
# 人工核对：搜索栏按会话 ID 过滤

改动合入前在真实窗口里点三下。代码在分支 `feature/主窗口搜索栏支持用会话-ID-过滤卡片`（提交 `5387854`）。

- [x] 复制一段会话 ID（例如 `01a1360f-7573-7ed0-b25b-35cddae9eeaf` 这样不带前缀的部分，可从终端或 `agentwatcher-cli session list` 输出里拿），粘进 AgentWatcher 主窗口顶部搜索框：列表只剩这一张（或 ID/标题同样包含该片段的几张）卡片
- [x] 把同一段 ID 片段改成全大写再粘一次：看到的卡片和上一步完全一样
- [x] 清空搜索框：全部卡片恢复显示，各状态筛选按钮的数字回到搜索前的值
