---
schema_version: 1
protocol: 1.3.0
artifact: manual-test
artifact_id: ar_01M4WQWTDA0007KCZPE2V2AK3W
work_item_id: wi_5SYZXNKVTFYTV99EVBBBMJK0S6
created_at: 2026-10-14T16:42:00Z
producer: aes-validate
result: passed
supersedes: null
dependencies:
  work_item_contract_digest: sha256:2bf0546a812236ff05d377823c4cea07a5a43bdabe190a850ba43a1afb2433e4
  artifacts: []
---

# 人工核对：常驻子服务 kill-on-close 作业

零条目。应用对用户可见的行为没有变化（泄漏只体现在任务管理器里的孤儿进程，机器已经用两条退出路径的实验直接验证过清零）；界面、扫描、卡片全部照旧，两轮冒烟流程覆盖。验收记录 evidence/validation.md 已含全部机器证据。
