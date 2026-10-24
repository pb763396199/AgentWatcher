---
schema_version: "1"
protocol: "1.3.0"
artifact: "change-note"
artifact_id: "ar_1HPPZXVZT7G2H0XFHPE092TTGY"
work_item_id: "wi_3KXFZQMQ9AH2XQ8PGQBW299E2P"
created_at: "2026-10-14T07:34:58Z"
producer: "aes-execute"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:288b282963a92773fb16e68c5fd502e3994c05fcdb071bb4aee77785c21e33b2"
  artifacts:
    - artifact_id: "ar_4BNZYPTFCZ2947619P78B2X5ZY"
      digest: "sha256:5b6d625991807dbbd0eb44e04a72c7e25416adf7cb37b4bc7eae8597bc8e3957"
      locator: "plan.md"
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
result: "draft"
---
# 修改说明

## 覆盖账本

| 文件 | 提交 | 计划步 |
| --- | --- | --- |
| src-tauri/src/lib.rs | 540a4ce | S1 S2 S3 S4 |
| src-tauri/capabilities/analytics-panel.json | 540a4ce | S5（capability 与 Rust 侧一并提交） |
| ui/index.html | fd56b7e | S5 S6 S7 |

## 修改理由

- 新增 `get_session_lifecycle` 命令与 `LifecycleCallPoint`/`SessionLifecycle` 结构、
  `downsample_lifecycle_calls` 降采样：让前端能按时间轴拿到逐调用用量
  （S1，设计「数据侧」节）。
- ZCode 走 `model_usage` 逐行、OpenCode 走 assistant 消息 `data.tokens`、
  Claude/Codex/Copilot Chat 三个 JSONL timeline 解析器（复用聚合器的事件选择与
  message.id 去重口径）、Codex rollout 链拼接：六源时序数据面（S2 S3）。
- 时序走独立懒加载缓存 `JSONL_TIMELINE_CACHE`，不进扫描热路径：设计原文写
  「聚合器在增量缓存值里保留逐调用数组」，实现改为独立缓存，理由是扫描缓存
  每轮逐会话 clone，塞进上千个点放大内存与拷贝；懒加载只在用户点开时读一次
  （上一任务实测 37–65ms/文件）。
- `parse_copilot_chat_usage` 抽出 `copilot_chat_collapsed_requests` 共用函数：
  聚合器与时间轴共用同一套键值补丁折叠，避免两份口径漂移（S3）。
- analytics-panel 窗口、生命周期视图（三图联动、分解区、粒度与模型过滤）、
  预览与右键两个入口、双语字典：S5 到 S7，对应 AC-001 到 AC-003 与 AC-007。
- capability 文件照 performance-panel.json 样板：窗口权限最小集。

## 代码范围

src-tauri/src/lib.rs（约 +700 行）、src-tauri/capabilities/analytics-panel.json（新）、
ui/index.html（约 +990 行）。无其他文件。

## 未解释或例外改动

无。Cargo.toml 在构建中出现过行尾假差异，已 checkout 还原，未进提交。
