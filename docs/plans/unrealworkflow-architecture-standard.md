# Unreal Workflow 架构标准

日期：2026-07-10

`UnrealWorkflow` 独立仓库已经建立在 `F:\AiProject\UnrealWorkflow`。本文件保留产品级标准；插件仓库内的 `AGENTS.md`、`Docs/Architecture/`、`Docs/Standards/` 和自动化测试是实现层硬约束。

## 1. 结论

`Unreal Workflow` 应该先做成一个独立工具，而不是 AgentWatcher 的内部功能。AgentWatcher 负责发任务、看状态、存执行记录；`Unreal Workflow` 负责真正的虚幻项目开发流程。

最终结构采用：

- 外观贴近虚幻引擎：插件描述文件 + `Source/<ModuleName>` 模块目录。
- 实现采用 Rust workspace：每个模块是一个独立 Rust package。
- 依赖方向单向：业务模块依赖 `Core`，`Core` 不依赖业务模块。
- 组合层单独存在：`UnrealMaster` / `Cli` / `Bundles` 负责把模块组合起来，不把组合逻辑塞回 `Core`。
- 知识库按范围隔离：引擎核心、引擎内插件、项目插件、项目级内容分开存放。
- 旧工具允许过渡期适配，但所有新的一等代码都应 Rust 化。

## 2. 产品与命名

| 项 | 中文 | 英文 / 代码 |
| --- | --- | --- |
| 产品名 | 虚幻工作流 | Unreal Workflow |
| 命令 | 虚幻工作流命令 | `uwf` |
| 主控 Agent | 虚幻大师 | UnrealMaster |
| 开发流模块 | 虚幻开发流 | Dev Flow / `DevFlow` / `devflow` / `uwf dev` |
| 智能体模块 | 虚幻智能体 | Agent Hub / `AgentHub` / `agents` / `uwf agents` |
| 知识库模块 | 虚幻知识库 | Knowledge Base / `KnowledgeBase` / `kb` / `uwf kb` |

命名规则：

- 用户界面显示中文名和英文显示名。
- 目录名用 PascalCase：`Core`、`DevFlow`、`AgentHub`、`KnowledgeBase`、`UnrealMaster`、`Cli`。
- Rust package 用 kebab-case：`uwf-core`、`uwf-devflow`、`uwf-agenthub`、`uwf-knowledgebase`、`uwf-unrealmaster`、`uwf-cli`。
- Rust crate 导入名自然变成 snake_case：`uwf_core`、`uwf_devflow`、`uwf_agenthub`。
- 模块名前不再重复加 `UWF`。已经在插件名和 package 前缀里表达过一次，不要出现 `UwfDevFlowDevFlowManager` 这类名字。
- provider 兼容属于 `AgentHub` 内部能力，不单独拆成一堆顶层模块。

## 3. 目录结构

推荐结构：

```text
UnrealWorkflow/
  AGENTS.md
  Cargo.toml
  UnrealWorkflow.awplugin
  Source/
    Core/
      Cargo.toml
      src/
        lib.rs
        api.rs
        model/
        ports/
        internal/
    DevFlow/
      Cargo.toml
      src/
        lib.rs
        api.rs
        task/
        workspace/
        git/
        host/
        build/
        junction/
        internal/
    AgentHub/
      Cargo.toml
      src/
        lib.rs
        api.rs
        providers/
        roles/
        commands/
        installs/
        internal/
    KnowledgeBase/
      Cargo.toml
      src/
        lib.rs
        api.rs
        scopes/
        pipeline/
        query/
        index/
        export/
        internal/
    UnrealMaster/
      Cargo.toml
      src/
        lib.rs
        plan.rs
        dispatch.rs
        policy.rs
        review.rs
    Cli/
      Cargo.toml
      src/
        main.rs
        commands/
  Adapters/
    DevFlow/
    AgentHub/
    KnowledgeBase/
  Resources/
  .plugin-worktrees/
  Config/
    default.toml
    schemas/
  Docs/
    Architecture/
    Standards/
    Decisions/
  Tests/
    fixtures/
    contracts/
```

说明：

