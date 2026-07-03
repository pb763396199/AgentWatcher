# AgentHub Adapter

本目录保留给“虚幻智能体”的 typed adapter。公开层只暴露 AgentHub 的业务契约：provider 状态、虚幻大师、计划/执行/评审/诊断/文档沉淀角色和 provider command 包。

边界：

- 新主逻辑放在 `Source/AgentHub`。
- adapter 只负责读取或调用受控旧能力，不成为新主逻辑。
- 需要改来源工程时，必须使用 AgentWatcher 管理的专用 git worktree 分支。
- 禁止安装未知 provider、污染上下文、raw UE build、任意 shell 和破坏性删除。
