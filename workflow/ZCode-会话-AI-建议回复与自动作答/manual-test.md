---
schema_version: "1"
protocol: "1.3.0"
artifact: "manual-test"
artifact_id: "ar_01M65VBMNW000Q7J4T0K6XE2RP"
work_item_id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
created_at: "2026-11-04T08:00:00Z"
producer: "aes-validate"
result: "passed"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:2b1c17e9e8d949086387fef35bc752d834b0026f368ec9f17c071b583ecf4d0a
  artifacts: []
---
# 人工核对清单（终版）

用户已逐项确认（2026-11-03/04 多轮真机测试）。

- [x] M1：悬停任意 ZCode 卡片，预览窗顶部出现回复输入框（不限会话状态）
- [x] M2：输入框打字 → 点「发送」→ toast「已发送到会话」→ 消息落库 → 模型回复约 7-20 秒出现
- [x] M3：右键菜单恢复正常（接续会话、复制提示词、打包、自动作答开关）
- [x] M4：点「生成 AI 建议回复」→ 约半分钟建议自动填入输入框（预览窗开着时实时刷新）
