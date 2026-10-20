---
schema_version: 1
protocol: 1.3.0
artifact: manual-test
artifact_id: ar_01M5C8QHKK00036AN2CCJTH89A
work_item_id: wi_01M5C61SH5000FME6B0YSBBTDE
created_at: "2026-10-20T09:20:00Z"
producer: aes-validate
result: passed
supersedes: null
contract_revision: 1
verified_at_commit: 141fb7659875fa25e8fd4bc6aa8e6b437ff8c005
dependencies:
  work_item_contract_digest: sha256:66c5eb385a5e74388b6cd2c4a87f41f8d92e69603379b6bb23da5196597a30de
  artifacts: []
---

# 人工核对：CLI 全历史与归档会话搜索

- [x] 把新 `agentwatcher-cli.exe` 部署到 PATH（合入 dev 后跑 `npm run package:exe` 或手动替换），再在任意 AI 助手里让它"用 agentwatcher 技能搜一下我六月的 codex 会话"：确认它能列出几个月前的会话、点开一个能看到摘要，并且等结果的时间你能接受
      预期是列表十秒左右出全量（万级库）、单个会话详情秒级。如果等了一分钟以上或列表缺了你知道存在的会话，在这里写 [!] 并注明是哪个会话。

      用户授权由 subagent 代测（2026-10-20，代理不带实现上下文，只凭技能文档 + 用户原话执行）。结果：部署后按"搜六月 codex 会话"独立完成，找到 93 条（含 zst 归档），选中 `codex:019e837b-fdda-71e0-96e7-52abdd98b6fa`，sessionPath 路径 `...\codex\sessions\2026\06\01\rollout-2026-06-01T...jsonl.zst` 直接证实是 2026-06-01 的会话；show 预览级与 handoff 导出（解压副本 876,885 字节）全部成功。耗时：最优路径约 20 秒（判据"流畅"档），含两次文档口径踩坑 31.5 秒（"可接受"档）——踩坑点（timeLabel 相对天数、列表缺绝对时间戳）已当轮修复（提交 141fb76）并复验（updatedMs 直接过滤 92 条六月会话）。测试过程与逐命令耗时见 `builds/2.md`。

      如需本人再验：在任意 AI 助手里说"用 agentwatcher 技能搜一下我六月的 codex 会话"即可复现同一路径。