- `Source/<ModuleName>` 对齐 Unreal 的模块心智模型。
- 每个 `Source/<ModuleName>` 同时是 Cargo workspace member。
- `src/api.rs` / `src/ports/` 相当于 Unreal 模块的公开面。
- `src/internal/` 加 `pub(crate)` 相当于 Unreal 模块的私有实现。
- `Adapters/` 只包裹旧工具，不能成为新架构的主逻辑。
- AgentWatcher 集成只通过 `UnrealWorkflow.awplugin` 和外部进程协议完成，不在插件源码中反向依赖宿主。

## 4. 依赖方向

必须保持单向依赖：

```text
Cli
  -> UnrealMaster
      -> DevFlow
      -> AgentHub
      -> KnowledgeBase
          -> Core

Adapters/*
  -> Core
  -> 被适配的旧工具
```

硬规则：

- `Core` 不能依赖 `DevFlow`、`AgentHub`、`KnowledgeBase`、`UnrealMaster`、`Cli`、`Bundles` 或 `Adapters`。
- `DevFlow`、`AgentHub`、`KnowledgeBase` 默认不能互相直接依赖。
- 跨模块协作走 `Core` 的数据契约，或由 `UnrealMaster` 编排。
- 如果某个组件需要依赖所有模块，它就不是 `Core`，应叫 `UnrealMaster`、`Runtime` 或 `Bundle`。
- `Core` 只放稳定对象、trait、错误码、事件、artifact 引用和路径策略，不放具体 provider、UE build、git worktree 或知识库 pipeline 实现。

## 5. 模块职责

### Core

负责稳定契约：

- `TaskContext`
- `WorkspaceId`
- `ModuleId`
- `CommandSpec`
- `CommandOutcome`
- `ArtifactRef`
- `KnowledgeScope`
- `ProviderRole`
- `SafetyLevel`
- `DryRunPlan`
- 统一错误分类：用户输入错误、环境错误、工具错误、内部错误。

`Core` 不做实际执行。

### DevFlow

负责虚幻项目开发流：

- 工作区发现与选择。
- 任务创建。
- git worktree、分支、提交、合并、清理。
- Host 项目与任务项目的路径隔离。
- Junction 状态。
- 两类 build 入口。

Build 命令必须区分：

- `uwf dev build-task <task-id>`：从任务 worktree / Host 隔离环境出发构建，适合任务内验证。
- `uwf dev build-project --start-path <path>`：从主项目或当前路径出发构建，适合 AgentHub 统一纳管的 build wrapper 逻辑。
- `uwf dev build-check --start-path <path>`：只做构建策略检查和 dry-run，不启动真实 UE 编译。

选择规则：

- 有明确任务 ID 或任务 worktree 时，用 `build-task`。
- 只给主项目路径、插件路径或当前目录时，用 `build-project`。
- 用户只是问能不能构建、该怎么构建、会构建什么时，用 `build-check`。
- 任何命令都不能默默退回 raw UE build。

### AgentHub

负责 AI 协作流：

- provider 发现。
- provider 安装与检查。
- role / skill / command 注册。
- 虚幻大师 `UnrealMaster` 的 provider surface 适配。
- 计划代理、执行代理、评审代理、诊断代理。
- VSCode Copilot、Codex、Claude Code、OpenCode 等 provider 的差异处理。

约束：

- provider 兼容放在 `AgentHub` 内，不拆成顶层模块。
- 对外暴露统一角色和 command，不暴露原始脚本名、profile 路径或 provider 内部文件名。
- 共享语义是主，provider 文件只是适配层。

### KnowledgeBase

负责知识流：

- 知识范围注册。
- 引擎核心知识。
- 引擎内插件知识。
- 项目插件知识。
- 项目级规则和历史任务。
- 查询、生成、沉淀、导出。
- 兼容知识库来源能力的 engine mode / plugin mode。

约束：

- 不允许把不同插件内容混在一起。
- 不允许把项目插件和引擎核心混在一起。
- 引擎内插件应作为独立 `engine_plugin` scope，不应只混在 engine mode 的大索引里。

### UnrealMaster

负责把用户目标变成流程：

