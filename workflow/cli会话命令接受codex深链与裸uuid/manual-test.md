---
schema_version: "1"
protocol: "1.3.0"
artifact: manual-test
artifact_id: "ar_01M74RXCJ7000CMXXZPHHJJ1W4"
work_item_id: "wi_7FY75QZZZZ3Z0YTX8PZVK8N0BN"
created_at: "2026-11-11T07:47:00Z"
producer: "aes-validate"
result: "passed"
dependencies:
  work_item_contract_digest: sha256:ac9ef900a601c9ec2c7772f8a1df16791d2d07a04de86d850b7c7a89ea08446b
  artifacts: []
---
# 人工核对清单

零条目。这次改的是命令行行为，五条验收条件全部由单元测试、契约测试（起真实二进制）和本机真实会话的命令复验覆盖：深链/裸形态命中同一会话、其他 provider 精确匹配不变、not-found 退出码 1，都是跑一条命令读一段输出就能判定的现象，机器验得比人稳，按准入规则不立人工项。用户要亲手感受的话，装上新版后在任意终端粘深链跑一次 `agentwatcher-cli session show "codex://threads/<某条真实会话的uuid>"` 即可，不作为阻塞项。
