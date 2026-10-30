---
schema_version: "1"
protocol: "1.3.0"
artifact: "design"
artifact_id: "ar_01M63D46ME00012C350EWBQ3AP"
work_item_id: "wi_5VYQZTXYZD0H5M6CN9TKTRFJC3"
created_at: "2026-10-29T08:40:00Z"
producer: "aes-brainstorm"
supersedes: "ar_7KQYQVDZFB4P0TY6T8K70B38QK"
dependencies:
  work_item_contract_digest: sha256:6848ab9fb6f89246f58decd1503a13d728d7aa614109393c23af3b6f2ffae203
  artifacts:
    - artifact_id: "ar_51YQVBEYQZ5KTHC6V0BP3BT5B9"
      digest: sha256:b273c0fa3a5000025905e06a766fd207b60fdc3aabe28b1685326f6ace0fd697
      locator: "research/provider-audit-signals-research.md"
result: "accepted"
---

# 逐轮降智审计器设计（第二版：并入会话分析面板）

## 这一版改了什么

第一版把逐轮审计表放进悬浮预览面板、另起一个 `get_session_turn_audit` 命令。用户指出应该和已有的会话分析（analytics-panel 单会话视图）合在一起。核对代码后这个方向成立且更省：会话分析的数据通道 `session_lifecycle`（lib.rs，`SessionLifecycle.calls` 的点位结构 `LifecycleCallPoint` 已含 `t_ms`、`model`、输入输出 token、`context_tokens`，六个 provider 各自的 `parse_*_timeline` 已在跑）就是逐轮审计该走的路——它缺的只是请求模型、档位、重试、错误这几个维度。

## 要解决什么

不变：每一轮是谁在回答、什么档位、花了多少、有没有换模型或重试，从体感变成可核对的数据。

## 非目标

不变：不抓原始思维链、不干预、不新增网络请求；审计结构里只有元数据没有正文。

## 方案对比

| 方案 | 怎么做 | 代价 | 为什么没选 |
| --- | --- | --- | --- |
| A1（第一版）悬浮预览放表 + 新命令 | 预览面板塞 14 列宽的表；新开 `get_session_turn_audit` | 预览面板宽 400px 放不下；与已有的按模型时间轴、模型分解两套口径并存，同一个「模型」两处算 | 被用户否了，且确实重复 |
| A2（本版）并入会话分析 | 扩展 `session_lifecycle` 的点位与返回；分析面板加信号标记与审计表；预览只留一行摘要 | 分析面板代码要动（时间轴标记 + 新 section） | ——（选定） |
| B 本地审计库累积 / C 什么都不做 | 见第一版，理由不变 | 同第一版 | 同第一版 |

## 选定方案

### 数据：扩 `session_lifecycle`，不新增命令

`LifecycleCallPoint` 追加字段（全部可空，缺就 null）：`modelRequested`、`effortOrVariant`、`retries`、`errorType`、`contextExceeded`、`reasoningTokens`、`costUsd`、`ttftMs`。四类信号（模型变化、档位变化、用量异常、可靠性）在 Rust 侧算好放进每个点位（`signals: Vec<String>`），前端不再各算一遍。映射来源就是调查记录的六源字段表：

| 新字段 | Claude | Codex | OpenCode | ZCode | Copilot Chat |
| --- | --- | --- | --- | --- | --- |
| `modelRequested` | `requestedModel` | null | null | null | `modelId` 去前缀 |
| `effortOrVariant` | `effort`/`perTurnEffort` | `turn_context.effort` | `data.variant` | `variant` | null |
| `retries`/`errorType`/`contextExceeded` | null | null | null | `model_usage` 对应列 | null |
| `reasoningTokens` | `thinking_tokens` | null | `tokens.reasoning` | `reasoning_tokens` | null |
| `ttftMs` | null | null | null | `time_to_first_token_ms` | null |

Copilot CLI 分支维持现状（返回降级结果），分析视图里已有降级布局，审计表显示「该数据源不支持」。

### 展示：分析面板为主，预览只留摘要

1. **用量时间轴加信号标记**：模型切换画竖参考线（轨迹泳道已有同款「跨泳道模型切换参考线」，同一画法提到主时间轴）；低输出、重试画点标记。悬停当桶时明细里带上信号；点击标记滚动定位到下方审计表对应行。
2. **分析面板新增可折叠「逐轮审计」表**（列：轮次、模型、档位、输出 token、首 token、信号），完整表住在这里，不挤预览。
3. **悬浮预览只加一行**：`最近 gpt-6-luna · 3 个信号`，点击走既有「会话分析」入口打开分析视图。这行由 `session_lifecycle` 懒加载（预览打开时取一次，不进扫描热路径，Codex 全文件读的代价只发生在用户真的悬停时）。
4. **按模型分解加档位维度**：`model × variant` 分组（有 variant 的源），与审计表同源同口径。
5. 中英文、dark/light、隐私边界（不进 localStorage、无正文）全部沿用既有口径。

原型稿：`mockups/turn-audit-analytics.html`（分析视图合并版：时间轴信号标记 + 审计表 + 模型×档位分解）；第一版的预览小表原型 `mockups/turn-audit-panel.html` 降级为「预览摘要行」参照保留。

### 场景走查

- 顺利：用户觉得回复变差 → 悬停卡片看到「3 个信号」→ 点进会话分析 → 时间轴第 12 轮一条模型切换参考线 → 点标记跳到审计表该行，模型从 `gpt-6.1-sol` 变 `gpt-6-luna`，一目了然。
- 出错：源缺字段 → 点位对应列为 null，无信号；`session_lifecycle` 超预算降级时审计表一并降级为累计口径（沿用 AC-006 的既有行为）。
- 中断回来：无状态要恢复，重开分析视图即重算。

## 影响面

复算：`grep -c "parse_.*_timeline" src-tauri/src/lib.rs`、`grep -n "LifecycleCallPoint" src-tauri/src/lib.rs`（定义在 lib.rs:188）。

| 落点 | 改动 | 相对第一版 |
| --- | --- | --- |
| lib.rs：`LifecycleCallPoint` 与六个 `parse_*_timeline`/`zcode/opencode_session_timeline` | 点位追加可空字段 + 信号判定，预计 300–450 行 | 少一个新命令、少一套独立提取器（复用时间序解析），总量更低 |
| ui/index.html 分析面板段 | 时间轴信号标记、审计表 section、模型×档位分解，预计 150–200 行 | 预览侧只剩一行摘要（约 20 行），比第一版少 |
| 悬浮预览 | 一行摘要 + 点击进分析（复用既有入口） | —— |
| `export_session_package` | 包内 `audit/turns.ndjson` 不变 | 不变 |

## 边界与失败

第一版关于 OpenCode 大库按会话过滤、Codex 全文件读上限、SQLite 只读加超时的边界全部沿用；另加一条：审计字段跟随 `session_lifecycle` 的降级路径——超预算降采样时信号只基于已采到的点位，并在返回里带 `degraded`（字段已有），UI 显示「已降级」。

## 怎么算做对

AC-001–006 的映射不变；AC-004 的「UI 可见」落点改为：预览摘要行 + 分析面板审计表两处，`test:tauri:smoke` 与既有会话分析相关 flow 覆盖。

## 未决问题

第一版三条（CLI 子命令、阈值配置、跨会话趋势）不变；其中「跨会话趋势」在并入分析面板后与阶段 B 的跨会话聚合视图是同一个家，将来一起做。