- 读取目标。
- 判断需要哪些模块。
- 生成计划。
- 分发命令。
- 收集结果。
- 触发评审和失败诊断。
- 决定哪些内容应沉淀进知识库。

`UnrealMaster` 可以依赖 `DevFlow`、`AgentHub`、`KnowledgeBase`，因为它是编排层，不是基础层。

## 6. Rust 化策略

目标：所有新的一等代码使用 Rust。

迁移策略：

- 新的核心模型、CLI、插件契约、任务状态机、路径策略、执行记录、知识 scope 规则必须 Rust 化。
- 现有 Python / PowerShell / 旧 Rust 工具可以先通过 `Adapters/` 包住。
- adapter 必须有 typed request / typed output，不能把任意 shell 暴露给上层。
- adapter 输出必须逐步规范成 JSON 或稳定 exit code。
- 旧脚本只允许作为过渡实现，不允许成为新的公开契约。
- 删除旧实现前必须有契约测试和真实 fixture 覆盖。

不采用一次性大爆炸重写。理由是 UE / Windows / provider 自动化里有很多隐性行为，直接全量重写容易丢掉已经稳定的细节。正确方式是先把边界和契约锁住，再逐个替换实现。

## 7. 命令契约

统一入口：

```text
uwf master run
uwf dev ...
uwf agents ...
uwf kb ...
uwf doctor
```

命令要求：

- 每个命令都必须有稳定 `--json` 输出。
- 人类输出和机器输出分开。
- 任何危险命令必须有安全等级、dry-run、确认边界和执行记录。
- 命令执行记录必须包含 program、args、cwd、关键环境变量差异、exit code、stdout/stderr 摘要、artifact 路径。
- 不允许任意 shell。
- 不允许 raw UE build。
- 不允许破坏性删除。

dry-run 的含义：

- dry-run 不是独立业务流程。
- dry-run 是危险命令的预演参数或预演阶段。
- 它回答“如果真的执行，会做什么、写哪里、调用谁、风险是什么”。

## 8. 知识库存储标准

知识库分两层：

- 团队 Git 知识库：保存稳定文档、规则、方案、复盘、轻量索引和 scope 注册表。
- 本机缓存：保存知识库重型索引、数据库、pipeline 状态、日志和临时产物。

推荐团队仓库：

```text
UnrealWorkflowKnowledge/
  README.md
  kb-registry.yaml
  schemas/
    scope.schema.yaml
    doc-frontmatter.schema.yaml
    kb-registry.schema.yaml
  global/
    rules/
    workflows/
    index/
  scopes/
    engines/
      ue-5.5.4/
        scope.yaml
        engine-core/
          docs/
            guides/
            research/
            solutions/
            retrospectives/
            rules/
            index/
        engine-plugins/
          niagara/
            scope.yaml
            docs/
              guides/
              research/
              solutions/
              retrospectives/
              rules/
              index/
    projects/
      shanghai-p4-neon/
        project.yaml
        project/
          docs/
            guides/
            research/
            solutions/
            retrospectives/
            rules/
            index/
        plugins/
          aesworld/
            scope.yaml
            docs/
              guides/
              research/
              solutions/
              retrospectives/
              rules/
              index/
```

scope 类型：

- `engine_core`：虚幻引擎核心。
- `engine_plugin`：引擎目录里的插件，例如 `Engine/Plugins/...`。
- `project_plugin`：项目目录里的插件，例如 `Project/Plugins/AesWorld`。
- `project`：项目级内容，不属于某个插件。
- `global`：跨项目规则和流程。

每个知识条目必须先绑定 scope，再进入用途目录。不能先按“方案/复盘/规则”大池子混放。

`scope.yaml` 最小字段：

```yaml
scope_id: project/shanghai-p4-neon/plugin/aesworld
scope_kind: project_plugin
display_name: AesWorld
project_id: shanghai-p4-neon
plugin_id: aesworld
engine_version: "5.5.4"
source_identity:
  git_remote: "<team repo url or none>"
  plugin_descriptor: AesWorld.uplugin
  stable_path_hint: Plugins/AesWorld
kb_maker:
  preferred_mode: plugin
  cache_key: project-shanghai-p4-neon-plugin-aesworld-ue-5.5.4
```

