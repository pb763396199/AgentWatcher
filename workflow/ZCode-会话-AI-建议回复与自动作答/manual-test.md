---
schema_version: "1"
protocol: "1.3.0"
artifact: "manual-test"
artifact_id: "ar_01M65VBMNW000Q7J4T0K6XE2RP"
work_item_id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
created_at: "2026-10-30T07:52:00Z"
producer: "aes-validate"
result: "pending"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:2b1c17e9e8d949086387fef35bc752d834b0026f368ec9f17c071b583ecf4d0a
  artifacts: []
---
# 人工核对清单（零配置版）

按顺序做，改这个文件就是反馈：`[ ]` 没测，`[x]` 过了，`[!]` 没过并在下一行写现象。

- [ ] M1：等任意一个 ZCode 会话停下来等你回复（或在 ZCode 里让它问你一个带选项的问题），约半分钟后看 AgentWatcher 那张卡片下方自动出现灰色建议（带选项时是一排可点的选项按钮）；中英文、深浅色各看一遍
- [ ] M2：点建议行上的「发送」，切到 ZCode 确认这句话出现在原会话里并且 agent 接着往下跑，AgentWatcher 卡片状态离开"待回复"（顺带看 ZCode 窗口是否实时刷新）
- [ ] M3：对一张等待中的 ZCode 卡片点右键，确认有「换一条建议」和「自动作答此会话的选项」两项且设置面板没有任何 AI 建议配置；点「自动作答」后让该会话再问你一个带选项的问题，确认它被自动答了一次，%TEMP%\AgentWatcher\auto-answers\audit.jsonl 里多了一行
- [ ] M4：右键「换一条建议」点两次，确认每次都出不同建议且第三次提示到上限

预计十分钟以内。M1/M3 需要真实的等待中 ZCode 会话；建议生成走你 ZCode 自己的账号，每次约半分钟。
