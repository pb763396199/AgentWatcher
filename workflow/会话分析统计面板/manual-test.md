---
schema_version: "1"
protocol: "1.3.0"
artifact: "manual-test"
artifact_id: "ar_3VQFQZVZBZ1KS97P64WE8RXKXC"
work_item_id: "wi_3KXFZQMQ9AH2XQ8PGQBW299E2P"
created_at: "2026-10-24T07:09:12Z"
producer: "aes-validate"
supersedes: null
dependencies:
  work_item_contract_digest: "sha256:3d9399399dc2ba070fc3f50d89a89b3cbd3655a22f1980b16c15899a6bc02fbc"
  artifacts: []
result: "passed"
---
# 人工核对清单

说明：本任务经用户本人 12 轮交互实测（2026-10-14 至 10-24），以下条目按
用户实测与 CDP 真实运行时走查双重证据closure；未覆盖项明确列后。

- [x] 右键会话卡出现「会话分析」并打开分析窗口
- [x] 悬浮预览用量区块的「会话分析」按钮打开同一会话的分析（入口统一）
- [x] 分析窗口 × 按钮与 Esc 可关闭（真实鼠标九点位测试）
- [x] 标题栏可拖动窗口，且不吞按钮点击
- [x] 缩小窗口：整体出现竖向滚动条，无任何子面板被压缩或裁切（四档尺寸扫描）
- [x] 粒度自动档按会话跨度选桶宽，y 轴/x 轴刻度可读
- [x] 泳道点击展开合并明细，收起无残留
- [x] 多智能体会话显示子代理泳道与开销排行（27 条合计 77.3M）
- [x] 悬浮预览对 Copilot CLI 会话显示降级说明
- [ ] 125%/150% DPI 下的布局（未测，沿用仓库未测口径）
- [ ] 8 小时长跑内存观察（未测）
