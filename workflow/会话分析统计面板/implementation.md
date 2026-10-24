---
schema_version: "1"
protocol: "1.3.0"
artifact: "implementation"
artifact_id: "ar_1FNXZNBZFFDWAMW4WC06E51AVC"
work_item_id: "wi_3KXFZQMQ9AH2XQ8PGQBW299E2P"
created_at: "2026-10-14T07:34:58Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:288b282963a92773fb16e68c5fd502e3994c05fcdb071bb4aee77785c21e33b2"
  artifacts:
    - artifact_id: "ar_1HPPZXVZT7G2H0XFHPE092TTGY"
      digest: "sha256:b4c57f818b477dc9c19c6d0147ef9233b9dab4f0a8208d41ae7aba30131bc4b9"
      locator: "change-note.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "fd56b7e56f1d5655ed61226f17481689fd883ea6"
    revision: "fd56b7e56f1d5655ed61226f17481689fd883ea6"
    tree: "1908f208f2be77bede57f4bfa560f91aef85d58c"
    content_digest: "sha256:f551acd650cf40c3cd2aa9159bf27f0c9ddbb4be707e99e50f36c6ee7ccc0c3e"
    branch_or_pr: "feature/会话分析统计面板"
    workflow_excluded: true
result: "complete"
---
# 实现回执

## 做了什么

按 plan.md S1 到 S7 完成单会话全生命周期分析视图（阶段 A）：Rust 数据面（结构体、
降采样、时序缓存、三源 JSONL 解析、ZCode/OpenCode SQL、命令分发注册）与前端
（analytics-panel 窗口、三图联动视图、分解区、预览与右键入口、双语 32 键）。

## 每步结果

- S1 结构与降采样：`downsample_lifecycle_calls` 按「同模型序列位置」分段合并，
  12000 个双模型交替点降到 5000 内且总量保真（单测
  `downsample_lifecycle_caps_points_and_keeps_sums`）。与设计一处偏差：设计写
  「连续同模型合并」，实现改为按模型各自分段——交替调用在连续口径下永不合并。
- S2 SQLite 时序：`zcode_session_timeline_on` / `opencode_session_timeline_on`
  拆成可注入连接的内核函数（照 `zcode_usage_by_session` 模式）以便 fixture 测试；
  OpenCode 的 role/tokens 都在 data JSON（json_extract），part 用 type='tool'。
- S3 JSONL 解析：Claude/Codex/Copilot 三个 timeline 解析器与聚合器同口径
  （message.id 去重、isSidechain/isMeta 过滤、Codex 全零 input 跳过）；
  `copilot_chat_collapsed_requests` 抽为共用函数。
- S4 命令：`get_session_lifecycle({request:{id,provider,sessionPath}})`，
  前端传扫描结果里的定位字段（与 open_session 同一来源约定），已注册。
- S5 到 S7 前端：窗口照 performance-panel 样板（几何、显隐、ready 握手同构），
  视图数据全部来自命令返回，i18n 两字典各加 32 键。

## 跑过哪些检查

- `cargo test --manifest-path src-tauri/Cargo.toml`：lib 104 通过 0 失败
  （含 6 条新增），cli_taxonomy 8 通过，cli_contract 8 通过。
- `npm run build:ui`：通过。
- `npm run test:tauri:smoke`：通过（真实 Tauri 壳，步骤全绿，
  `.tmp/tauri-realtest/20261014-151957/result.json`）。
- 专项 CDP 走查（worktree 自建 dev 运行时，`window.__TAURI_INTERNALS__` 为真）：
  右键菜单「会话分析」打开窗口；真实 Claude 会话 claude:c25596b2…
  「Splatmap 纯数据方案研究」显示 413M 毛输入、808 次模型调用——808 与上一任务
  调研中该样本「2225 条 assistant 记录去重后 808 个 distinct message.id」一致；
  悬停 tooltip、粒度切换、模型过滤、分解区逐项可用；预览入口切到真实 ZCode 会话
  （53.7M、234 次调用，即本次对话自身）；主窗口切中文后标题变「会话分析」、
  粒度变「小时」。截图与 walkthrough.json 在 `.tmp/tauri-realtest/analytics-walkthrough/`。

## 哪里没按计划走

1. 时序缓存独立于扫描缓存（设计偏差，理由见 change-note）。
2. 降采样从「连续同模型」改为「按模型分段」（见 S1）。
3. 第一次冒烟检测到主检出残留 Vite 端口，有挂错 UI 的风险；已用 worktree 自建
   dev 运行时重做 CDP 走查确认测的是本分支代码。
