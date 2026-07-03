# DevFlow Adapter

本目录保留给“虚幻开发流”的 typed adapter。公开层只暴露 DevFlow 的业务契约：工作区、任务、worktree、构建策略检查和安全执行记录。

边界：

- 新主逻辑放在 `Source/DevFlow`。
- adapter 只负责读取或调用受控旧能力，不成为新主逻辑。
- 需要改来源工程时，必须使用 AgentWatcher 管理的专用 git worktree 分支。
- 禁止 raw UE build、任意 shell 和破坏性删除。
