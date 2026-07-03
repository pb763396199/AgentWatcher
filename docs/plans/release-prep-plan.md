---
created: 2026-06-01
updated: 2026-06-01
status: active
topic: release-prep-plan
---
# AgentWatcher 发布前准备计划

## 目标

- [ ] 收敛 v0.1.x 发布前必须完成的功能、验证和文档工作。
- [ ] 把发布阻断项和待用户决策项分开，避免临近发布时混在一起。
- [ ] 保持执行清单短、明确、可逐项验收。

## 非目标

- [ ] 不在本计划中确定 prompt 非剪贴板通道的最终方案。
- [ ] 不重构 AgentWatcher 主架构。
- [ ] 不引入大型后台服务、账号体系或远程同步。
- [ ] 不把 docs 是否进入 git 作为既定结论。

## 执行顺序

1. [ ] 确认当前 release scope：只保留发布必须项，延期非必要功能。
2. [ ] 清理发布阻断项：逐项修复、验证、记录结论。
3. [x] 将 prompt 非剪贴板通道移入 Future，不作为本次发布阻断。
4. [x] 更新 README / CHANGELOG / bridge README / TODO 等用户可见文档。
5. [ ] 跑 i18n、theme、DPI、窗口行为和打包验证矩阵。
6. [ ] 做 code review & simplify，删除临时调试逻辑和重复分支。
7. [ ] 生成 release artifact，完成安装/解压后的 smoke test。
8. [ ] 由用户决定 docs 是否进入 git，以及是否发布当前版本。

## 发布阻断项

- [ ] 发布包必须能在干净目录启动，不依赖源码目录。
- [ ] Bridge extension 自动安装/更新失败时，UI 必须给出可操作错误。
- [ ] Session 跳转失败不能静默失败，至少要提示 workspace/session 目标不可达。
- [ ] Session Preview 不能把正文写入 localStorage 或项目目录临时文件。
- [ ] 打包产物内必须包含所需 bridge 文件和 VSIX。
- [ ] release artifact manifest 的 VS Code 连接组件身份和 VSIX 文件必须稳定一致。
- [ ] 启动扫描失败、权限不足、路径不存在时不能导致主界面崩溃。
- [ ] 长时间运行不能出现明显内存增长、窗口卡死或刷新风暴。

## Future：prompt 非剪贴板通道设计待评审

本项延期到 Post-v0.1.x，不作为本次发布阻断；本次只记录可行方向，不落实现。核心约束：不能默认依赖剪贴板；如果需要临时文件，禁止写到项目目录，只能写 AgentWatcher 自身运行时临时目录或系统全局临时目录，例如 `Windows %TEMP%\AgentWatcher\...`。

| 方向 | 安全性 | 复杂度 | 目标窗口绑定 | 清理策略 | 评审问题 |
|---|---|---|---|---|---|
| temp file + token | 中。文件路径和 token 都要短期有效，文件内容不能落项目目录。 | 中 | 需要把 token 与目标 VS Code window/session 绑定。 | TTL 过期删除，成功读取后删除，启动时清理陈旧文件。 | token 如何防复用？失败后是否保留诊断信息？ |
| localhost bridge | 较高。可限制 `127.0.0.1`，但要防任意本机进程调用。 | 高 | bridge 可维护当前活动窗口或显式 window id。 | 请求结束即释放，进程退出自动失效。 | 端口发现、鉴权和防重放怎么做？是否值得增加常驻 listener？ |
| VS Code command arg | 中。参数经过 VS Code command route，必须 whitelist。 | 低到中 | 由 VS Code extension 执行时天然接近目标窗口，但仍要确认 window。 | 不落盘，执行后丢弃参数。 | 参数长度限制是多少？特殊字符和多行 prompt 怎么传？ |
| named pipe | 较高。可限制本机 IPC，但 Windows 权限细节要验证。 | 高 | pipe name 可带 session/window token。 | 连接关闭即释放，超时销毁 pipe。 | Tauri/Rust 与 VS Code extension 两端实现成本是否过高？ |

