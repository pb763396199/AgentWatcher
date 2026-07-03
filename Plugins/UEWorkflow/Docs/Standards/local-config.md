# 本机配置标准

本机配置目录：

```text
%USERPROFILE%/.unrealworkflow/
  config/
  tasks/
  artifacts/
  knowledge-cache/
  bindings.json
  preferences.json
```

本机绑定可以记录机器绝对路径，例如来源工程路径、知识库缓存路径、provider 安装状态。提交到 git 的插件清单不能包含 `F:/...` 这类个人路径，只能保存业务模块键，例如 `DevFlow`、`AgentHub`、`KnowledgeBase`。

示例：

```json
{
  "modules.DevFlow.sourceRoot": "本机私有路径",
  "modules.AgentHub.sourceRoot": "本机私有路径",
  "modules.KnowledgeBase.sourceRoot": "本机私有路径"
}
```
