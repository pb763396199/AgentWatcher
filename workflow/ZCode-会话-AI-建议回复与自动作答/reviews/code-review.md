---
schema_version: "1"
protocol: "1.3.0"
artifact: "review"
artifact_id: "ar_5HZQXQQFZHK6WYKJPCMAJ6XVZ2"
work_item_id: "wi_6NWY7FKVENCGEWWHB1CYAZ61G0"
created_at: "2026-10-30T04:30:09Z"
producer: "aes-review"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:2b1c17e9e8d949086387fef35bc752d834b0026f368ec9f17c071b583ecf4d0a
  artifacts:
    - artifact_id: "ar_4QY67ZCXZ7YPS1Z73K7P90PVTR"
      digest: sha256:f12d8fecf62a32153d4ac169630c9f2729075bb0be0ba186d07b8ca3b96711c4
      locator: "implementation.md"
  subject:
    kind: "change_set"
    digest: null
    repository: "https://github.com/pb763396199/AgentWatcher.git"
    base_revision: "d0e0d7ab880c95fb8bdd3d525813057fe2e74c32"
    revision: "d0e0d7ab880c95fb8bdd3d525813057fe2e74c32"
    tree: "0b9851b68f33b1ef0a92d7ade6e09528c8ab0fcd"
    content_digest: "sha256:66e68d9e461b950bf4d3ba1765215aa70d2cd2b2c5fd2db4c7ddeac2e7f57d91"
    branch_or_pr: "feature/ZCode-会话-AI-建议回复与自动作答"
    workflow_excluded: true
verdict: "changes_requested"
review_type: "code"
reviewers: []
---
# 代码评审（第一版）

审查范围：feature/ZCode-会话-AI-建议回复与自动作答 全部施工提交（d4b38a6..3c2d51d 加 AGENTS.md 提交），只读审查。

## 阻断问题

1. **生成线程持锁调用全自动判定，同线程二次加锁会死锁，进而冻住扫描。**
   - 位置：src-tauri/src/lib.rs 的 spawn_suggestion_generation（约 15350-15366 行）。
   - 后果：`let mut runtime = suggestion_runtime().lock().ok()` 的锁守卫存活到函数结尾，而第 15365 行的 `maybe_auto_answer(&app, &session_id, &suggestion)` 内部再次 `suggestion_runtime().lock()`。std::sync::Mutex 不可重入，生成线程当场卡死并一直持有锁；下一次扫描的 maybe_run_suggestion_pipeline 也要拿同一把锁，scan_sessions 从此不返回，主界面停止刷新。
   - 依据：Rust 标准库 Mutex 语义（同线程重复 lock 未定义/死锁），以及本文件其余调用点（send_zcode_message、run_hotkey_action）都把锁收在作用域块里，唯独这里漏了。
   - 最小改法：存完 latest/current 之后、emit 与 maybe_auto_answer 之前显式 `drop(runtime);`。

## 非阻断问题

2. 会话从 waiting 离开再回来时，前端 suggestionBySession 里留着的旧建议会先渲染出来（点击发送会被后端指纹校验拦下，安全但体验突兀）。位置：ui/index.html 的 decorateCardWithSuggestion。建议后续在收到非 waiting 状态的渲染时清对应条目，本轮不改。
3. run_child_with_timeout 在进程退出后才读 stderr，子进程输出超过管道缓冲（约 64KB）会写阻塞。zcode.cjs --json 的信封远小于该值，当前场景安全；若日后复用要改并发读。
4. 热键注册失败（如被占用）只 toast 一次，无重试入口；重新保存设置可再触发注册，可接受。
5. 模型 API 密钥存 localStorage（设计已声明的取舍），评审知悉，不构成阻断。

## 审查过但没发现问题的范围

- 指纹防陈旧、预算、熔断、选项命中的判定路径（有单元测试覆盖）。
- 发送子进程的 CREATE_NO_WINDOW、超时、错误分类。
- 隐私边界：建议正文只走事件，localStorage 只存设置项。
- i18n 中英文与深浅色样式成对出现。
- CLI 三入口与 scan_sessions_blocking 未被触碰（零副作用保持）。

## 结论

changes_requested：问题 1 必须修复后重新评审。