## 文档更新

- [x] README：确认当前版本、安装方式、release artifact 结构、Bridge 行为、Known Issues、Post-v0.1.x roadmap、发布验证清单。
- [x] CHANGELOG：补齐用户可感知变化、修复项和已知限制。
- [x] VS Code 连接组件 README：确认安装、更新、重装、失败排查步骤。
- [x] TODO：补 Future / Post-v0.1.x 和发布验证清单。
- [ ] DESIGN：只更新仍然准确的当前状态，不做大段重写。
- [ ] 是否把 docs 纳入 git：待用户决定。

## i18n & theme 验证

- [ ] 中文界面：标题、筛选、设置、错误提示、空状态都不溢出。
- [ ] 英文界面：长 workspace 名、长状态文案、按钮文本不挤压。
- [ ] Dark theme：waiting/running/idle、preview、设置面板对比度可读。
- [ ] Light theme：边框、阴影、状态色和透明窗口背景可辨认。
- [ ] 切换语言和主题后，设置持久化正常，刷新/重启后能恢复。
- [ ] 切换语言和主题后，`html.lang`、Session Preview、Handoff Panel 与主窗口保持同步。

## Known Issues / 发布不阻断项

- [ ] Prompt 非剪贴板通道：延期到 Post-v0.1.x，本次不实现、不阻断发布。
- [ ] 历史 VS Code 连接组件测试扩展可能残留在开发机上；这些都是临时 ID/已废弃路径，当前发布只以稳定组件身份为准。
- [ ] docs 是否进入 git：待用户决定，不在当前准备中默认改变仓库策略。

## code review & simplify

- [ ] 检查发布路径中是否还有 mock、调试日志、硬编码本机路径。
- [ ] 删除重复的状态判断和已废弃的 fallback 分支。
- [ ] 检查 Tauri command error 是否都能返回前端可展示信息。
- [ ] 检查 Bridge command route 是否只开放 whitelist command。
- [ ] 检查临时文件、缓存、preview 正文是否遵守隐私边界。
- [ ] 优先做小清理，不为发布临时引入新抽象。

## 验证矩阵

| 场景 | 验证项 | 通过标准 |
|---|---|---|
| Dev UI | `npm run dev:ui` | 页面可打开，基础交互正常。 |
| Tauri dev | `npm run dev` | 桌面窗口、resize、设置、preview 正常。 |
| Release package | `npm run package:exe` | 生成 `artifacts/AgentWatcher/` 可分发目录和 `artifacts/AgentWatcher-v<version>-windows-x64.zip`。 |
| Clean launch | 从发布目录启动 | 不依赖源码目录，Bridge 状态可检查。 |
| Bridge install | 未安装/旧版本 Bridge | 自动安装或更新成功；失败时提示可操作。 |
| VS Code connector reinstall | 手动卸载/重装 VS Code 连接组件 | `code --install-extension <连接组件 VSIX> --force` 后 URI 路由可用，`code --list-extensions` 只剩稳定组件。 |
| Session scan | Copilot / Claude sessions | waiting/running/idle 识别不回退旧状态。 |
| Jump | 点击 session 卡片 | 能打开目标 workspace/session；失败有提示。 |
| Preview | hover 展开 | 内容可读、可滚动、不写 localStorage。 |
| Child windows | 切换中英和 dark/light | 主窗口、Session Preview、Handoff Panel 同步。 |
| DPI | 100% / 125% / 150% | UI 不重叠，窗口 resize 手感正常。 |
| Long run | 8 小时观察 | 无明显内存增长、卡死、刷新风暴。 |

## 待用户决策

- [ ] 当前 release scope：哪些 TODO 必须进本次发布，哪些延期。
- [ ] docs 是否进入 git，以及哪些 docs 属于发布包/仓库资料。
- [x] prompt 非剪贴板通道是否进入本版本：不进入本版本，移入 Future。
- [ ] 发布格式：只发 zip，还是同时发布 installer 产物。
- [ ] 版本号和 release note 口径。