4. AC-007 的 Verify 是「smoke 加面板截图」：smoke 本身未覆盖新面板，补了专项
   CDP 走查（含截图）作等效证据；走查未固化成 `test:tauri:flow` 项，留给后续。

## 用户实测反馈的三个缺陷与修复（de62853）

1. 窗口关不掉：标题栏 pointerdown 无差别 startDragging，Windows 移动循环吞掉
   真实鼠标在关闭按钮上的 click（Playwright 合成点击测不出该路径，教训：窗口
   按钮必须走真实指针事件验证）。照 startWindowDrag 的守卫跳过按钮命中，另加
   Escape 关闭。经真实 mouse.down/up 路径复验：点击与 Esc 都能隐藏窗口。
2. 缩放时用量时间轴逐渐消失：逻辑尺寸、连续风暴、页面级缩放三条程序化路径
   均未能复现，按机制防御：rAF 结算后量宽、resize 事件兜底、极值展开改循环
   （超大数组 spread 会 RangeError 中断绘制）、grid 项 min-width:0、svg 块级。
   风暴复验 viewBox 追随窗口宽度并完整恢复。若用户复测仍现，需记录具体操作
   （拖边/最大化/Ctrl+滚轮）再定位。
3. 高频工具条无填充：span 为 inline，height 属性不生效；track/fill 显式
   display:block 修复，复验填充宽度与背景色正常。

## 第四轮实测反馈：时间轴一根线（7db6db0）

用户以「重构南侧厂房一楼3DGS场景」（codex:01a1398b…）为例报用量时间轴成一根
直线。数据核对：90 次调用、73 分钟跨度、上下文 49k→177k、无超过 40% 的骤降
（该会话本就未压缩）、窗口上限 828k 远高于峰值。根因是展示口径：默认小时粒度
把整个会话落进一个桶；y 轴固定 20M 取整把小会话压平。修复：粒度加「自动」档
并设为默认（1 分钟到 7 天台阶按跨度选桶宽）、y 轴 1/2/5 自适应、x 轴刻度自适应
并在分钟桶显示 HH:mm、压缩标注最多三个且间隔拉开。复验：15 分钟桶、y 轴 500k、
x 轴 16:45–17:45，柱形起伏可见。已知口径差：Codex 逐调用求和（9.9M）与累计
total_token_usage（9.25M）差约 7%，时间轴与按模型分解用逐调用口径，指标行用
累计口径，两者并存属 Codex 数据源语义（上一任务调研已记录该差异）。

## 第五轮实测反馈（本轮提交）

1. 悬停详情框钉在图表顶部盖住柱形（用户视角「图表被挡」）：改为跟随光标、
   贴边翻转；合成 mousemove 复验 tip 落在光标旁而非顶部。
2. 分析窗口滚动条为浏览器默认样式：照主窗口 session-scroll 同款补齐
   （dark #5f5f5f/#252526、light #c8c8c8/#f3f3f3，10px 圆角）。

## 第六轮：缩窗口压成竖线的机制封堵（e47d13e）

用户复测仍复现。隔离真实壳（1421+CDP 9224，规避与主检出 dev 的端口冲突）多轮
set_size 风暴、交替尺寸、极限 560x480 均无法复现，但注入实验抓到了确凿机制：
把 viewBox 人为改大后，svg 内固定坐标被整体压缩成左侧 6px 细条——即任何原因
（漏掉的 resize 事件、中间态测量）造成 viewBox 与宿主宽度失配时，呈现就是
「一条竖线」，且 preserveAspectRatio=none 防不住（实验证伪）。修复分三层：
viewBox 宽与宿主宽差超 2px 时立即用新鲜测量重绘（自校验）、三个图表宿主直接
挂 ResizeObserver、窗口 resize 事件继续兜底；capability 补 maximize/unmaximize。
复验：所有 resize 路径 viewBox 始终等于宿主宽；注入陈旧 viewBox 后触发 resize
即被纠正。另发现并绕开的环境问题：用户 10:08 起的 dev 来自主检出（dev 分支，
无本功能），复测须从 worktree 起 dev。

## 第七轮：关闭问题按像素间歇复现的根因（本轮提交）

