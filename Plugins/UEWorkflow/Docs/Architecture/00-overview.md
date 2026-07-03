# Unreal Workflow 架构总览

`Unreal Workflow` 是 AgentWatcher 内的正式插件根，但它不是 AgentWatcher 的内部业务逻辑。它必须能作为独立 `uwf` 命令运行，AgentWatcher 只负责插件安装、设置、任务发布、状态监控、执行记录和产物展示。

## 分层

```text
AgentWatcher
  -> Bundles / host integration
    -> uwf CLI
      -> UnrealMaster
        -> DevFlow
        -> AgentHub
        -> KnowledgeBase
          -> Core
```

`Core` 是稳定契约层，只定义共享模型、错误、事件、schema 和安全边界。`DevFlow`、`AgentHub`、`KnowledgeBase` 是业务模块，默认不能互相直接依赖。`UnrealMaster` 是编排层，负责选择模块、生成计划、派发命令和汇总结果。

## 边界

- AgentWatcher 不直接调用任何来源工程脚本；所有业务能力必须通过 `uwf` JSON 契约进入。
- 旧工具只能通过 `Adapters/` 的 typed adapter 接入。
- 所有用户可见业务能力必须通过 `uwf ... --json` 契约暴露。
- 任何危险动作必须先提供 dry-run、确认边界和执行记录。

## 完成方向

第一阶段已经建立 Rust workspace 骨架。后续阶段按 `Core -> DevFlow/AgentHub/KnowledgeBase -> UnrealMaster -> Cli -> AgentWatcher` 的顺序推进，确保每一层都能独立验证。
