# 目录结构标准

目录结构贴近 Unreal 插件模型，但实现使用 Rust workspace。

```text
Plugins/UEWorkflow/
  AGENTS.md
  Cargo.toml
  UnrealWorkflow.uwplugin.json
  Source/
    Core/
    DevFlow/
    AgentHub/
    KnowledgeBase/
    UnrealMaster/
    Cli/
  Adapters/
    DevFlow/
    AgentHub/
    KnowledgeBase/
  Config/
    schemas/
  Docs/
    Architecture/
    Standards/
  Tests/
    contracts/
```

规则：

- `Source/<ModuleName>` 是唯一的一等模块目录。
- 每个 `Source/<ModuleName>` 都是 Cargo workspace member。
- 模块目录使用 PascalCase。
- Rust package 使用 kebab-case，例如 `uwf-devflow`。
- `Adapters/` 只能包裹旧工具，不能承载新主逻辑。
- `target/`、本机缓存、重型知识库索引不能进入 git。
