---
schema_version: "1"
protocol: "1.3.0"
artifact: "change-note"
artifact_id: "ar_01M7A945S1000809QW2T004P35"
work_item_id: "wi_1QYFZ7SPZ2TGAMZAMK448JP45C"
created_at: "2026-11-13T11:01:46Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:807ccdf792d9da9509eddb8e1cc706954c00ea16bf8f9c44bb645b13b854af56"
  artifacts:
    - artifact_id: "ar_1ZPZFSSPSZ1NFNZ0BXRH5HFXDQ"
      digest: "sha256:ebc63be0d35671e333ef4f414ea4229a0805366392009dfbe858f212680c2fe6"
      locator: "plan.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "34493f3b35552dad87cdbe860d8b0341caf35e6d"
    revision: "34493f3b35552dad87cdbe860d8b0341caf35e6d"
    tree: "caca832a448a9e09a44cad3402650b7430e8fa0c"
    content_digest: "sha256:65871a827481ffc6dbfb249e25d5f9938826706c91f3d4b0067dbbdf3b60185c"
    branch_or_pr: "dev"
    workflow_excluded: true
result: "complete"
---
# 修改理由

## 覆盖账本

| 文件 | 修改依据 |
| --- | --- |
| `AGENTS.md` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `README.md` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/Cargo.lock` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/Cargo.toml` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/bin/agentwatcher-cli/cli.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/bin/agentwatcher-cli/main.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/bin/agentwatcher-cli/skill/SKILL.md` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/bin/agentwatcher-cli/workflow_commands.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/lib.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/workflow/backup.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/workflow/codex.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/workflow/git.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/workflow/legacy.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/workflow/mod.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/workflow/service.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/src/workflow/store.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/tests/cli_contract.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/tests/cli_taxonomy.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/tests/workflow_codex.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/tests/workflow_contract.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/tests/workflow_git.rs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `src-tauri/workflow-guide.md` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `tools/tauri-realtest/cli.mjs` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |
| `ui/index.html` | 通用工作流实现、入口、回归或指南；对应 P1–P9 |

## 为什么修改

SQLite 和单例服务承担正式状态、事务和收件；Codex 运行器独立持有 thread，避免退出界面中断施工。Git 先造候选再比较目标，交付与代码整合分别记录。规划规模不变成运行期截止条件。来源是已接受设计与 P1–P9。

## 例外和未解释改动

没有具体 AES 插件源码进入宿主。既有未跟踪待办保留。当前修改未提交，subject 的 revision/tree 是 Git 基线，实际工作区内容以 content_digest 和 [逐文件摘要](assets/working-changes.json) 为准。旧 awf assemble 要求干净 Git 树而拒绝，此处不伪造已提交证据。
