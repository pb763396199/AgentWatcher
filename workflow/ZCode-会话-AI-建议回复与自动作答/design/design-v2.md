---
schema_version: "1"
protocol: "1.3.0"
artifact: "design"
artifact_id: "ar_01M65T78VY000B46E2GATW3KB5"
work_item_id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
created_at: "2026-10-30T07:00:00Z"
producer: "aes-brainstorm"
supersedes: "ar_0KMYDEZFWFEYQ4A58SPWM0MAV4"
dependencies:
  work_item_contract_digest: sha256:2b1c17e9e8d949086387fef35bc752d834b0026f368ec9f17c071b583ecf4d0a
  artifacts: []
result: "accepted"
---
# 设计 v2：零配置，用会话自己的引擎生成建议

用户 2026-10-30 纠正：这个功能不应该在设置里有任何参数，不应该填模型；每个会话卡片应该复用该 agent 自己的原始设置——这正是 ChatGPT 的流程（预测跑在线程自己的模型和账号上，经 thread/sample 分叉，客户端零配置）。v1 引入的外部模型 API 与设置分组整个方向错了，本版取代。

## 改成什么样

- **生成引擎换成 ZCode 本身**：用 `ELECTRON_RUN_AS_NODE=1` 运行 `zcode.cjs -p "<提示词+上下文>" --cwd <临时目录> --json`，模型、账号、供应商全部沿用用户在 ZCode 里的现有配置，AgentWatcher 不碰任何凭据。
- **临时目录**：`%TEMP%\\AgentWatcher\\suggestion-scratch`。落在 %TEMP% 下的会话本来就被 AgentWatcher 默认的工作区黑名单挡住，不会在卡片墙上出现（复用现有 workspacePathBlacklist 机制，不加新过滤代码）。
- **交互入口**：会话卡片右键菜单 + 卡片幽灵行。ZCode 会话新进入 waiting 自动生成（ChatGPT 同款节奏）；右键「换一条建议」按需重生成。生成完成经既有建议事件点亮卡片幽灵行/选项芯片，动作仍是 复制/发送/换一个。
- **全自动作答**：右键「自动作答此会话的选项」按会话开关（菜单项打勾），预算、连续同答案熔断、审计三道护栏不变，常量内置（每小时 6 次、等 20 秒），不再进设置面板。
- **删掉的东西**：设置面板「AI 建议」九项、外部模型 API 调用与 ureq 依赖、全局热键与 tauri-plugin-global-shortcut 依赖、set_suggestion_settings 命令。热键属 v1 私加，随设置一起删。

## 复用与新增

保留 v1 已实现且仍然成立的部分：状态转换检测、上下文提取、v5 改造提示词、输出契约解析、指纹防陈旧、发送通道、护栏纯函数、审计落盘。新增：无头生成调用（复用发送通道的进程机制，参数从 `--resume -p` 换成 `-p`）、右键菜单两个入口、按会话的自动作答开关。

## 代价（用户已接受的方向，写明数字）

- 每次生成消耗一次用户 ZCode 账号的真实模型调用（v1 的外部 API 是另掏一份钱，这才是"复用原始设置"）。
- 每次生成会在 ZCode 的会话库里留下一个 scratch 会话：AgentWatcher 侧被默认黑名单挡住；ZCode 桌面端的会话列表会看到它们积累。缓解：临时目录固定一个，便于人眼识别；未来 CLI 提供删除时再清理。
- 无头一轮耗时约二十到六十秒（实测试点），远慢于 v1 的直连 API。自动档在等待场景可接受；右键按需时菜单先显示"生成中"。

## 未决问题（沿用 v1 的两条，加一条）

1. ZCode 桌面窗口对命令行续话的实时刷新行为，随人工 M2 验证。
2. headless 的 --mode 沿用会话现状，不显式传。
3. 生成会话当前用 ZCode 默认模型；目标会话若用了别的模型，建议可能慢一档。后续可从会话库读出该会话实际模型并透传 `--model`，本轮不做。
