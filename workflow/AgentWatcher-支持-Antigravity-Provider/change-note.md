---
schema_version: "1"
protocol: "1.3.0"
artifact: "change-note"
artifact_id: "ar_7VWSBYZSYZV5V9VWHFEG9JCC2T"
work_item_id: "wi_0ZQ8FFAX6K2B4D8F66T96TFDEM"
created_at: "2026-11-23T04:30:42Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:377edb0dac11131577aa3523826b0b7accc7a012a8b0fef5fe9226ef0433ed54
  artifacts:
    - artifact_id: "ar_3SNE7ZQBXNDEWVYD7394M01FCA"
      digest: sha256:455eb9bbf2755caf6d9975fa8d8daa8e4236634757cb9986150b940d8091732b
      locator: "plan.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "1b81016f80326705a98e600da89baa4c8b9b9de1"
    revision: "1b81016f80326705a98e600da89baa4c8b9b9de1"
    tree: "f7dde8baab2eb35e4472b65f25e8e5cdcd56f5d2"
    content_digest: "sha256:90994b90b2b68feb47236bc125b15a975e4f6c309833abbd5c99ea82226c6b9b"
    branch_or_pr: "feature/AgentWatcher-支持-Antigravity-Provider"
    workflow_excluded: true
result: "complete"
---

# 修改说明

## 覆盖账本

| 提交 | 计划步 | 覆盖内容 |
| --- | --- | --- |
| 0a15bfa | S1–S4、S6 | lib.rs Antigravity 实现块（wire-format 遍历器、清单扫描、状态机、预览/用量、打开提示、接续导出、打包）+ 8 处接线 + 20 处字面量补字段 + CLI 四处 + 技能宿主表 + 契约断言 + 15 个单测 |
| 1b81016 | S5、S7、S8 | ui/index.html 18 组接线 + cli.mjs 流程注册与实现 + AGENTS/README/CHANGELOG 口径 |

## 修改理由

- 接入依据：任务设计（accepted）按本机实测数据布局拍板——清单库出卡片元数据、
  conversations 库 protobuf 裸解析出正文与用量、无深链返回明确提示、接续照 ZCode 强口径
  导出 Markdown、技能宿主表加 `~/.gemini/config/skills`（本机已有用户手装副本为证）。
- 状态机的「超窗 RUNNING 按 idle」来自实测坑：429 配额停掉的会话状态列仍标 RUNNING。
- waiting 用尾部 `ask_question` 未闭合，与 Claude/ZCode 的 AskUserQuestion 同构（结构推断，
  设计记录未决问题 3）。

## 代码范围

- `src-tauri/src/lib.rs`：Antigravity 区块约 800 行 + 接线 10 处 + 测试约 380 行。
- `src-tauri/src/bin/agentwatcher-cli/{cli.rs,commands.rs,skill.rs}`：参数帮助、provider
  映射、宿主表。
- `src-tauri/tests/cli_contract.rs`：宿主名单断言。
- `ui/index.html`：18 组接线位。
- `tools/tauri-realtest/cli.mjs`：流程注册与 `runAntigravitySessionFlow`。
- `AGENTS.md` / `README.md` / `CHANGELOG.md`：口径同步。

## 未解释或例外改动

无。两笔提交的每一处改动都能追到计划步或设计决定。
