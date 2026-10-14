---
schema_version: "1"
protocol: "1.3.0"
id: "wi_19X7WBYXTZK9DH6X1KT55XHK7N"
short_id: "nf31t3pn"
title: "主窗口搜索栏支持用会话 ID 过滤卡片"
status: "done"
kind: "feature"
created_at: "2026-10-14T02:10:49.499330500+00:00"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
branch_or_pr: "feature/主窗口搜索栏支持用会话-ID-过滤卡片"
revision: "2"
updated: "2026-10-14"
---
# 主窗口搜索栏支持用会话 ID 过滤卡片

## 原始请求

用户原话（2026-10-14）："搜索栏要支持用id来搜索，如01a1360f-7573-7ed0-b25b-35cddae9eeaf"

## 目标

把一个会话 ID 的片段粘进主窗口搜索栏，就能在卡片列表里定位到那张卡片。

## Why

搜索栏现在只匹配卡片标题（`ui/index.html` 的 `cardMatchesBaseFilters`，把查询转小写后对 `sessionTitle` 做 includes）。会话标题是从会话内容采样出来的摘要，不同会话可能同题；用户从终端、日志或 `agentwatcher-cli` 输出里拿到的是会话 ID（`provider:自然键`，如 `claude:01a1360f-7573-7ed0-b25b-35cddae9eeaf`），目前没有入口按它找卡片。

## 范围

做：

- 主窗口 Watcher 搜索框（`#sessionSearch`）的过滤逻辑在标题之外同时匹配会话 ID：子串、大小写不敏感，与现有标题搜索同一语义。不带 provider 前缀的自然键（完整 UUID 或其任意连续片段）自然命中。
- 冒烟测试（`tools/tauri-realtest/cli.mjs`）在现有标题搜索断言之后加一步 ID 片段搜索断言。

不做：

- 不改 workspace 选择器搜索（另一个输入框，按 workspace、路径、分支过滤）。
- 不改 `agentwatcher-cli`（按 ID 查会话走 `session show <id>`，已存在）。
- 不改 Rust 侧扫描、状态机和 Tauri 命令面。

## 强约束

- 过滤仍是纯前端行为：不新增 Tauri 命令、不改事件契约。
- 隐私边界不变：搜索只作用于已扫出的内存卡片数据，不新增落盘。
- 现有过滤行为（标题搜索、workspace、provider、状态、清空恢复）不回退。

## 自主边界

| 不用问直接定 | 必须停下来问 |
| --- | --- |
| 匹配语义取子串、大小写不敏感（与现有标题搜索一致） | 无。用户原话已给出期望行为和示例 |
| ID 与标题是"或"关系：任一命中即显示该卡片 | |
| 测试断言沿用现有标题搜索步骤的模式 | |

## 验收条件

- AC-001: 在主窗口搜索栏输入某张卡片会话 ID 的连续片段（包括不带 provider 前缀的自然键、完整 `provider:自然键`、大写输入），列表只剩标题或 ID 包含该片段的卡片。Verify: manual:在真实 Tauri 壳上跑 npm run test:tauri:flow -- 筛选，ID 片段搜索步骤通过且失败不出现在该步骤
- AC-002: 现有过滤行为不变：标题搜索、workspace/provider/状态过滤、清空后恢复全部卡片。Verify: manual:同一环境跑 npm run test:tauri:smoke，现有断言全部通过

## 访谈记录

| 时间 | 谁说的 | 内容 | 结论 |
| --- | --- | --- | --- |
| 2026-10-14 | 用户 | "搜索栏要支持用id来搜索，如01a1360f-7573-7ed0-b25b-35cddae9eeaf" | 需求与示例明确，无分叉，未追问 |

## 设计取舍

| 方案 | 怎么做 | 代价 | 结论 |
| --- | --- | --- | --- |
| 一直同时匹配标题和 ID | 过滤函数对 `data-session-id` 多做一次 includes | 非 UUID 查询也会对 ID 比一次，默认 80 张卡片的量级无感知 | 采用 |
| 识别出 UUID 形态才启用 ID 匹配 | 用正则判断查询是否形如十六进制片段，再切到 ID 匹配分支 | 两套匹配语义；片段该多长才切分支要单独定义，边界说不清 | 不采用 |
| 什么都不做 | 维持只搜标题 | 拿着 ID 找不到卡片，需求不满足 | 不采用 |

## revision 历史

| 版本 | 日期 | 为什么改 |
| --- | --- | --- |
| 1 | 2026-10-14 | 首版 |
| 2 | 2026-10-14 | 两条 Verify 的命令写错入口：ID 片段断言在筛选流程（npm run test:tauri:flow -- 筛选），不在冒烟流程 |
