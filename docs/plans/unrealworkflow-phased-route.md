# UnrealWorkflow 分阶段路线

## 目标
- 先把 `UnrealWorkflow` 做成独立工具/CLI，统一编排开发流、智能体、知识库三类业务能力。
- 先验证它能被 VSCode Copilot 等 provider 手动/半自动执行。
- 再把 `AgentWatcher` 接成任务发布与流程监控层。

## 现状依据
- DevFlow 来源能力已经覆盖 worktree + NTFS Junction 的任务隔离，核心命令包括任务创建、推进、构建状态、合并和清理，并明确要求任务上下文冻结，避免串项目。
- AgentHub 来源能力已经把 Copilot / Claude Code / Codex 的角色语义收敛到共享 contracts，并提供 build wrapper，适合承载 provider 手动/半自动执行入口。
- KnowledgeBase 来源能力已经支持插件模式、自动检测、Skill 生成和分阶段 pipeline，适合作为知识侧的生成器/校验器，而不是第一阶段的主编排器。
- `AgentWatcher` 里已经有 `Plugins/UEWorkflow` 模块映射和 `ueworkflow_execution.rs` 的 dry-run / knowledge 记录入口，说明“发布与监控”可以作为后置层接入。参考 [Plugins/UEWorkflow/AgentWatcher.awplugin.json:10](../../Plugins/UEWorkflow/AgentWatcher.awplugin.json#L10)、[Plugins/UEWorkflow/AgentWatcher.awplugin.json:22](../../Plugins/UEWorkflow/AgentWatcher.awplugin.json#L22)、[Plugins/UEWorkflow/AgentWatcher.awplugin.json:34](../../Plugins/UEWorkflow/AgentWatcher.awplugin.json#L34)、[src-tauri/src/ueworkflow_execution.rs:88](../../src-tauri/src/ueworkflow_execution.rs#L88)、[src-tauri/src/ueworkflow_execution.rs:102](../../src-tauri/src/ueworkflow_execution.rs#L102)、[src-tauri/src/ueworkflow_execution.rs:160](../../src-tauri/src/ueworkflow_execution.rs#L160)。

## 阶段 1: 独立 CLI 成型
### 目标
- 定义 `UnrealWorkflow` 的最小稳定命令面：发现项目、生成计划、执行受控步骤、导出状态。
- 把三个仓库映射成清晰的职责边界，而不是把它们混成一个大壳。

### 验收标准
- CLI 能显式配置或发现三个 repo 根目录，并在同一份计划中引用它们。
- `dry-run` 能输出结构化阶段、检查点、预期产物和回滚边界，且不写入外部副作用。
- 任何 provider 都只消费同一套 CLI 契约，不依赖 AgentWatcher 才能产出计划。
- 失败时返回可读的原因和缺失输入，而不是静默降级。

### 应该避免
- 不要先把 UI、daemon、消息总线一起做成大一统。
- 不要把 AgentWatcher 作为第一入口。
- 不要在这一阶段绑定某一个 provider 的专有语法。
- 不要默认执行 raw UE build 或破坏性文件操作。

## 阶段 2: provider 手动 / 半自动验证
### 目标
- 先证明 VSCode Copilot 能手动或半自动跑通 `UnrealWorkflow`。
- 再证明至少一个非 Copilot provider 也能复用同一份 CLI 契约。

### 验收标准
- 在 VSCode Copilot 路径下，能完成一次完整的“读取上下文 -> 生成计划 -> 产出 dry-run 包 -> 人工确认”的闭环。
- 在另一个 provider 路径下，能用同一 CLI 契约完成同样的闭环，且结果一致到阶段级别。
- 产物包含任务 ID、阶段列表、检查点、输入摘要和输出位置，便于之后由 AgentWatcher 接管。
- 手动/半自动执行过程中不要求 AgentWatcher 在线。

### 应该避免
- 不要只验证 Copilot，然后把“多 provider”当作口号。
- 不要把 provider 适配做成各自一套独立流程。
- 不要让验证依赖隐藏状态、浏览器会话或本地临时文件约定。
- 不要把知识库生成强行塞成首发必须项；它应该是可插拔增强，而不是基础闭环前提。

## 阶段 3: AgentWatcher 发布与监控接入
### 目标
- 让 `AgentWatcher` 成为任务发布、状态跟踪、知识沉淀和流程监控层。
- 复用现有 `UEWorkflow` 插件和 `ueworkflow_execution.rs` 的记录机制，接住 CLI 产物。

### 验收标准
- AgentWatcher 能基于 CLI 产物发布任务，并把执行记录落到统一的 execution / knowledge 根目录。
- AgentWatcher 能展示任务当前状态、阶段进度和知识沉淀结果，而不是只显示一个“已发起”标记。
- AgentWatcher 的插件映射保持到 DevFlow、AgentHub、KnowledgeBase 的明确业务连接，不靠隐式路径猜测。
- 发布与监控过程仍然遵守 dry-run 先行、可审计、可回放的约束。

### 应该避免
- 不要让 AgentWatcher 反过来成为所有逻辑的中心，把 CLI 变成薄壳。
- 不要让监控层直接修改三个 repo 的工作内容。
- 不要把任务状态与 UI 状态混在一起，导致后续回放困难。
- 不要跳过 dry-run 或把“监控”做成实际执行的遮羞布。

## 阶段 4: 验证与收口
### 目标
- 用最小但真实的验证证明三层链路都成立：CLI、provider、AgentWatcher。

### 验收标准
- CLI 有单测覆盖：repo 解析、阶段生成、dry-run 产物、失败提示。
- provider 路径有至少一次真实手动/半自动验证记录，证明同一契约可跨编辑器/ provider 复用。
- AgentWatcher 有 smoke 级验证，证明发布与监控能吃到 CLI 产物并正确记录。
- 每一层都能单独失败、单独定位，不会把问题混成一团。

### 应该避免
- 不要只看“能跑起来”，不看产物结构和失败路径。
- 不要只测 happy path。
- 不要把一次成功的人工演示当成稳定流程。

## 风险与缓解
- 风险: 路径与仓库归属混乱。缓解: 明确 repo root 配置，计划文件里固定引用来源，不做模糊扫描。
- 风险: provider 差异导致流程分叉。缓解: 只允许 provider 适配层变化，CLI 契约保持单一。
- 风险: AgentWatcher 过早吸收业务逻辑。缓解: 先把它限定为发布/监控/记录层。
- 风险: 知识库与技能生成把首发复杂度抬高。缓解: 把它放在后置增强，不作为 MVP 依赖。

## 结论
- 推荐顺序是: `UnrealWorkflow` 独立 CLI -> provider 手动/半自动验证 -> AgentWatcher 发布与监控。
- KnowledgeBase 先作为知识生产/校验能力接入，不作为第一阶段主干。
- 这个顺序的核心原则是先证明“可被人和 provider 使用”，再证明“可被平台自动托管”。
