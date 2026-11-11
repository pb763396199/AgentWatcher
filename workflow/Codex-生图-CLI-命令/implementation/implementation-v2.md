---
schema_version: "1"
protocol: "1.3.0"
artifact: "implementation"
artifact_id: "ar_01M74A16HG000AA60QDHRGM4RG"
work_item_id: "wi_01M71Q3R0W0004MNNQHHS0ZSZ1"
created_at: "2026-11-10T04:03:13Z"
producer: "aes-execute"
supersedes: "ar_15QZ5SZWH6YBJ6AKRSHHY9CTRN"
dependencies:
  work_item_contract_digest: sha256:08218fcaad2d9ec21a92a9335bb0cea91f075df699314c2971d7d17dfd158939
  artifacts:
    - artifact_id: "ar_1BQQSXZVDYWZFEVT2Q1EAEPP8Y"
      digest: sha256:bffb80fc9bf3bd5ce2ebda57e02f3dcbe185062c76b8c9e1d350c76b5dfa416c
      locator: "change-note.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "34600a6a84017e628673841d74ef71c744af8dbb"
    revision: "6df0e00a848f20e9ce655a31fd85eef6ffdc1790"
    tree: "660c9b8c488fa7b46a49e2f15c4cef725305baa9"
    content_digest: "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945"
    branch_or_pr: "feature/Codex-生图-CLI-命令"
    workflow_excluded: true
result: "complete"
---
# 实现回执

## 做了什么

按 plan.md 六步全部完成，两次代码提交（c1d1b4a 代码、c0efe38 文档）：

| 改动 | 文件 |
| --- | --- |
| 固化副本（2026-11-10 本机版逐字节冻结） | `src-tauri/src/imagegen_skill/SKILL.md` |
| lib 侧生图入口：`generate_image_via_codex` + `run_codex_exec_with_auth_watch` + 事件流解析 + 产物搬运 + 4 条单测 | `src-tauri/src/lib.rs` |
| CLI 接线：命令定义 / 分发 / 信封输出与错误码映射 | `src-tauri/src/bin/agentwatcher-cli/{cli,main,commands}.rs` |
| 测试：解析用例 2 条 + 帮助文本无 --model 断言 + 信封契约 1 条 | `src-tauri/tests/{cli_taxonomy,cli_contract}.rs` |
| 文档口径：命令面清单与计费类别 | `AGENTS.md` |

## 计划步骤 0 的实测结论（写进实现的原因）

- read-only 沙箱下内置 image_gen 工具可用（工具在云端执行，本地无副作用）→ 实现固定 `-s read-only`，比计划预期的 workspace-write 更收紧。
- 编排模型自己会读系统 imagegen skill → 双来源分工据此定型：系统 skill 在场时不注入任何说明。
- 未登录态输出**不稳定**：第一次实测 2 秒出 401（stderr 的 JSONL error 事件），复测时 40 秒静默挂起无任何输出。

## 哪里没按计划走

1. **固化副本位置**：计划写 CLI 侧 `bin/agentwatcher-cli/imagegen/`，实际放 lib 侧 `src-tauri/src/imagegen_skill/`。模块依赖方向是 bin → lib，prompt 组装在 lib 的 pub 入口里，include_str! 必须跟着 lib 走。
2. **未登录错误分类**（AC-004 口径修正）：计划假设未登录可稳定识别为独立错误码。实测 codex-cli 0.167.0 未登录有两种表现——401 立即出现（此时报 `codex_auth_failed`，需要流式监视 stderr，`run_child_with_timeout_stdin` 等进程退出才扫输出的做法捕获不到）或静默挂起直到超时（归 `codex_timeout`）。为此新写 `run_codex_exec_with_auth_watch`：stdout/stderr 双读线程分块增量监视，看到 401 立即 kill 子进程。超时的 message 里同时引导检查登录态，两种表现都能指向正确处置。
3. **AC-004 自动化范围**：契约测试只锁 `invalid_argument`（稳定、无环境依赖）；codex 缺失/未登录/超时依赖本机环境，归人工验收（已全部实测过，见 validation）。

## 跑过的检查

| 检查 | 结果 |
| --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 166+5+12+19 全过（含新增 lib 4 条、taxonomy 2 条、contract 1 条） |
| AC-001 真实生图（system-skill 来源） | ok=true，47.5 s，1.5 MB PNG，`promptSource: system-skill` |
| AC-002 双来源（skill 改名后 embedded 来源） | ok=true，36.0 s，`promptSource: embedded`，skill 已恢复 |
| 失败路径抽查：APPDATA 指空 → `codex_unavailable` 退出码 1 | 通过 |
| 失败路径抽查：CODEX_HOME 指空 → 超时/401 二者之一，退出码 1 | 通过（两种表现都实测到） |

## 剩余风险

- Codex 云端行为变化（事件流字段、产物目录名）时解析会失败，失败有明确分类不静默；事件流兜底提取产物路径（计划风险表里的方案）本版未做，等真出现再做。
- 仓库存量 warning 2 处（`zcode_message_landed` dead_code、一处 unused_mut），是此前 revert 遗留，与本任务无关未动。
