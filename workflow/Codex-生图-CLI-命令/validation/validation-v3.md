---
schema_version: 1
protocol: 1.3.0
artifact: validation
artifact_id: "ar_01M74A16SE0005HRYZMNP5J5F9"
work_item_id: wi_01M71Q3R0W0004MNNQHHS0ZSZ1
created_at: 2026-11-11T11:05:00+08:00
producer: aes-validate
supersedes: "ar_01M7493RPB000A2QT23S1SVD9J"
dependencies:
  work_item_contract_digest: sha256:08218fcaad2d9ec21a92a9335bb0cea91f075df699314c2971d7d17dfd158939
  artifacts:
    - artifact_id: ar_15QZ5SZWH6YBJ6AKRSHHY9CTRN
      digest: sha256:3f833e4fde1bfab6461f52918f02666c2959e6ed59c739aebb482c8c186e9eef
      locator: implementation.md
    - artifact_id: ar_01M71TFMK90002TF44G3KGPK44
      digest: sha256:bfa635181574f7a479038e18209e3a0bd508605319d95bc4179bd8bc80511d3f
      locator: reviews/code-review-v2.md
  subject:
    kind: change_set
    digest: sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: "34600a6a84017e628673841d74ef71c744af8dbb"
    revision: "6df0e00a848f20e9ce655a31fd85eef6ffdc1790"
    tree: "660c9b8c488fa7b46a49e2f15c4cef725305baa9"
    content_digest: sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945
    branch_or_pr: feature/Codex-生图-CLI-命令
    workflow_excluded: true
outcome: "passed"
acceptance:
  - id: "AC-001"
    verdict: "passed"
    method: "本机真实生图并核对信封字段与落盘文件"
    evidence: "提交 3fc6275 后在真实环境多次复验：2026-11-10 cat-fishing-final（65.5s，system-skill，1.5MB PNG）；2026-11-11 三次 iOS mockup（108.5s / 89s / 88.3s，信封均含 outputPath/sourcePath/tokenUsage，产物 PNG 魔数核对）"
  - id: "AC-002"
    verdict: "passed"
    method: "系统 skill 改名后重跑同命令，再恢复，核对模板来源字段"
    evidence: "2026-11-10 实测：改名后 promptSource=embedded、36.0s 出图、skill 已恢复（SKILL_RESTORED）；改名前 promptSource=system-skill"
  - id: "AC-003"
    verdict: "passed"
    method: "cargo 单测断言参数面无模型覆盖项 + 帮助文本无 --model"
    evidence: "3fc6275 上重跑 cargo test imagegen：imagegen_exec_args_flags_and_no_model_pinning 过；taxonomy imagegen_help_lists_command_without_model_flag 过"
  - id: "AC-004"
    verdict: "passed"
    method: "三种失败场景实测（APPDATA 指空模拟 codex 缺失、CODEX_HOME 指空目录、收紧 --timeout 逼超时）"
    evidence: "codex_unavailable：APPDATA 指空实测退出码 1；codex_timeout：CODEX_HOME 指空两轮实测（401 流式捕获路径与静默挂起路径各一）；invalid_argument 由契约测试 json_imagegen_empty_prompt_fails_in_envelope 自动化锁定"
  - id: "AC-005"
    verdict: "passed"
    method: "在 3fc6275 上重跑全套测试"
    evidence: "cargo test：lib 166 过（imagegen 5 条）、cli_taxonomy 19 过（imagegen 2 条）、cli_contract 12 过（imagegen 信封 1 条）、doc 5 过；失败 0"
executed_at: "2026-11-11T11:00:00+08:00"
environment: "Windows 11 26200，codex-cli 0.167.0（npm 安装、账号登录态），worktree feature/Codex-生图-CLI-命令 @ 3fc6275"
---

# 验收（第二版）

第一版锚定 925e259，其后追加了 tokenUsage 功能提交 3fc6275（信封透出编排 token 用量），证据按协议在最终变更集上重做。五条验收标准全部通过；tokenUsage 增补本身经 2026-11-11 三次真实调用验证（信封 tokenUsage 字段与原始事件流 turn.completed usage 逐字段一致，如 input 116518/cached 74368/output 572 那次）。人工核对清单两条已由用户本人于 2026-11-11 勾选通过（manual-test.md）。
