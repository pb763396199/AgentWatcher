---
schema_version: 1
protocol: 1.3.0
artifact: delivery
artifact_id: ar_01M2FDFWCZQGZE2NQEYHE4ZV0F
work_item_id: wi_3QQPFJCPXD24P1CCXMXAZ730TQ
created_at: 2026-09-14T07:58:00Z
producer: aes-finish
outcome: delivered
supersedes: null
dependencies:
  work_item_contract_digest: sha256:efa5a5233be0f6e70b70442ff1fb5cb63dad810669e2356c04643038eb83e195
  artifacts: []
  subject:
    kind: change_set
    digest: sha256:b60025e1431c0ee311f478d7afc97e421507a7622c71084518b4a11adc575a21
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: cd721467e82a7199ca0da299071e2b99f33312b7
    revision: 10649708e7168a397c850b38fccf1b31cecebfbd
    tree: 1f344685d607a9bccaacef077f1e1ec3d07fc9be
    content_digest: sha256:b60025e1431c0ee311f478d7afc97e421507a7622c71084518b4a11adc575a21
    branch_or_pr: dev
    excluded_prefixes: []
    workflow_excluded: true
---

# 交付：ZCode Provider v0.1.5

落地版本：dev 分支 `1064970`（feat 7dd5778 + workflow f6c477d + release 1064970），
tag `v0.1.5`，GitHub Release 上传 `AgentWatcher-v0.1.5-windows-x64.zip`
（SHA256 `EC356AF8FABAECE38FA8E1C75DB1B1D19C43BDC902C17C76E324AF22EF653DE7`）。

回退方式：`git revert 1064970 f6c477d 7dd5778`（或回退到 `cd72146`）；
发布的 zip 不自更新，旧版本 zip 仍可用。
