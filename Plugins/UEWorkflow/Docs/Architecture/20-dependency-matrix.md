# 依赖矩阵

依赖必须单向，不能形成循环。

| 模块 | 允许依赖 |
| --- | --- |
| `Core` | 无 |
| `DevFlow` | `Core` |
| `AgentHub` | `Core` |
| `KnowledgeBase` | `Core` |
| `UnrealMaster` | `Core`、`DevFlow`、`AgentHub`、`KnowledgeBase` |
| `Cli` | `UnrealMaster` |

禁止项：

- `Core` 依赖任何业务模块。
- `DevFlow`、`AgentHub`、`KnowledgeBase` 互相直接依赖。
- `Cli` 直接依赖业务模块并绕过 `UnrealMaster`。
- AgentWatcher 后端直接调用三个原工程脚本。

机器校验位于 `Source/Core/tests/architecture_standards.rs`，会检查 Cargo workspace 成员、依赖方向、文档和 schema 是否存在，以及插件清单是否写入本机绝对路径。
