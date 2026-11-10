---
schema_version: 1
protocol: 1.3.0
artifact: manual-test
artifact_id: ar_01M71TW7CW0005RGVDFHMPR2DY
work_item_id: wi_01M71Q3R0W0004MNNQHHS0ZSZ1
created_at: 2026-11-10T12:24:00+08:00
producer: aes-validate
result: pending
supersedes: null
dependencies:
  work_item_contract_digest: sha256:08218fcaad2d9ec21a92a9335bb0cea91f075df699314c2971d7d17dfd158939
  artifacts: []
---

# 人工核对清单

两条，都是机器证明不了「图片内容对不对」的部分，大约五分钟（每条包含一次约一分钟的生图等待）。

- [ ] 在终端跑一次真实生图并看图。命令（worktree 里构建的调试版二进制）：
  `F:\AiProject\AgentWatcher\.aes-workflow\worktrees\Codex-生图-CLI-命令\src-tauri\target\debug\agentwatcher-cli.exe imagegen generate "a cozy cabin in snowy mountains at dusk, warm window light" --out %TEMP%\cabin-test.png`
  打开生成的图片，看到雪山黄昏中的小木屋、窗户透暖光，就算过。
  <!-- AC-001 -->
- [ ] 把电脑上的 imagegen 技能目录临时改名，再跑一次生图，确认还能出图、显示用的是内置副本：
  1. `ren "%USERPROFILE%\.codex\skills\.system\imagegen" imagegen-bak`
  2. 用上面同一条命令再跑一次（换个 --out 路径），打开 JSON 结果里 `"promptSource"` 应显示 `embedded`，图片照常生成。
  3. 测完改回来：`ren "%USERPROFILE%\.codex\skills\.system\imagegen-bak" imagegen`
  <!-- AC-002 -->

用户反馈：