上轮修复只在 pointerdown 判断按钮命中，高 DPI 下点在 × 边缘时 target 是
标题栏而非按钮，startDragging 照样吞掉 click——用户按像素位置间歇踩中。
改为移动阈值拖拽（累计移动超 4px 才 startDragging，与原生标题栏语义一致），
× 命中区加大到 34x26。真实鼠标九点关闭全过，拖拽复验仍生效。

## 第八轮：轨迹泳道落地（5f068bb + 2a5a3cb，S9–S12）

按 design/agent-trajectory-design-v2 施工：Rust 子代理发现（thread_spawn 精确 +
guardian 时间窗近似、rollout_time_bounds 首尾时间戳、预算外用量变体）与编排事件
采集；前端轨迹泳道区（收拢默认、泳道点击懒加载合并明细、并行度曲线、效率排行、
窄窗 clamp 网格 + 排行横滚）。用户实测反馈「窄窗两个新控件被外框裁切」已修：
三列网格标签列 clamp 响应式收窄、参考线改按轨道实测定位、排行套滚动，560x480
复验无溢出。真实数据对数修正两处认知：本会话 16 个 worker 全部 gpt-6.1-sol
（gpt-6-astra 的 rollout 属另一父会话 01a15802，原型里「06:36 切换」归因有误，
参考线不出现是正确行为）；父文件 wait_agent 计数为 0（此前 grep 到的是工具声明），
waitIntervals 为空正确。回归：cargo 全量 123 通过（含 3 条新单测）、build:ui 过、
CDP 真实会话走查（收拢/展开/明细懒加载/排行/并行度）全过，截图在
`.tmp/tauri-realtest/analytics-walkthrough/traj-impl-*.png`。

## 第九轮：二次审查（本轮提交）

自查抓到重大问题：上一轮「窄窗裁切修复」脚本在断言处失败整体未落盘，只留下
一个未定义函数的孤儿调用（resize 时 ReferenceError）。本轮完整落地并验证：
响应式网格两档断点、排行横滚、参考线实测定位；另修子代理异模型取色回退、
短命 worker 明细分桶两缺陷。复验 560x480 零溢出、短命明细正常、零运行时
错误；子代理合计修正 77.3M。教训（写给复盘）：多段 python 补丁脚本必须
原子落盘并在脚本尾部回读校验关键锚点是否真的生效，断言失败静默等于没修。

## 第十轮：窄窗泳道压扁（本轮提交）

用户实测窄窗轨迹泳道区压成细线。修复：行高/轨道高双向锁死、面板与标签
min-width:0、resize 自校验扩展泳道几何检测（坏即整区重渲染）。560/480/880
三档实测几何恒定。另排查中确认一个 dev 环境现象：中文路径 worktree 下 Vite
对 `@tauri-apps/api` 裸说明符的 rewrite 偶发不生效（窗口动态 import 首次
静默失败，第二次点击成功）——不影响发布产物（ui-dist 用 build:ui 打包），
作为已知 dev 环境行为记录。

## 第十一轮：两个分析入口不统一（本轮提交）

用户实测右键与悬浮窗按钮入口结果不一致。定位到三个叠加根因并修复：
sessionData 随滚动队列重建导致解析失败静默留旧（加常驻目录）、预览路径
只发 sessionId 依赖主窗口查表（改带完整 target 直达，顺带修预览 payload
缺 provider/sessionPath）、无子代理会话整块隐藏轨迹区被误读为旧版
（改常显加明确空态）。验证：同会话两入口结果一致；无子代理会话显示
「该会话没有子代理」空态。

## 第十二轮：主体布局改块级流，终结面板裁切类问题（本轮提交）

用户明确 directive：缩小窗口只应整体竖向滚动，不得压缩裁切任何子面板。
活体扫描最终抓到一直以来的真正机制：分析主体是 CSS Grid，部分尺寸下
行高被解析成 1.33px（此前「图表逐渐变一根线」「面板被裁切」全部源于此，
此前的 SVG 宽度自校验修的是表层不是根因）。主体改块级流：面板自然高度
不可压缩，缩小窗口只产生 body 整体竖向滚动。四档尺寸实测零塌缩零横溢。
教训（写给复盘）：同一症状第三次修复才命中根因，第二次就该做逐尺寸
几何扫描而不是继续在渲染层加保险。

## 还剩什么风险

- 主题切换同步只验证了语言事件路径（同一 settings 事件），未逐项截图。
- 超长会话（超 5000 调用点）的降级分支只有单测，没有真会话实测。
- OpenCode 时序查询未在 4.46GB 真库上压测最坏耗时。
