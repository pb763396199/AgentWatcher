---
schema_version: 1
protocol: 1.3.0
artifact: manual-test
artifact_id: ar_01M5YDKRC40007537FQFK9P56N
work_item_id: wi_25PNYKYZZZEZF9VBX61N79GPEG
created_at: 2026-10-27T10:30:00Z
producer: aes-validate
result: passed
supersedes: null
dependencies:
  work_item_contract_digest: sha256:e03deeaffb83f1d3d5bcc288625d29642afd773ef027a39c45750a1581c454813
  artifacts: []
---
# 人工核对：打包会话用于跨电脑交接

代码在分支 `feature/会话卡片右键打包整个会话跨电脑交接`（提交 `c948a51`）。以下为人在真实窗口里验过的项；机器已盖住的不重复列（见 evidence/validation.md）。

- [x] 右键真实会话卡片，点「打包会话用于跨电脑交接」：toast「已打包并复制到剪贴板，可直接粘贴发送」。（用户实测多轮，含 git 锚点与 README 版本）
- [x] 打包后在 IM 里 Ctrl+V：粘出的是 zip 文件本体。（用户把包发给另一个 agent 分析，即端到端实证）
- [x] 解开 zip：HANDOFF.md 是定位块（含「代码锚点」行）、README.md 是格式说明、raw/ 是原始文件全量、manifest.json 的 SHA256 与文件对得上。（用户 + 外部 agent 双重核对，外部结论"作为对话存档接近无损"）
- [x] 打包过程不再闪终端窗口。修复（`5cb7b19` 补 CREATE_NO_WINDOW）后用户继续打包使用，未再复报；该项为视觉现象，机器不可验，以未复报计。
- [x] 打包 Codex 服务器侧线程（`aw-` 开头卡片）得到明确报错「无本地转录可打包」。（用户机器实测发现，行为符合设计）
- [ ] 英文界面与浅色主题下的菜单样式未逐项目视：文案键已在中英字典各就位（逐键核对），菜单复用现有样式类，风险低。发布后使用中观察。

用户于 2026-10-27 下达「发布新版本」指令，视为对本清单的放行；唯一未目视项（英文/浅色样式）已如实标注。
