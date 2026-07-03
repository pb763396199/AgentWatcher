# 命令契约标准

`uwf` 是唯一公开命令入口。所有机器接口必须支持 `--json`。

## 顶层命令

- `uwf doctor`
- `uwf modules`
- `uwf master plan`
- `uwf master dry-run`
- `uwf master execute`

## 模块命令

每个业务模块都必须提供：

- `status`
- `capabilities`
- `commands`
- `schema`
- `dry-run`
- `execute`
- `history`
- `artifacts`

## 安全规则

- `dry-run` 不执行外部副作用。
- `execute` 必须检查安全等级、确认边界和命令清单。
- 默认禁止任意 shell、raw UE build、破坏性删除。
- 失败必须返回结构化错误，不能静默降级。

命令面 golden 文件是 `Tests/contracts/uwf-command-surface.golden.txt`。
