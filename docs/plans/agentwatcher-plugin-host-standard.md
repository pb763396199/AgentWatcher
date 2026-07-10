# AgentWatcher 插件宿主标准

日期：2026-07-10

## 结论

AgentWatcher 是宿主，不是插件集合。它可以没有任何插件；只有外部插件被挂载并启用后，对应工作流、设置动作和命令才会出现。

Unreal Workflow 已迁到独立仓库。AgentWatcher 仓库不再保存它的源码、模块 worktree、知识库内容或构建缓存。

## 与 Unreal 插件模型的对应

| Unreal Engine | AgentWatcher |
| --- | --- |
| Engine / Project 宿主 | AgentWatcher |
| 插件搜索目录 | 用户安装目录和显式开发挂载目录 |
| `<PluginName>.uplugin` | `<PluginName>.awplugin` |
| `Source/<ModuleName>` | 插件仓库内的 `Source/<ModuleName>` |
| Plugin Browser | AgentWatcher 设置页的插件管理 |
| 项目启用状态 | `%APPDATA%/AgentWatcher/plugins.v1.json` |
| Additional Plugin Directories | `%APPDATA%/AgentWatcher/plugin-mounts.v1.json` |
| 模块加载阶段 | 描述文件的 `LoadingPhase` |

和 Unreal 一样，扫描器在目录中发现插件描述文件后，就把该目录视为一个完整插件，不继续把它的内部子目录识别成其他插件。

## 仓库边界

```text
AgentWatcher/
  src-tauri/src/plugin_catalog.rs
  ui/
  docs/

UnrealWorkflow/
  UnrealWorkflow.awplugin
  Source/
    Core/
    DevFlow/
    AgentHub/
    KnowledgeBase/
    UnrealMaster/
    Cli/
```

AgentWatcher 只能依赖公开插件协议，不能依赖 `UnrealWorkflow` Rust crate、目录结构、来源工程或内部命令。

## 生命周期

插件有三个明确状态：

1. 未挂载：宿主不知道插件存在，UI 和任务工作流中不出现。
2. 已挂载但未启用：设置页能看到插件，但不加载工作流，不允许执行命令。
3. 已挂载且启用：宿主显示描述文件声明的贡献，并允许调用声明过的命令。

卸载只删除挂载记录，不删除外部仓库。停用只修改宿主状态，不修改插件描述文件。

旧任务使用过的工作流 ID 由插件贡献的 `LegacyIds` 自行声明。AgentWatcher 只做通用别名解析，不保存任何具体插件的迁移表。

## 进程协议

插件使用外部进程而不是动态 Rust DLL。原因是独立仓库需要稳定发布边界、崩溃隔离和版本握手，而 Rust ABI 不适合作为长期动态插件 ABI。

宿主固定执行描述文件声明的可执行文件和参数，并通过标准输入发送：

```json
{
  "schemaVersion": 1,
  "command": "plugin.handshake",
  "request": {}
}
```

插件标准输出只能返回一个 JSON 文档，诊断信息写标准错误。宿主不调用 shell，也不允许 UI 自行指定 program 或 args。

## 安装与开发

- 普通安装：插件完整包进入 `%LOCALAPPDATA%/AgentWatcher/Plugins/<PluginName>`。
- 开发挂载：设置页把独立插件仓库路径写入 `plugin-mounts.v1.json`。
- 插件内部的 CLI、Agent、Skill 和 Provider 安装由插件自己的设置命令完成。
- AgentWatcher 只显示并调用描述文件中 `Placement: Settings` 的命令，不知道安装细节。

## 验收门禁

- AgentWatcher 源码树内不存在具体 `*.awplugin` 和插件源码。
- 零插件测试通过，普通任务创建仍可使用。
- 已挂载未启用时，插件工作流不出现在任务创建选项中。
- 启用后，工作流来自描述文件贡献。
- 描述文件名与 `Name` 不一致时拒绝挂载。
- 重名插件拒绝加载，避免路径顺序导致不确定行为。
- 插件可执行文件必须位于插件根内。
- 未声明命令、任意 shell 和越界路径一律拒绝。
