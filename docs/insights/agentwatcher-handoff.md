---
created: 2026-05-30
updated: 2026-06-01
status: active
topic: agentwatcher-handoff
---
# AgentWatcher Handoff

AgentWatcher handoff 不复制原 session 文件。新 session 只接收 source locator、latest snapshot 和显式 new task：原 session 是参考材料，不是指令来源，new task wins。

VS Code handoff launch target 必须先 canonicalize。目标是目录时，目录同时作为 CLI argument 和 cwd；目标是 `.code-workspace` 文件时，文件作为 CLI argument，父目录作为 cwd。

VS Code bridge 的 command route 必须 whitelist。`/command?command=...` 只能执行固定允许列表中的命令，不能把任意 VS Code command 暴露给 deep link 或 extension command 参数。

## Release Packaging Notes

2026-06-01 发布准备回流：

- 真实发布文档不能低估 clipboard 依赖；当前 handoff prompt 仍是 clipboard bridge，非剪贴板通道属于 Post-v0.1.x 延期项。
- VS Code extension identity 必须永久稳定；历史临时测试 ID 不能以 cache-busting 方式发布。
- App release version 可以在应急资产替换时继续保持 `v0.1.2`，Bridge package version 独立为 `0.1.10`，两者不要混成同一个版本判断。
- Bridge 安装流程必须先清理 legacy IDs 再安装，并在安装后确认只剩稳定 ID、没有历史测试 extension。
- Release validation 必须同时检查 source manifest、packaged manifest、随包 VSIX 文件，以及重复执行 `code --install-extension` 的结果。
- 打包脚本必须清理 release bridge 目录，避免旧 VSIX 残留污染发布包；zip 必须从明确版本路径生成，并记录 SHA256。
- 发布前验证要区分自动检查通过和人工 smoke 完成；未完成的发布目录启动、Bridge 安装/重装、session 跳转等 smoke 必须显式标注。
