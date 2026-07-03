# AgentWatcher v0.1.1 发布前准备审计

日期：2026-06-01
范围：发布前准备任务审计；不改业务代码、不运行验证、不提交。

## 结论先行

- 完成状态：partial。发布文档、打包脚本、设置同步和 release zip 已完成；仍有人工发布验证项未关闭。
- 验证状态：partial/pass。已记录并由本轮上下文提供的命令验证通过；本审计未重新运行验证。
- 需求覆盖：partial。文档、多中英/light dark 同步、过时说明、打包清理已有覆盖；完整 code review/simplify 和人工 smoke 仍未完全闭环。
- 计划兑现：justified_deviation。非剪贴板 prompt 通道延期到 Post-v0.1.x，docs 是否入 git 保持用户决策。
- 主要后续动作：完成发布目录 smoke、Bridge 安装/重装和 session 跳转人工验证；确认 docs 是否入 git；发布前确认是否清理或忽略旧 v0.1.0 历史产物。

## 任务结果裁定（Task Outcome Verdict）

本轮发版准备已经达到“可进入最终人工发布验证”的状态，但不应标为完全发布就绪。原因是 zip 已生成且哈希已记录，自动化检查已通过；不过 README/TODO/计划中的发布验证清单仍有多项未勾选，尤其是从发布目录启动、Bridge 自动安装/更新、session 跳转、DPI 和长时间运行。

## 需求台账（Requirement Ledger）

| ID | 需求 | 裁定 |
|---|---|---|
| R1 | 补齐发布文档、CHANGELOG、TODO、Bridge README | satisfied |
| R2 | 多语言和 dark/light 同步到主窗口、Session Preview、Handoff Panel | partial；代码和 changelog 记录已覆盖，人工 UI 验证未闭环 |
| R3 | 清理过时注释/说明，明确稳定 Bridge ID，历史测试 ID 仅为临时 ID/已废弃路径 | satisfied |
| R4 | code review/simplify | partial；有发布路径清理和脚本收敛，但计划中的完整 review checklist 未全部关闭 |
| R5 | 生成并核对 v0.1.1 发布 zip | satisfied by recorded validation |
| R6 | 不改业务代码、不运行验证、不提交，仅写精简审计 | satisfied for this audit |

## 计划基线（Plan Baseline）

基线来自发布准备计划、TODO 和用户本轮上下文：先收敛 release scope，再更新文档，修正同步/打包问题，执行验证矩阵，最后由用户决定 docs 入 git 和是否发布。

## 交付物清单（Outcome Inventory）

- 文档：README、CHANGELOG、TODO、Bridge README、release-prep-plan、handoff insight 已更新或已有记录。
- 代码/脚本：ui settings sync、scripts/package-exe.mjs、VS Code 连接组件入口已纳入本轮变更范围。
- 产物：artifacts/AgentWatcher-v0.1.1-windows-x64.zip，SHA256 `970E3EF47191558ED0138679F975E8C11B211D825794BC14486B3D83D175200D`。
- 审计文档：本文件。

## 需求到结果对账矩阵（Requirement-To-Outcome Matrix）

| 需求 | 预期结果 | 实际结果 | 覆盖状态 | 证据 | 缺口 |
|---|---|---|---|---|---|
| R1 文档 | 用户能看到安装、风险、验证、发布说明 | README/CHANGELOG/TODO/Bridge README 已覆盖 | satisfied | README Known Issues、CHANGELOG v0.1.1、Bridge README | none |
| R2 设置同步 | 中英/theme/layout/always-on-top 同步到子窗口 | CHANGELOG 记录已完成，UI 文件包含 light theme 和 preview/handoff 窗口逻辑 | partial | CHANGELOG v0.1.1、ui/index.html | 仍需人工 UI smoke |
| R3 过时说明 | 历史测试 ID 不再混入当前发布路径 | 文档明确当前使用稳定连接组件身份，旧测试 ID 仅为历史测试 | satisfied | README 连接组件章节、连接组件 README | none |
| R4 打包 | 生成当前版本 zip，Bridge 目录干净 | package 脚本清理 Bridge 输出目录并生成 zip；产物目录含 exe、Bridge、VSIX | satisfied | scripts/package-exe.mjs、artifacts/AgentWatcher/ | zip 内容检查来自上下文，审计未重跑 |
| R5 验证 | 自动化检查可追溯 | 已记录 node check、build:ui、cargo check、diff check、package:exe、zip 内容检查 | partial/pass | CHANGELOG 验证行、用户上下文 | 本审计未重新执行 |
| R6 发布决策 | 区分阻断项与用户决策项 | docs 入 git、非剪贴板通道都保留为决策/延期项 | satisfied | README Known Issues、TODO Future、release-prep-plan | none |

## 偏差登记表（Deviation Register）

| ID | 偏差类型 | 原计划 / 原需求 | 实际变化 | 理由 | 代价 | 状态 |
|---|---|---|---|---|---|---|
| D1 | scope_drop | prompt 非剪贴板通道可能进入发布准备 | 延期到 Post-v0.1.x | 风险和设计空间较大，不适合作为 v0.1.x 阻断项 | 当前仍依赖 clipboard | evidence_backed |
| D2 | validation_gap | 完整发布验证矩阵应全部关闭 | 自动化验证完成，人工 smoke/DPI/长跑未全关 | 发布前准备先完成代码和产物，人工验证留到最终 gate | 不能宣称 fully release-ready | evidence_backed |
| D3 | artifact_residue | artifacts 根应尽量只看当前发布 | 仍保留旧 v0.1.0 历史产物 | 历史产物被 gitignore，不影响当前 zip | 本地目录易让人工排障混淆 | evidence_backed |
| D4 | decision_deferred | docs 是否入 git | 保持待用户决定 | 仓库策略不能由本轮默认改变 | 发布前还需明确 | evidence_backed |

