---
schema_version: 1
protocol: 1.3.0
artifact: review
artifact_id: ar_01M2FDDDCPCX9ZNE75XB7QEQHT
work_item_id: wi_3QQPFJCPXD24P1CCXMXAZ730TQ
created_at: 2026-09-14T07:45:00Z
producer: aes-review
verdict: approved
supersedes: null
dependencies:
  work_item_contract_digest: sha256:efa5a5233be0f6e70b70442ff1fb5cb63dad810669e2356c04643038eb83e195
  artifacts: []
  subject:
    kind: change_set
    digest: sha256:b60025e1431c0ee311f478d7afc97e421507a7622c71084518b4a11adc575a21
    repository: https://github.com/pb763396199/AgentWatcher.git
    base_revision: cd721467e82a7199ca0da299071e2b99f33312b7
    revision: 10649708e7168a397c850b38fccf1b31cecebfbd
    tree: 1f344685d607a9bccaacef077f1e1ec3d07fc9be
    content_digest: sha256:b60025e1431c0ee311f478d7afc97e421507a7622c71084518b4a11adc575a21
    branch_or_pr: dev
    excluded_prefixes: []
    workflow_excluded: true
---

# 评审：ZCode Provider（自评审声明：执行者本人评审，按协议在开头声明）

## 结论

批准。变更把 ZCode 作为只读监控 provider 接入，跳转按用户拍板下线并给出明确提示；
接续上下文归属守卫修复了一个跨 provider 的真实缺陷并有回归固化。隐私边界未破：
预览正文仍走运行时事件、handoff 导出只落系统临时目录约定位置、无 Bridge 合同变更。

## 审查中抓到的问题

| 问题 | 怎么发现的 | 处置 |
| --- | --- | --- |
| 接续面板把上一次导出的 sourceContext 带给新来源会话（串号，opencode 同样受影响） | 用户实测 prompt 中会话 ID 与导出文件不符，插桩复现 | 已修：handoffContextMatchesSource 四处守卫 + flow 归属断言 |
| TUI 跳转初版会弹出立即退失败的终端窗口 | 真机验证 node zcode.cjs --resume 报缺 @zcode/tui | 已按用户拍板整体下线，返回明确提示 |
| 并行恢复警示 toast 会被 opening/opened 立即覆盖 | 代码走查时序 | 随跳转下线一并移除 |
| 状态机若用 session.time_updated 判新鲜度会把空闲会话误判 running | db 实测该字段为亚秒级心跳 | 已用 message/part 内容时间戳，单测锁定 |

## 已核实的事实

- 扫描只读打开 db（READ_ONLY + query_only），过滤 subagent_child；
- `@zcode/tui` 在官方渠道不存在（3.11.2/3.12.1 解包、npm/镜像 404、CDN 无 CLI）；
- 81 项单测、smoke、两个 zcode flow、双主题双语言四组合核验、发布产物验证全部通过。

## 非阻断问题

- CLI 会话不出现在桌面侧边栏属 ZCode 产品行为（feedback#617），非本仓库可控。

## 没覆盖的范围

- 125%/150% DPI、8 小时长跑、干净机器矩阵（TODO 已列）；PATH 独立 CLI 环境的恢复分支未真机复验。
