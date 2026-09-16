---
schema_version: "1"
protocol: "1.3.0"
artifact: "implementation"
artifact_id: "ar_17QYMV7XZVJT2YMZ97TDYEPB8T"
work_item_id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
created_at: "2026-09-16T07:17:16Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:32abb9b77345e1c4412d7442f9ae353d1bca154ec66037eeaf403b276872b2ea
  artifacts:
    - artifact_id: "ar_6QZQXNZXZKAB0CDMCJKYF5N9EY"
      digest: sha256:c096034517012db1fb61739fa91081516480364e41e94b7c3c5e96d0d6ee6ca0
      locator: "change-note.md"
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
result: "complete"
---
# 实现回执

## 做了什么

计划八步全部完成，六个施工提交落在 `feature/agent-cli`（979d377 → de5a981），基线 3e5ed2c。第 8 步（skill 分发）是 2026-09-16 用户补充拍板并入的：改验收合同（加 AC-008）、设计未决问题落定、计划补第 8 步，然后实现。

## 每步结果

| 步骤 | 结果 | 证据 |
| --- | --- | --- |
| S1 基线 | 92 测试：91 过 1 忽略 0 失败 | cargo test 输出 |
| S2 可见性 | 构建零警告，测试结果与基线一致 | cargo build + test |
| S3 骨架 | 分类学测试通过 | cargo test --test cli_taxonomy |
| S4 命令 | 契约测试通过（修复前全部挂死，见下） | cargo test --test cli_contract |
| S5 打包 | node --check 通过，拷贝段与主程序对称 | scripts diff |
| S6 文档 | README/AGENTS 口径齐全 | ce57bf2 |
| S7 全量 | 全绿、CLI debug 产物 6.1MB、git diff --check 干净 | 本轮命令输出 |
| S8 skill 分发 | 分类学 8 项、契约 8 项全过（各 +1）；全量 cargo test 绿（91+8+8 过、1 忽略） | de5a981 测试输出 |

## 哪里没按计划走

- 计划第 2 步说三个函数「各一行改 pub」。实际 `get_session_usage_detail` 因 tauri command 宏与 pub 冲突拆成命令壳加实现函数；`HandoffSourceContext` 两个字段、`AgentSession` 八个字段也改了 pub。仍在「纯可见性」范畴，设计已回填。
- 计划第 4 步撞上环境级缺陷：契约测试全挂在 `Command::output()`。根因是扫描 Codex 会话经 `with_codex_app_server_client` 拉起常驻 `codex.cmd app-server`（cmd→node→codex 链），链上进程继承 CLI 的 stdout 管道写端，子进程退出后管道仍不 EOF。差分实验（排除 codex 正常、仅 codex 挂）与因果实验（杀链即恢复）双重确认。修法是 CLI 禁用 app-server 加清 stdio 继承标志，都在提交 cc9aa64。
- 计划外补充：package-exe.mjs 给 CLI 加显式 `cargo build --release --bin agentwatcher-cli`（tauri build 只保证 default-run 主程序）。
- S8 补充拍板改变了 v1 范围（原访谈把 skill 分发列为非目标）：合同加 AC-008 后评审与验收按协议重跑，即本版记录。

## 跑过哪些检查

cargo test（全量，六轮）、cargo build --bin agentwatcher-cli、node --check scripts/package-exe.mjs、git diff --check、真实二进制管道读写手工验证（四查询命令双模式）、真实 handoff export 落盘验证、skill 生命周期经临时目录的契约验证。

## 还剩什么风险

- Codex 会话在 CLI 里来自本地文件扫描，比 GUI 的 app-server 实时列表可能少最新线程元数据；状态机与预览不受影响。
- skill 候选宿主目录表是硬编码清单，新 AI 宿主出现要改代码；`--dir` 可兜底。
- release 构建下 CLI exe 未做 rcedit 图标嵌入（设计即如此）；human 模式表格排版只有手工目检。
