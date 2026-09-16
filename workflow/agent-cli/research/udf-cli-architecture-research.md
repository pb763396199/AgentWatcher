---
schema_version: "1"
protocol: "1.3.0"
artifact: "research"
artifact_id: "ar_4FPTMQVXYVBPRZ28NJ33D9EJ5B"
work_item_id: "wi_1QYBMJJZVNGF7XP88ZEEBHVMRT"
created_at: "2026-09-16T03:13:00Z"
producer: "aes-research"
supersedes: null
dependencies:
  work_item_contract_digest: sha256:32abb9b77345e1c4412d7442f9ae353d1bca154ec66037eeaf403b276872b2ea
  artifacts: []
result: "complete"
topic: "udf-cli-architecture"
---
# 调查：UnrealDevFlow 的 udf CLI 架构

## 看了哪些地方

| 位置 | 看什么 |
| --- | --- |
| `F:\AiProject\UnrealDevFlow\Cargo.toml` | 二进制目标、依赖清单 |
| `src/main.rs` 38-62 行 | 入口：先定输出格式再跑命令、统一错误边界 |
| `src/cli.rs` 13-148 行、762-871 行 | 命令枚举、六个名词组、帮助文本模板 |
| `src/output.rs` 12-154 行 | JSON 信封、进度行捕获、失败信封、防重复输出 |
| `src/commands/` 21 个模块、`src/commands/build_policy.rs` 86-143 行 | 命令编排、nextCommand、gate 退出码 |
| `src/commands/aw_status.rs` 83-85 行、根目录 `AgentWatcher.awmodule.json` | 给 AgentWatcher 用的固定形状输出 |
| `src/commands/skills.rs` 93-140 行 | skill 分发到各 AI 宿主目录 |
| `tests/`（cli_taxonomy、execution_contract、query_semantics 等） | 分类学测试、输出契约测试、真实二进制集成测试 |
| `README.md`、`AGENTS.md` | 使用者定位、发布门禁 |

## 查到的

- 入口：Rust + clap 4.5（derive 宏），二进制名 `udf`，`[[bin]]` 声明在 Cargo.toml 11-17 行。帮助文本全量换成语义化中文，注释写明「使用者是中文开发者和替他们干活的 AI 办手」。
- 命令组织：noun-verb 结构。顶层六个名词组（workspace / task / build / run / package / skill）加一个隐藏命令 `aw-status`。跨组动词词表统一：`check`（现在能不能做）、`plan`（将执行什么）、`status`（已执行的证据）三组语义完全一致，共用同一段帮助常量，`tests/cli_taxonomy.rs` 219-236 行用测试锁死逐字相同。
- 输出契约：全局 `--format json|human`。JSON 模式下一条命令只输出一个信封文档 `{ command, ok, data, error, messages }`（camelCase）。进度和警告文本在 JSON 模式下收进 `messages` 数组，不直接打印，避免污染 stdout。失败也走信封（`emit_failure`），注释原话「callers never have to scrape stderr」。
- 给 agent 指路：`build check` 的输出带 `nextCommand` 字段直接告诉 agent 下一步敲什么。
- 退出码：成功 0，错误统一 1。特判：`build check` 无论结论如何都返回 0（「结论就是答案，不是失败」）；`build gate` 拦截时返回非 0，用于挂 hook。
- 命令注册：集中式。cli.rs 嵌套枚举定义形状，main.rs 159-505 行 match 分发，`command_name()` 把每个叶子映射成稳定字符串作为信封的 `command` 字段。
- 分层：单 bin crate 内分三层——cli.rs（纯定义无 IO）、commands/（编排加输出）、领域模块（不感知 clap）。
- 与 AgentWatcher 的既有集成：`udf aw-status` 隐藏命令输出固定形状 JSON 并刻意绕过信封（aw_status.rs 83-85 行注释「AgentWatcher 读的是这个固定形状，套上信封会打断它」）；根目录 `AgentWatcher.awmodule.json` 是声明式模块清单，每个子命令带 `safety: readOnly/boundedWrite` 标注和超时。
- skill 分发：`udf skill install` 把 SKILL.md 复制到四个 AI 宿主的技能目录。
- 测试：三层——分类学测试（直接引入 cli.rs 验证解析）、输出契约测试（结构序列化逐字段断言 + `Command::cargo_bin` 起真实二进制按信封断言）、环境变量 `UNREALDEVFLOW_CONFIG_DIR` 重定向配置目录做隔离。

## 推断

- 信封加进度捕获解决了「人读输出污染机器解析」，AgentWatcher 的 CLI 首要消费者是 AI，这套契约可以直接搬。
- noun-verb 加统一动词词表的价值随命令数量增长：四个动词时看不出差别，第二个版本扩类（运维、插件）时词表测试就是防漂移的地基。v1 就把测试立起来。
- `aw-status` 绕过信封输出固定形状，是「消费者已知、契约锁定」时的合理例外。AgentWatcher v1 不需要这种例外，全部命令都走信封。
- AgentWatcher 比 udf 多一层现成优势：领域逻辑已经在 library crate（`agentwatcher_lib`）里，CLI 二进制直接链接即可，不用像 udf 测试那样用 `#[path]` 引入源文件。

## 没读到的地方

- 21 个命令模块只通读了 build_policy、aw_status、skills 三个，其余看的是签名和帮助文本。
- `src/git/`、`src/host/` 等领域模块没有逐行读，只确认了它们不感知 clap。
