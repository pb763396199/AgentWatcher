#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

// GUI 壳只在 gui feature（默认开启）下存在；`--no-default-features` 是给
// headless CLI 构建用的，直接构建 GUI 二进制时给出明确报错而不是链接失败。
#[cfg(feature = "gui")]
fn main() {
    agentwatcher_lib::run()
}

#[cfg(not(feature = "gui"))]
fn main() {
    eprintln!("AgentWatcher GUI 需要 gui feature（默认开启）；headless 构建请用 --bin agentwatcher-cli");
    std::process::exit(2);
}