## 修正后的完整计划定稿版（Finalized Revised Plan）

- 定稿状态：conditional。
- 适合复用程度：项目局部。
- 修正后目标：把 v0.1.1 发布准备收敛到“当前 zip 可交付 + 人工 gate 清晰”，不把后续设计债混入本次发布。
- 适用条件：AgentWatcher v0.1.x、Windows zip 发布、Tauri 2 主壳、VS Code 连接组件稳定身份。
- 不适用条件：需要 installer 发布、非剪贴板 prompt 通道、跨平台发布或远程同步。
- 修正后步骤：确认版本与 release notes；确认 zip 和哈希；完成 Bridge 安装/重装与 session 跳转 smoke；确认 i18n/theme 子窗口同步；确认 docs 入 git策略；发布。
- 与原计划差异：非剪贴板 prompt 通道移入 Future；docs 入 git保留为用户决策；长跑/DPI仍作为 release gate 而非本轮审计结论。
- 验证门槛：当前自动化验证命令不回退，人工 smoke checklist 至少关闭发布目录启动、Bridge、session jump、子窗口同步、包内容。
- 不采用方案：本次不引入 prompt 临时文件、不新增大文档体系、不把旧测试 ID 作为当前发布路径。

## 理由与决策记录（Rationale & Decision Log）

- 以 zip 为唯一当前发布物，符合 README 和 package 脚本输出。
- VS Code 连接组件当前只接受稳定身份；旧测试 ID 都是历史临时 ID/已废弃路径。
- 发布验证必须核对 release artifact manifest：VS Code 连接组件身份和 VSIX 文件保持稳定一致，`code --list-extensions` 应只剩稳定组件。
- prompt 非剪贴板通道延期是合理取舍；它影响安全、窗口绑定和清理策略，不宜在发布前临时落地。
- docs 是否入 git属于仓库策略，保留人工确认是正确边界。

## 证据包（Evidence Pack）

- README：当前版本、release artifact、Bridge 行为、Known Issues、发布验证清单。
- CHANGELOG：v0.1.1 改动、zip 文件名、SHA256、验证命令。
- TODO：当前优先级、发布验证清单、Future / Post-v0.1.x。
- docs/plans/release-prep-plan.md：目标、非目标、阻断项、验证矩阵、待用户决策。
- scripts/package-exe.mjs：复制 Bridge 文件、生成 VSIX、生成 `AgentWatcher-v<version>-windows-x64.zip`。
- artifacts/AgentWatcher/：当前可分发目录包含 `AgentWatcher.exe` 和 VS Code 连接组件。

## 执行时间线（Execution Timeline）

- 发布准备阶段：文档、设置同步、脚本、Bridge README 和 docs 计划/insight 更新。
- 验证阶段：按用户上下文，已执行 node check、UI build、cargo check、diff check、package、artifact/zip contents checks。
- 审计阶段：本轮只读核对文件和产物目录，并写入本审计记录。

## Agent 表现与过程轨迹

- 本审计遵守只读业务代码、不运行验证、不提交的边界。
- 审计依据主要来自工作区文件、产物目录和用户提供的验证事实。
- 未独立读取 git status/diff；当前工具集中没有运行 git 命令的必要动作，且用户未要求重新验证。

## 用户纠正记录

empty。本轮用户明确给出审计目标、范围、已验证事实和已知风险，未出现纠正。

## 验证质量（Validation Quality）

- 自动化验证覆盖较好：JS 语法、Bridge extension 语法、前端构建、Rust/Tauri check、diff whitespace、打包脚本和 zip 内容。
- 人工发布验证仍不足：发布目录启动、Bridge 自动安装/更新、session 跳转、DPI、长时间运行仍需最终确认。
- 本审计验证状态：not-run by design；仅做证据核对。

## 系统改进建议

- 发布前把 README/TODO 的发布验证清单作为单一 gate 逐项勾选，避免 changelog 中的“验证已过”被误读为人工 smoke 全部完成。
- 若保留旧 v0.1.0 产物，发布说明里只引用 v0.1.1 zip，避免人工上传错包。
- 后续可以把 zip SHA256 生成和内容清单输出并入 package 脚本日志，减少手工记录误差。

## Knowledge Curator Handoff

- 可回流经验：发布准备要区分“自动化验证通过”和“人工发布 gate 完成”。
- 可回流经验：prompt handoff 的非剪贴板通道涉及安全与清理策略，不应在临近发布时作为小修补混入。
- 不写长期 memory：本审计按当前模式只输出候选，不写 memory。

## 后续动作（Follow-up Actions）

1. 从 `artifacts/AgentWatcher/AgentWatcher.exe` 启动，确认不依赖源码目录。
2. 在未安装/旧 Bridge 环境验证自动安装或更新，并验证失败提示。
3. 手动重装 VS Code 连接组件后验证 session 跳转，并确认 `code --list-extensions` 不再列出旧测试 ID。
4. 切换中英和 dark/light，确认主窗口、Session Preview、Handoff Panel 同步。
5. 确认 docs 是否入 git，以及发布时是否只上传 v0.1.1 zip。

## 跳过记录 / 标记（Skip / Marker）

- status：not-skipped。
- marker：审计已执行；业务代码未修改；验证未重跑；git 未提交。
