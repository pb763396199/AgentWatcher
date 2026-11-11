---
schema_version: "1"
protocol: "1.3.0"
artifact: "change-note"
artifact_id: "ar_1BQQSXZVDYWZFEVT2Q1EAEPP8Y"
work_item_id: "wi_01M71Q3R0W0004MNNQHHS0ZSZ1"
created_at: "2026-11-10T04:03:02Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:08218fcaad2d9ec21a92a9335bb0cea91f075df699314c2971d7d17dfd158939
  artifacts:
    - artifact_id: "ar_01M71QFY8X000C5SGDAPGSYSF9"
      digest: sha256:42b39ee9f951482c92db71eb8ff834c60b341ae9fa32eedd0bfe734be54e0d4f
      locator: "plan.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "34600a6a84017e628673841d74ef71c744af8dbb"
    revision: "69e674cdbb3ae7e9fbc1f8235285da8cf5ad2cdf"
    tree: "d296ab2e610c0b1f557befa5a08c9651feba3e9f"
    content_digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
    branch_or_pr: "feature/Codex-生图-CLI-命令"
    workflow_excluded: true
result: "superseded"
---
# 修改说明

给 agentwatcher-cli 加 `imagegen generate`：经 codex exec 触发 Codex 内置 image_gen 工具生图，所有 AI 助手可通过这一条命令获得生图能力，不需要 `OPENAI_API_KEY`，也不依赖各自宿主有没有生图工具。生图模型由 Codex 云端自动选择，命令面与实现代码不出现任何 gpt-image 版本号（AC-003 有单测锁定），云端升级时命令行为自动跟随——这是"不跟版本绑定"的实现方式。提示词双来源：系统 `%USERPROFILE%\.codex\skills\.system\imagegen\SKILL.md`（尊重 `CODEX_HOME`）在场时由 Codex 编排模型自读最新版（实测它本来就会读）；缺失时注入二进制内嵌的固化副本（`src-tauri/src/imagegen_skill/SKILL.md`，2026-11-10 本机版逐字节冻结）。

依据：用户 2026-11-10 委派（见 work-item 原始请求）；codex-cli 0.167.0 通道实测（见 implementation 实测节）。未解释的改动：无。

<!-- 2026-11-11 指针维护：rebase（amend）洗掉原锚定提交，代码净补丁经 git patch-id 比对与审查版一致，锚点行按协议 rebase 重锚教义机械改写到重放后的最后代码提交。证据正文未改动。 -->
