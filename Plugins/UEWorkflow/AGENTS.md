# AGENTS.md

本目录是 AgentWatcher 仓库里的 `Unreal Workflow` 插件根。它的职责是提供独立可运行的 `uwf` 工具，并让 AgentWatcher 以后只负责安装、发任务、监控和收集产物。

## 硬规则

- 新的一等代码必须使用 Rust。
- 目录结构必须贴近 Unreal：模块放在 `Source/<ModuleName>`。
- `Core` 只能定义共享契约、模型、错误、事件、schema 和安全边界；`Core` 不能依赖 `DevFlow`、`AgentHub`、`KnowledgeBase`、`UnrealMaster`、`Cli`、`Adapters` 或 `Bundles`。
- `DevFlow`、`AgentHub`、`KnowledgeBase` 默认不能互相直接依赖；跨模块协作必须走 `Core` 契约，或由 `UnrealMaster` 编排。
- `UnrealMaster` 是编排层，可以依赖三个业务模块。
- `Cli` 是命令入口，只做参数解析、调用编排层、渲染输出和返回 exit code。
- `Adapters` 只能包裹旧工具，不能成为新的主逻辑。
- 不允许任意 shell。
- 不允许 raw UE build。
- 不允许破坏性删除。
- 不允许直接修改被适配来源工程主线；需要改动时只能使用 AgentWatcher 管理的专用 git worktree 分支。
- 危险命令必须有命令清单、安全等级、dry-run、确认边界和执行记录。
- 所有机器接口必须支持稳定 `--json` 输出。
- 文档默认中文，除非明确说明非中文理由。

## 命名

- 产品名：虚幻工作流 / Unreal Workflow。
- 命令：`uwf`。
- 主控 Agent：虚幻大师 / UnrealMaster。
- 模块：`Core`、`DevFlow`、`AgentHub`、`KnowledgeBase`、`UnrealMaster`、`Cli`。
- Rust package：`uwf-core`、`uwf-devflow`、`uwf-agenthub`、`uwf-knowledgebase`、`uwf-unrealmaster`、`uwf-cli`。
- 命令组：`uwf dev`、`uwf agents`、`uwf kb`。

## 本阶段验收

- `cargo test --manifest-path Plugins/UEWorkflow/Cargo.toml` 通过。
- `cargo run --manifest-path Plugins/UEWorkflow/Cargo.toml -p uwf-cli -- doctor --json` 输出稳定 JSON。
- `cargo run --manifest-path Plugins/UEWorkflow/Cargo.toml -p uwf-cli -- modules --json` 输出三个业务模块。
- `cargo run --manifest-path Plugins/UEWorkflow/Cargo.toml -p uwf-cli -- dev status --json`、`agents status --json`、`kb status --json` 均可运行。
