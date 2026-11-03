---
schema_version: "1"
protocol: "1.3.0"
artifact: "debug"
artifact_id: "ar_01M65YR0Q80004EJ63GJHC4PYN"
work_item_id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
created_at: "2026-10-30T09:05:00Z"
producer: "aes-debug"
result: "mitigated"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:2b1c17e9e8d949086387fef35bc752d834b0026f368ec9f17c071b583ecf4d0a
  artifacts: []
---
# 调试记录：真机实测撞上的两个上游限制与一处超时缺陷

用户指定 session_45ec10e84429c65ad62778409cf81046 做真机实测（2026-10-30 下午）。

## 现象

对该会话 headless 发送直接报错；换 CLI 会话做链路实测，发送一轮在 120 秒被杀。

## 怀疑过什么、凭什么排除

- 先怀疑会话被桌面占用或 --cwd 影响：加 --surface desktop、换临时目录 --cwd 重试，同一报错，排除。
- 再对比试点会话（CLI 创建）同参数秒通，锁定差异在会话的模型绑定，不是通道本身。

## 真正原因（三条）

1. 桌面会话 headless 发送被上游挡死（未解决，等上游）。目标会话（桌面创建，模型 GLM-5.3）用 zcode.cjs --resume 发送报 "Select a model before sending a message"；CLI 创建的会话（claude-fable-5-1）同参数秒通。证据：~/.zcode/cli/db 的 model_usage 表里桌面会话全部是 GLM-5.3、CLI 会话是 claude-fable-5-1；zcode.cjs 反查到报错点 CliDefaultSelection 为空即抛，而 CLI 没有 --model 参数。要给桌面会话发送，得走桌面自己的 app-server 协议，另开任务。
2. headless 造不出等待状态（测试用夹具绕过，不是产品缺陷）。headless 里 AskUserQuestion 工具立即 error，到不了 pending，而 AgentWatcher 的 waiting 检测认 pending。实测用 db 夹具把自建 scratch 会话的提问 part 置 pending 并补时间戳（会话摘要缓存按 session.time_updated 键控，不动时间戳 GUI 不刷新）。
3. 发送超时 120 秒太短（已修，提交 899b948）。76k 上下文的会话一轮回复跑不进两分钟，子进程被杀且不落库；提到 300 秒。

## 怎么修的、加了什么回归测试

超时修复与观测日志见 899b948；信封解析失败打 stdout 头两百字。上游两条限制无法在本仓库修，已写进实现记录的风险节，app-server 通道留待新任务。

## 链路实测结果（含一次完整走通）

- waiting 检测、无头生成（ZCode 自身引擎）、建议事件、卡片建议行：一轮完整走通，建议文本「继续完整回归」正是选项之一，截图在 .tmp/selftest/m1-suggestion-row.png。
- 发送通道：命令响应正常，超时与防陈旧两类错误按设计返回；超时修复后未再拿到完整落库的一轮——应用重启后生成不再稳定出结果，且 dev 日志管道断掉失去观测（stdout 文件停在 16:20），疑与账号侧限流或多轮测试残留有关，未定位。
- 「换一个」上限、右键菜单、自动作答真机流程：未走完，留在人工清单。
