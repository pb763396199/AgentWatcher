---
schema_version: "1"
protocol: "1.3.0"
artifact: "plan"
artifact_id: "ar_1ZPZFSSPSZ1NFNZ0BXRH5HFXDQ"
work_item_id: "wi_1QYFZ7SPZ2TGAMZAMK448JP45C"
created_at: "2026-11-13T08:53:41Z"
producer: "aes-plan"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:807ccdf792d9da9509eddb8e1cc706954c00ea16bf8f9c44bb645b13b854af56"
  artifacts:
    - artifact_id: "ar_3FY7V5SQZEN596WM62158WQY55"
      digest: "sha256:4a893d322ee93da68b92f8589d02f6389884270f3737a62ab0ef92e72d56bcde"
      locator: "design.md"
result: "ready"
---
# 多会话任务服务施工计划

## 基线和执行边界

在 AgentWatcher 主检出 dev 施工，遵从用户的仓库分支要求。当前已有设计记录和待办文件不回滚、不混入业务变更。代码与记录分开保留，本次不自动提交、推送或创建 PR。具体 AES 插件在独立目录接入；宿主源码只建设通用框架。

## 步骤

| 步骤 | 输入与修改文件 | 做什么 | 验证命令与预期 | 证据与退回 |
| --- | --- | --- | --- | --- |
| P1 | 新增 src-tauri/tests/workflow_contract.rs，扩充 src-tauri/tests/cli_taxonomy.rs | 先固定请求与错误信封、原子认领、幂等收件、版本检查、暂停和反思场景。 | cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --test workflow_contract；实现前应明确因缺命令失败，实现后全通过。 | 保留失败与成功输出；测试只用临时目录。 |
| P2 | 新增 src-tauri/src/workflow/{mod,store}.rs，lib.rs 注册 | 建 SQLite 迁移、项目/路线/节点/执行/交付/评审/事件与请求去重。 | cargo test --manifest-path src-tauri/Cargo.toml --no-default-features workflow::；并发和事务回滚断言通过。 | 数据库只建在测试目录；失败保留样本，不操作真实业务库。 |
| P3 | 新增 workflow/service.rs，CLI cli.rs/main.rs/workflow_commands.rs | 单例、按需唤起、私有名片、本地鉴权、服务退出及 JSON 请求；旧 session 命令不改。 | cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --test workflow_contract；真实子进程启动、退出、唤起和幂等通过。 | 保存测试服务日志；每次测试明确 stop，不按端口杀进程。 |
| P4 | workflow/store.rs 与 workflow_contract.rs | 交付、独立评审、人确认、改线冻结和回执；两次无进展反思、三次暂停。 | cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --test workflow_contract；跨项目隔离、失效授权与老归属拒写通过。 | 事件序列可核对；不设置上下文硬上限。 |
| P5 | 新增 workflow/git.rs，新增 Git 临时仓库测试 | 登记 workspace，准备节点分支/worktree，roadmap 集成分支、隔离候选、比较原目标后更新。 | cargo test --manifest-path src-tauri/Cargo.toml --no-default-features workflow_git；快进、目标前进、脏现场和冲突保护通过。 | 测试仓库之外不创建或清理 worktree；失败不自动回退用户代码。 |
| P6 | 新增 workflow/codex.rs，CLI 内部运行器入口 | 独立 Codex thread/turn 所有权，启动操作去重，状态收件和自动跟进；服务退出保留施工。 | cargo test --manifest-path src-tauri/Cargo.toml --no-default-features workflow_codex；协议夹具与结果不明恢复通过，再用临时仓库运行真实 Codex 交付。 | 记录 thread/turn 与用量；写锁不支持时明确降级，不能争抢真实会话。 |
| P7 | workflow/store.rs/service.rs，workflow_contract.rs | 一致性备份、恢复检查、旧记录只读预览与选择导入、跨仓库里程碑版本集合。 | cargo test --manifest-path src-tauri/Cargo.toml --no-default-features --test workflow_contract；备份可打开、重复迁移不重复、部分整合阻塞通过。 | 不迁移用户真实任务、不覆盖真实库；备份回退只作用测试目录。 |
| P8 | lib.rs、ui/index.html、capabilities 和 scripts 相关测试 | 提供总控查询、任务与交付确认入口，窗口收起和显式退出分离；遵循通用插件调用。 | npm run build:ui；npm run test:tauri:smoke；真实 Tauri 无 pageerror，工作流状态与操作能返回明确结果。 | 保留截图与 smoke 输出；不以 mock 页面冒充验收。 |
| P9 | README.md、skill/SKILL.md、AGENTS.md；任务实现/评审/验收记录 | 文档、指南、发布产物与新命令同步，汇总实现证据与限制。 | cargo test --manifest-path src-tauri/Cargo.toml --test cli_taxonomy --test cli_contract；cargo check --manifest-path src-tauri/Cargo.toml --no-default-features --bin agentwatcher-cli；awf record validate --repo . --work-item 多会话任务服务与-Codex-调度。 | 输出真实结果；未覆盖的 macOS 实机或人工项明确标出。 |

## 依赖与写入归属

P1 → P2 → P3 → P4。P5、P6 在服务合同稳定后接入，P7 依赖正式状态与外部操作账本，P8 使用相同服务接口，P9 在代码和证据固定后完成。当前会话独占维护共用文件，避免多个实现者同时改 lib.rs、cli.rs 和 ui/index.html。

## 风险与处理

| 风险 | 处理 |
| --- | --- |
| GUI 退出清理误杀施工 | 运行器独立于 GUI 和服务；不修改已有建议进程的回收合同。 |
| 手工 Codex 会话写锁 | 返回实际控制能力；无控制通道可主动收件，不能谎报自动续跑。 |
| 外部调用已成功但回执丢失 | 操作先入库，恢复核对 thread/turn/Git；结果不明冻结，不盲目重放。 |
| 路线改版与发送竞争 | 执行前核对节点版本与控制状态，保存停止回执和受影响范围。 |
| 旧协议验证口径与新库不同 | 新行为由隔离运行时测试证明，旧 awf 只验证本任务文档结构。 |


## 第二轮：CLI 主闭环

上一轮真实桌面测试证明窗口能显示状态，但用户确认按钮把协议逻辑复制到了 UI。按用户批准的方向，本轮把 CLI/服务作为唯一控制面，Tauri 工作流窗降为只读诊断。

| 步骤 | 输入与修改 | 验证 |
| --- | --- | --- |
| C1 | 新增 `workflow watch --after` 事件游标与可选等待 | CLI 能从游标读取增量，服务重启后不丢事件 |
| C2 | 新增 `workflow next` 动作收件箱 | handoff、暂停通知、人工阻塞和授权按紧急程度返回 |
| C3 | AI 技能、README、指南和人工表改为 CLI 操作 | 不要求人点击业务按钮；Tauri 只读诊断 |
| C4 | 契约测试和真实 Tauri 只读流程 | `watch/next` 回放、UI 无业务变更按钮、真实壳通过 |

这轮不新增业务状态转移；批准、暂停、退回、恢复和下游授权继续由已有 `workflow apply` 命令执行。
