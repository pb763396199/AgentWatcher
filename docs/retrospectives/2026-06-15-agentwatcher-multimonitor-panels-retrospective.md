---
title: AgentWatcher 多屏子面板可见性修复复盘
purpose: retrospective
status: done
date: 2026-06-15
language: zh-CN
evidence_refs:
  - ui/index.html
  - tools/tauri-realtest/cli.mjs
  - .tmp/tauri-realtest/20260615-120832/result.json
  - .tmp/tauri-realtest/20260615-121413/result.json
  - .tmp/tauri-realtest/20260615-121449/result.json
---

# AgentWatcher 多屏子面板可见性修复复盘

## summary

本轮目标是修复 AgentWatcher 主窗口拖到非主屏幕后，创建接续面板时面板不可见的问题，并检查其它独立子面板是否存在同类风险。最终修复覆盖了 `handoff-panel`、`performance-panel`、`todo-panel`、`session-preview` 四类 Tauri WebViewWindow 的定位、显示和失效句柄处理。

任务闭环状态：已完成。核心验收点包括三屏真实 Tauri 环境下副屏 `DISPLAY43`（scale 1）和 `DISPLAY46`（scale 2.25）的 handoff / performance / preview 首次创建可见性验证，以及项目自带真实 Tauri smoke、filtering、接续面板流程回归。

## changes

- 将子窗口定位从“物理坐标读取 + 逻辑坐标设置”的混合路径，改为基于当前 monitor `workArea` 的物理坐标计算，并优先用 `PhysicalPosition` 设置窗口位置。
- 新增统一 reveal 路径，集中处理子窗口的 `setSize`、物理定位、runtime settings 同步、数据同步、`show()` 和 `setFocus()`。
- 为首次创建窗口增加延迟 reveal fallback，避免 `tauri://created` / ready 事件时序丢失后窗口长期保持 hidden。
- 新增 `usableNativeWindow()` 验活逻辑，修复子窗口被外部关闭后 JS 缓存句柄仍存在，导致后续打开既不重建也不显示的问题。
- 将 session preview 的位置计算改为主窗口物理坐标加 DOM anchor 的缩放偏移，再 clamp 到当前 monitor workArea。
- 扩展真实测试中的 filtering flow，并修正测试自身两个脆弱点：下拉控件被 compact rail hit-test 拦截、todo fixture 选中了已有 active session 的 workspace。

## validation

- `npm run build:ui` 通过。
- `npm run test:tauri:smoke` 通过，结果文件：`.tmp/tauri-realtest/20260615-120832/result.json`。
- `npm run test:tauri:flow -- filtering` 通过，结果文件：`.tmp/tauri-realtest/20260615-121413/result.json`。
- `npm run test:tauri:flow -- 接续面板` 通过，结果文件：`.tmp/tauri-realtest/20260615-121449/result.json`。
- 手工脚本通过 WebView2 CDP 附着真实 Tauri，识别三块显示器：
  - `DISPLAY42`：主屏，scale 1.5，workArea `(0,46) 3840x2114`。
  - `DISPLAY43`：副屏，scale 1，workArea `(3840,0) 1080x1920`。
  - `DISPLAY46`：副屏，scale 2.25，workArea `(695,2160) 2732x2048`。
- 在 `DISPLAY43` 与 `DISPLAY46` 上验证：`performance-panel`、`handoff-panel`、`session-preview` 首次创建后 `visible=true`，窗口物理坐标均落在当前副屏 workArea 内。
- `todo-panel` 当前主界面入口会进入 AgentTask 内嵌面板，不直接弹独立窗口；本轮验证的是其独立窗口定位路径，物理坐标落在当前副屏 workArea 内。

## blockers

无阻塞项。过程中出现的失败均已归因并修复：

- handoff 首次点击后无窗口：不是菜单点击失败，而是窗口 ready/show 握手时序存在 hidden 风险。
- 强制 close 子窗口后再次打开失败：缓存 JS handle 已失效但仍被复用。
- filtering 回归初次失败：测试 fixture 插入到了已有 waiting/running 会话的 workspace，按产品逻辑本来就不应显示 ready nudge。
- filtering 回归第二次失败：自动化 pointer click 被 compact rail 拦截，改为 DOM click 后稳定。

## required_follow_ups

- 建议把本轮临时多屏 CDP 验证沉淀为正式 `multi-monitor` realtest flow；现有验证证据充分，但脚本尚未固化到仓库流程。
- 如果未来 Todo 独立窗口重新暴露为主界面直接入口，需要补一条真实用户触发路径验证，而不是只验证定位 API 路径。
- 建议在子窗口创建逻辑里继续避免分散重复代码；这次已经收敛 reveal 路径，后续新增窗口应复用同一模型。

## 反思

这次问题的真实根因不止一个“非主屏坐标错了”。最早观察到的是 handoff 在副屏看不见，但实际拆开后有三层问题叠在一起：坐标系混用会让窗口漂移，首次创建过度依赖 ready 事件会让窗口保持 hidden，缓存的 WebViewWindow 句柄在 native 窗口关闭后还会误导后续打开逻辑。只修第一层会让问题在某些机器上看似消失，但用户继续拖屏、关面板、再打开时仍可能复现。

执行上最大的教训是：这类 Tauri 桌面壳问题不能靠纯网页预览推断。网页 DOM 看起来正确，不代表 native window 的物理坐标、DPI 缩放和显示器 workArea 正确。后续遇到窗口、置顶、子面板、悬浮预览这类问题，应第一时间进真实 Tauri + WebView2 CDP，而不是先做浏览器 demo 或截图级判断。

测试侧也暴露了一个坏味道：测试 fixture 原本假设“有 ready todo 就一定会显示 nudge”，但产品逻辑里 workspace 有 waiting/running 会话时会抑制 nudge。这个失败很有价值，它提醒测试必须编码业务约束，而不是只编码理想数据形状。修测试时没有绕开产品逻辑，而是让 fixture 选择符合产品条件的安静 workspace。

本轮收敛后的模型更稳：所有子窗口先验活句柄，再用物理坐标定位，再统一 reveal；ready 事件是加速路径，不再是唯一显示路径。这个结构比针对 handoff 单点补丁更可靠，也降低了 performance、preview、todo 将来复发同类问题的概率。

## doc_contract_status

pass

## artifact_purpose

retrospective

## artifact_language

zh-CN

## canonical_path

docs/retrospectives/2026-06-15-agentwatcher-multimonitor-panels-retrospective.md

## legacy_violations

[]
