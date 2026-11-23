---
schema_version: "1"
protocol: "1.3.0"
artifact: manual-test
artifact_id: "ar_01M83B66AY000EKXHXNNFXXQFD"
work_item_id: "wi_0ZQ8FFAX6K2B4D8F66T96TFDEM"
created_at: "2026-11-23T04:40:00Z"
producer: "aes-validate"
result: "draft"
dependencies:
  work_item_contract_digest: sha256:377edb0dac11131577aa3523826b0b7accc7a012a8b0fef5fe9226ef0433ed54
  artifacts: []
---
# 人工核对清单

按「只写机器验不了的」的准入规则重整过一次（2026-11-23）：原五条里的「接续导出
落盘」与「429 会话超窗变闲置」两条，机器证据已经盖住——真实 CLI 的 handoff
export 落盘核验与终审评审黑盒验证（validation-v4 的 AC-008 证据），以及真实
session show 对那个 429 停掉的会话返回 idle（状态列滞留兜底按设计工作）——移回
机器证据，不算人工项。剩下的三条都是人眼在界面上才能判定的现象。

应用已经替你开着（任务分支最新代码）。前两条各花 30 秒；第三条不用刻意造，
下次 Antigravity 真的弹选择题问你时顺带看一眼就行——但它会一直开着，等不起就
按下面的说明处理。

- [ ] <!-- AC-003 --> 鼠标悬停 Antigravity 卡片（徽标 AG）。预览窗能看到工作区、标题、状态、你最后的提问和 AI 的回复，跟 Antigravity 桌面端同一会话的内容对得上。
- [ ] <!-- AC-004 --> 切换中文/英文、深色/浅色四种组合：设置面板 Data 组的「Antigravity sessions / Antigravity 会话」开关默认勾选；provider 筛选下拉里有 Antigravity，徽标 AG 四种组合显示正常。
- [ ] <!-- AC-001 --> 下次 Antigravity 里 AI 弹出选择题问你时，回看 AgentWatcher：卡片状态应显示「待回」；你回答之后状态应离开「待回」。

等不起第三条时：把这一条整行删掉，并在下面写一行「用户拍板：等回复判定按结构
推断上线，风险已知（设计记录未决问题 3 已留档）」。删条目是拍板，不是勾选。

用户反馈：
