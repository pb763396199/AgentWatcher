---
schema_version: "1"
protocol: "1.3.0"
artifact: "design"
artifact_id: "ar_3FY7V5SQZEN596WM62158WQY55"
work_item_id: "wi_1QYFZ7SPZ2TGAMZAMK448JP45C"
created_at: "2026-11-13T08:53:40Z"
producer: "aes-brainstorm"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:807ccdf792d9da9509eddb8e1cc706954c00ea16bf8f9c44bb645b13b854af56"
  artifacts: []
result: "accepted"
---
# 多会话任务服务实施依据

## 采用的设计

用户已明确要求依据[框架设计](../AES-Workflow-多会话调度框架设计/design.md)编写计划并施工。本任务继承其中的行为、平台范围、插件边界、确认规则和首版验收场景，不新增产品取舍。

## 实施组织

通用代码新增在 src-tauri/src/workflow/，经 lib crate 提供给 CLI 和 GUI。现有 session 查询保留原路径。服务通过单例锁、用户私有名片和本地请求提供统一数据库入口。复杂命令从 JSON 文件或 stdin 读取，命令名通过固定枚举分发，不执行输入中的任意 shell。

Codex 使用独立运行器持有会话；Git 操作使用 Command 参数数组和已登记仓库，先检查版本与工作区，再生成和更新候选。所有外部动作先记操作身份。结果不明时核对，不自动重放。

## 备选与理由

| 方案 | 结果 |
| --- | --- |
| 通用 Rust 服务和 SQLite | 采用；复用现有依赖与跨平台 CLI。 |
| 在界面 JavaScript 中保存执行状态 | 放弃；页面关闭会失去控制，不能统一多个 CLI 的事务。 |
| 把 AES 业务源码加入宿主 | 放弃；违反仓库插件规则，宿主只提供通用能力与适配接口。 |

## 验证与范围

数据库与 CLI 先用隔离临时目录验证；Codex 在临时仓库做真实执行；Git 用临时仓库与 worktree 验证版本与冲突。人工确认不会由测试替真实任务授权。实现步骤和证据写入 plan.md 与后续记录。
