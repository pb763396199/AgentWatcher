---
schema_version: "1"
protocol: "1.3.0"
artifact: "delivery"
artifact_id: "ar_5XZVQQZFQS3WGYJAT1NDMFNKYT"
work_item_id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
created_at: "2026-09-16T07:17:17Z"
producer: "aes-finish"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:32abb9b77345e1c4412d7442f9ae353d1bca154ec66037eeaf403b276872b2ea
  artifacts:
    - artifact_id: "ar_69Y6NNZY913M9KY5TSBJDZNB46"
      digest: sha256:c34a1faf956b7423b2bf64825f94c56eb4f09e06902e1f0994c64627bc0210d0
      locator: "reviews/code-review.md"
    - artifact_id: "ar_29WBZKFYZZPJXMYSQ8VF3JQ70E"
      digest: sha256:ade086ea177f4d55b2cec3d18f13bb3138e49ac04889dd21f9cd58f1ef9f9c55
      locator: "validation.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "de5a9813378a676dde2ffd2ad67c8ad5d995eda4"
    revision: "de5a9813378a676dde2ffd2ad67c8ad5d995eda4"
    tree: "598fe1d1a4da0bb3667ef0b1a50ad98502bb91bb"
    content_digest: "sha256:71cae4bb63af7db87e355b64ce3d0fa1f62d65a7781abd21f5d487949f26a920"
    branch_or_pr: "feature/agent-cli"
    workflow_excluded: true
outcome: "delivered"
landing_revision: "de5a9813378a676dde2ffd2ad67c8ad5d995eda4"
landing_branch: "feature/agent-cli"
---
# 交付（准备阶段）

## 要落地的代码

`feature/agent-cli` 分支六个提交（979d377 → de5a981），基线 3e5ed2c。新增 AI 命令行二进制 `agentwatcher-cli`：session list / show / usage、handoff export、skill install / list / remove；JSON 信封契约；正文按需输出；用法技能内嵌分发。宿主 lib.rs 开放三个查询入口并加 app-server 禁用开关，发布脚本与文档同步。

## 评审与验收

- 代码评审：reviews/code-review.md（范围并入 skill 后的重审版），approved，四条非阻断记录。
- 逐条验收：validation.md，AC-001 到 AC-008 全部通过（含 2026-09-16 补充的 AC-008）；四条 case 锚、三条人工项交清单。
- 人工核对清单：manual-test.md，12 条（查询 8 条加技能安装 4 条），等人测。

## 交付说明

合入 dev 前不改变任何 GUI 行为；发布 zip 自下次 `npm run package:exe` 起多一个 `agentwatcher-cli.exe`，无其他新文件。skill 内容升级走重跑 `skill install`。发版任务（CHANGELOG、SHA256、VSIX 重装验证）不在本任务范围。

## 回滚

`git revert` 六个提交即可，全部为增量文件加可见性改动，无数据迁移、无配置格式变化。
