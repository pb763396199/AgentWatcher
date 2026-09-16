mod cli;
mod commands;
mod output;
mod skill;

use clap::Parser;
use cli::{Cli, Commands, HandoffAction, SessionAction, SkillAction};

fn main() {
    // headless 无副作用：不拉起常驻 Codex app-server（扫描走本地会话文件），
    // 并清掉自身 stdio 句柄的继承标志，保证任何子进程都攥不住我们的输出管道。
    agentwatcher_lib::set_codex_app_server_disabled(true);
    clear_stdio_inherit_flags();

    let parsed = Cli::parse();
    output::set_format(parsed.format);
    match parsed.command {
        Commands::Session { action } => match action {
            SessionAction::List {
                provider,
                status,
                workspace,
                limit,
                active_days,
            } => commands::session_list(
                &provider,
                status.as_deref(),
                workspace.as_deref(),
                limit,
                active_days,
            ),
            SessionAction::Show { id, content } => commands::session_show(&id, content),
            SessionAction::Usage { id } => commands::session_usage(&id),
        },
        Commands::Handoff { action } => match action {
            HandoffAction::Export { id } => commands::handoff_export(&id),
        },
        Commands::Skill { action } => match action {
            SkillAction::Install { dir } => skill::skill_install(dir.as_deref()),
            SkillAction::List { dir } => skill::skill_list(dir.as_deref()),
            SkillAction::Remove { dir } => skill::skill_remove(dir.as_deref()),
        },
    }
}

#[cfg(target_os = "windows")]
fn clear_stdio_inherit_flags() {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE, HANDLE_FLAG_INHERIT};

    unsafe {
        let stdout = std::io::stdout();
        SetHandleInformation(stdout.as_raw_handle() as HANDLE, HANDLE_FLAG_INHERIT, 0);
        let stderr = std::io::stderr();
        SetHandleInformation(stderr.as_raw_handle() as HANDLE, HANDLE_FLAG_INHERIT, 0);
    }
}

#[cfg(not(target_os = "windows"))]
fn clear_stdio_inherit_flags() {}
