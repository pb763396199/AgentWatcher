---
schema_version: "1"
protocol: "1.3.0"
artifact: "validation"
artifact_id: "ar_0DP7HYTWKFKAGQ4YJ63QHKDKNR"
work_item_id: "wi_01M71Q3R0W0004MNNQHHS0ZSZ1"
created_at: "2026-11-10T04:16:56Z"
producer: "aes-validate"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:08218fcaad2d9ec21a92a9335bb0cea91f075df699314c2971d7d17dfd158939
  artifacts:
    - artifact_id: "ar_15QZ5SZWH6YBJ6AKRSHHY9CTRN"
      digest: sha256:a1a37475e7b3627e443c8aa934b962de533cb7387b502c029b0ad2f9c3ad2762
      locator: "implementation.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "925e259e3cc47d929d471846cb8fcbe5c5d07167"
    revision: "925e259e3cc47d929d471846cb8fcbe5c5d07167"
    tree: "bb7ffa1e07675883547c2317740e9db36821fccb"
    content_digest: "sha256:9404d3d863657b4615f4ee8a87ba560037a22d98a686458c759338b82ee39a53"
    branch_or_pr: "feature/Codex-生图-CLI-命令"
    workflow_excluded: true
outcome: "superseded"
acceptance:
  - id: "AC-001"
    verdict: "passed"
    method: "本机真实生图一次，核对信封字段与落盘文件"
    evidence: "命令 imagegen generate \"a minimal hero image of a ceramic coffee mug...\" --out .tmp/img-accept/hero.png：信封 ok=true、engine=codex/imagegen、promptSource=system-skill、elapsedMs=47462；hero.png 1534985 字节，前 8 字节 89 50 4E 47（PNG 魔数）；图片目视为提示词所描述的咖啡杯 hero 图"
  - id: "AC-002"
    verdict: "passed"
    method: "系统 skill 改名后重跑同命令，再恢复，核对模板来源字段"
    evidence: "skill 在场：promptSource=system-skill（AC-001 同次）；改名 %USERPROFILE%\\.codex\\skills\\.system\\imagegen 后：promptSource=embedded、elapsedMs=36044、compass.png 正常产出、退出码 0；skill 已恢复（SKILL_RESTORED）"
  - id: "AC-003"
    verdict: "passed"
    method: "cargo test 单测断言参数面无模型覆盖项 + 帮助文本无 --model"
    evidence: "cargo test imagegen：imagegen_exec_args_flags_and_no_model_pinning 过（断言无 -m、无 model_reasoning_effort）；taxonomy imagegen_help_lists_command_without_model_flag 过（帮助文本含 imagegen、无 --model）"
  - id: "AC-004"
    verdict: "passed"
    method: "三种失败场景各实测一次（PATH 缺 codex 用 APPDATA 指空模拟、CODEX_HOME 指空目录、--timeout 收紧逼超时）"
    evidence: "APPDATA 指空：error.code=codex_unavailable、退出码 1；CODEX_HOME 指空目录：两次实测分别 401（run_codex_exec_with_auth_watch 流式捕获，归 codex_auth_failed 的判定路径）与静默挂起归 codex_timeout（错误信息引导检查登录态）、退出码 1；invalid_argument 场景由契约测试 json_imagegen_empty_prompt_fails_in_envelope 自动化锁定（五键信封、退出码 1）。未登录两种表现的不稳定性记录在 implementation 的实测节"
  - id: "AC-005"
    verdict: "passed"
    method: "在最终变更集（925e259）上重跑全套测试"
    evidence: "cargo test --manifest-path src-tauri/Cargo.toml：lib 166 过（含 imagegen 4 条）、cli_taxonomy 19 过（含 imagegen 2 条）、cli_contract 12 过（含 imagegen 信封 1 条）、doc 5 过；失败 0"
executed_at: "2026-11-10T12:20:00+08:00"
environment: "Windows 11 26200，codex-cli 0.167.0（npm 安装、账号登录态），AgentWatcher worktree feature/Codex-生图-CLI-命令 @ 925e259"
---
# 验收

五条验收标准全部在最终变更集（925e259，R1 修复后）上真跑通过。两次真实生图产物与一次 401 提示词回归产物都留在 `F:\AiProject\AgentWatcher\.tmp\img-accept\`。401 误杀回归（评审 R1）也在本变更集上复验：提示词含 "401 Unauthorized" 仍正常出图（46.9 秒）。

人工核对清单 `manual-test.md` 两条待用户核对（真实生图目视、改名 skill 复跑），与本记录的机器证据互补。
