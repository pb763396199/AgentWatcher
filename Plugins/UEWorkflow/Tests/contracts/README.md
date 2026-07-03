# 契约测试

当前契约测试位于各 Rust package 内：

- `Source/Core/src/lib.rs`
- `Source/UnrealMaster/src/lib.rs`
- `Source/Cli/tests/cli_contract.rs`

统一入口：

```powershell
cargo test --manifest-path Plugins/UEWorkflow/Cargo.toml
```

本目录用于后续放跨模块 fixture、golden JSON 和 provider 兼容样例。
