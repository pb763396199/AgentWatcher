fn main() {
    // GUI 关掉时（`--no-default-features` 的 headless CLI 构建）不跑 tauri_build：
    // 它要消费 tauri.conf.json 与前端产物约定，且 `tauri::generate_context!` 只在
    // gui feature 里编译。feature 对 build script 只经 `CARGO_FEATURE_<NAME>` 可见。
    if std::env::var_os("CARGO_FEATURE_GUI").is_some() {
        tauri_build::build();
    } else {
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rerun-if-env-changed=CARGO_FEATURE_GUI");
    }
}
