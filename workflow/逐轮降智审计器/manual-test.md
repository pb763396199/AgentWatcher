---
schema_version: "1"
protocol: "1.3.0"
artifact: "manual-test"
artifact_id: "ar_01M65BVNP30000J2N0TEWEPY4S"
work_item_id: "wi_5VYQZTXYZD0H5M6CN9TKTRFJC3"
created_at: "2026-10-29T11:05:00Z"
producer: "aes-validate"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:6848ab9fb6f89246f58decd1503a13d728d7aa614109393c23af3b6f2ffae203
  artifacts: []
result: "passed"
---

# 人工核对清单

核对方式：用户本人运行打包版本（artifacts/AgentWatcher/AgentWatcher.exe，SHA256 67e9d5cb…）试玩后指示合入 dev，以下条目按试玩范围记录。

- [x] 悬停一张会话卡片，预览用量区下出现「模型链 · N 个信号」一行，点它打开会话分析
- [x] 会话分析里「逐轮审计」表能展开，每轮一行（模型、档位、输出、首 token、信号徽标），时间轴上异常轮有标记
- [x] 右键打包会话，交接包里有 audit/turns.ndjson，每轮一条 JSON
- [x] Copilot CLI 会话显示「该数据源不支持逐轮审计」而不是空白

补充核对（执行者在真实壳完成，用户拍板认可）：中文/英文、深色/浅色各过一遍无破版。