本机目录统一放：

```text
%USERPROFILE%/.unrealworkflow/
  config/
  tasks/
  artifacts/
  knowledge-cache/
    engines/
    projects/
  bindings.yaml
  preferences.yaml
```

本机缓存可以记录个人绝对路径；团队 Git 知识库不能记录 `F:/...` 这类个人路径，只能记录稳定身份和相对路径提示。

KnowledgeBase 兼容规则：

- engine mode 写入 `knowledge-cache/engines/<engine-version>/engine-core/KnowledgeBase/`。
- 引擎内插件使用 plugin mode，写入 `knowledge-cache/engines/<engine-version>/engine-plugins/<plugin>/KnowledgeBase/`。
- 项目插件使用 plugin mode，写入 `knowledge-cache/projects/<project>/plugins/<plugin>/KnowledgeBase/`。
- 团队 Git 只提交精选 Markdown、scope manifest、schema 和轻量索引，不提交 SQLite、pickle、大型中间文件、日志或 checkpoint。

Markdown frontmatter 最小字段：

```yaml
---
title: AesWorld 主插件识别问题解决方案
purpose: solution
status: done
date: 2026-06-19
language: zh-CN
scope_id: project/shanghai-p4-neon/plugin/aesworld
scope_kind: project_plugin
engine_version: "5.5.4"
evidence_refs:
  - task: Task-031
---
```

## 9. Agent Markdown 与强制约束

当前根级 `AGENTS.md` 主要约束 Codex / OMX 的工作方式，不足以锁定 `Unreal Workflow` 的架构、命名和依赖方向。

后续必须新增：

```text
UnrealWorkflow/AGENTS.md
UnrealWorkflow/Source/DevFlow/AGENTS.md
UnrealWorkflow/Source/AgentHub/AGENTS.md
UnrealWorkflow/Source/KnowledgeBase/AGENTS.md
UnrealWorkflow/Docs/Standards/
```

根 `AGENTS.md` 只放短硬规则：

- 只能从 `uwf` 入口暴露用户命令。
- 模块目录必须是 `Source/<ModuleName>`。
- `Core` 不能依赖业务模块。
- 业务模块不能互相直接依赖。
- 跨模块通信走 `Core` 契约或 `UnrealMaster` 编排。
- 新的一等代码必须 Rust 化。
- 旧脚本只能通过 `Adapters/` 被调用。
- 外部副作用必须有命令清单、安全等级、dry-run 和执行记录。
- 文档默认中文，除非明确说明非中文理由。
- 知识条目必须绑定 scope。

Markdown 不是唯一约束。必须配套机器校验：

- `cargo metadata` 检查依赖方向。
- schema 检查 `UnrealWorkflow.awplugin`、command manifest、scope manifest 和 frontmatter。
- CLI 契约测试检查 `uwf ... --json`。
- 文档写入测试检查 scope、语言、路径、purpose。
- provider 适配测试检查 AgentHub 输出的角色和命令是否一致。

## 10. 官方依据

- Unreal Engine 插件模型：插件由描述文件和模块组成，模块位于插件 `Source` 目录下，可通过描述文件声明。
- Unreal Engine 模块模型：模块是独立编译单元，有公开/私有边界和依赖声明。
- Cargo workspace：一个 workspace 管理多个 package，共享 lockfile、target、workspace dependencies、workspace lints。
- Rust API Guidelines：公开 API 命名、错误、trait、文档、可维护性需要稳定一致。
- Codex AGENTS.md：适合存放会反复影响 agent 行为的项目级规则，但仍需要测试和 schema 做机器约束。

## 11. 下一步实施顺序

1. 独立仓库、Rust workspace、描述文件、三个业务模块和 `uwf` CLI 已建立。
2. `AgentWatcherPluginStdio/1` 握手、插件发现、挂载、启用和通用命令调用已建立。
3. 继续把 AgentWatcher 中旧的 UWF 兼容任务数据迁为通用 workflow contribution 数据。
4. 完成插件发布包、版本锁定、升级和卸载测试。
5. 把三个 Legacy worktree 中尚未提交的安全契约改动迁入正式模块后再清理 Legacy。
