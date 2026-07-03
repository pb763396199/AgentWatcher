# KnowledgeBase Adapter

本目录保留给“虚幻知识库”的 typed adapter。公开层只暴露 KnowledgeBase 的业务契约：scope、查询、生成、沉淀、导出、本机缓存与团队知识库分离。

边界：

- 新主逻辑放在 `Source/KnowledgeBase`。
- adapter 只负责读取或调用受控旧能力，不成为新主逻辑。
- 需要改来源工程时，必须使用 AgentWatcher 管理的专用 git worktree 分支。
- 禁止生成重型缓存进 git，禁止 raw UE build、任意 shell 和破坏性删除。
