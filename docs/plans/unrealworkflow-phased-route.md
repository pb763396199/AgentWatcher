# Unreal Workflow 独立插件收口路线

日期：2026-07-10

## 已完成

- `UnrealWorkflow` 已从 `AgentWatcher/Plugins/UEWorkflow` 保留历史地拆为独立 Git 仓库。
- 插件根改为 `UnrealWorkflow.awplugin + Source/<ModuleName>`。
- AgentWatcher 不再跟踪或保留 UWF 插件源码、构建缓存和知识库内容。
- 六个模块 worktree 已迁到 `UnrealWorkflow/.plugin-worktrees`；三个旧 worktree 的未提交改动保存在 `Legacy/`。
- AgentWatcher 新增零插件安全的插件目录、挂载状态、默认禁用状态和外部进程调用器。
- UWF 新增 `uwf host invoke --json`，实现稳定握手、doctor、modules、模块状态、工作流计划和执行入口。
- 任务工作流入口改为读取已启用插件的 `Contributions.Workflows`，不再写死 UWF。
- `TaskSchema` 由宿主读取并动态生成任务字段；字段含义归插件所有。
- 预演和执行统一调用贡献声明的 `PlanCommand` / `ExecuteCommand`。
- AgentWatcher 已删除全部 `ueworkflow_*` 专用 Tauri 命令及专用执行存储模块。
- 当前机器已通过外部插件真实握手。

## 通用任务记录

插件任务只保存通用记录：

```json
{
  "workflow": "plugin:<PluginName>:<WorkflowName>",
  "pluginWorkflowInput": {},
  "pluginWorkflowPlan": {},
  "pluginWorkflowExecution": {},
  "pluginWorkflowLifecycle": {}
}
```

AgentWatcher 不解释这些字段里的业务含义；它只校验 schema、调用声明命令、保存结果和打开用户选择的 Provider。

## 剩余发布门禁

1. 增加真实 Tauri 三态测试：未挂载、挂载未启用、挂载已启用。
2. 建立 UWF 发布包，让新电脑安装插件时不依赖本机源码仓库。
3. 为描述文件和任务 schema 增加版本迁移策略。

宿主源码不得因为新增第二个插件而修改；这是后续所有插件接入的回归门禁。
