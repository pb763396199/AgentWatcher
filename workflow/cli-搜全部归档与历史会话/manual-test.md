---
schema_version: 1
protocol: 1.3.0
artifact: manual-test
artifact_id: ar_01M5C8QHKK00036AN2CCJTH89A
work_item_id: wi_01M5C61SH5000FME6B0YSBBTDE
created_at: 2026-10-20T09:20:00Z
producer: aes-validate
result: pending
supersedes: null
dependencies:
  work_item_contract_digest: sha256:66c5eb385a5e74388b6cd2c4a87f41f8d92e69603379b6bb23da5196597a30de
  artifacts: []
---

# 人工核对：CLI 全历史与归档会话搜索

列表与三条查询命令的机器验证都在真实数据上跑过（见 `evidence/validation.md`）。下面这一条是机器替不了的：它要在部署了新 CLI 的环境里，由你本人判断结果可用不可用。

- [ ] 把新 `agentwatcher-cli.exe` 部署到 PATH（合入 dev 后跑 `npm run package:exe` 或手动替换），再在任意 AI 助手里让它"用 agentwatcher 技能搜一下我六月的 codex 会话"：确认它能列出几个月前的会话、点开一个能看到摘要，并且等结果的时间你能接受
      预期是列表十秒左右出全量（万级库）、单个会话详情秒级。如果等了一分钟以上或列表缺了你知道存在的会话，在这里写 [!] 并注明是哪个会话。
