---
schema_version: "1"
protocol: "1.3.0"
id: "wi_01M6620690000FWYDRRAZPYPYS"
short_id: "4r5pxxz0"
title: "ZCode app-server 发送通道"
status: "active"
kind: "feature"
created_at: "2026-10-30T09:30:00.000000000+00:00"
home_repository: "https://github.com/pb763396199/AgentWatcher.git"
branch_or_pr: "feature/ZCode-appserver-发送通道"
---
# ZCode app-server 发送通道

## 原始请求

用户 2026-10-30 拍板："开『app-server 通道』任务，把发送（顺带生成）切到桌面自己的协议上，这是打通桌面会话的唯一正路。"起因：AI 建议任务实测发现 zcode.cjs --resume 对桌面创建的会话（全部 GLM-5.3）报 "Select a model before sending a message"，CLI 无 --model 参数，发送通道对主要目标会话不可用（调试记录见任务 38rcwjdf 的 debug.md）。

## 目标

AgentWatcher 经 ZCode 桌面自己的 app-server 协议向任意 ZCode 会话（重点是桌面创建的 GLM-5.3 会话）发送消息并触发模型一轮，替换被上游限制挡死的 CLI resume 通道。

## 范围

做：

- Rust 侧 app-server 客户端：spawn zcode.cjs app-server --stdio，换行分隔 JSON 的 JSON-RPC（{id,method,params}），处理服务器反向请求。
- 发送链路：session/resume → 应答 session/requestRuntimePreferences（两个 scope，schema 已实测）→ session/subscribe → session/send（content + modelSelection，模型从会话库 model_usage 读该会话自己的模型回显）。
- send_zcode_message 切换到该通道；CLI resume 保留为降级。
- 建议生成的 scratch 会话评估改走同一连接（省每次冷起）。

不做：

- 不实现订阅事件的完整消费（turn 事件流只做发送确认用）。
- 不动其他 provider。
- 不做 ZCode 点击跳转。

## 强约束

- 凭据零接触：模型与账号全部由会话自身与 ZCode 配置决定，AgentWatcher 只回显 model_usage 里该会话的模型三元组。
- 子进程 CREATE_NO_WINDOW、有超时、失败分类报错。
- 现有 146 测试不回退；zcode-session 流程测试通过。

## 验收条件

- AC-001: 协议调查记录落盘：传输层、方法表、resume/subscribe/send 与反向请求的精确 schema 与实测证据。
  Verify: manual:research 记录评审通过
- AC-002: app-server 客户端帧格式与反向请求应答有单元测试（含 runtime preferences 两个 scope 的 schema 与 modelSelection 回显）。
  Verify: case:zcode_appserver_client_contract
- AC-003: 对桌面创建的 GLM-5.3 会话真实发送一条消息，模型跑完一轮、回复落库、AgentWatcher 卡片状态流转。
  Verify: manual:对真实桌面会话完成一次往返
- AC-004: send_zcode_message 默认走 app-server 通道，通道不可用时降级 CLI 并在错误信息里注明所用通道。
  Verify: case:zcode_appserver_channel_fallback
- AC-005: 全量回归绿（cargo test、zcode 流程测试、build:ui）。
  Verify: case:cargo_test_full_suite
