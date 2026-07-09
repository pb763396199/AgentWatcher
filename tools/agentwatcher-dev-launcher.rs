#![windows_subsystem = "windows"]

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn main() {
    if let Err(error) = launch() {
        let _ = write_error(&error);
    }
}

fn launch() -> Result<(), String> {
    let exe = env::current_exe().map_err(|error| format!("current_exe failed: {error}"))?;
    let tools_dir = exe
        .parent()
        .ok_or_else(|| "launcher has no parent directory".to_string())?;
    let repo_root = tools_dir
        .parent()
        .ok_or_else(|| "tools directory has no parent repository".to_string())?;
    let node = find_node(repo_root)?;
    let tmp_dir = repo_root.join(".tmp");
    fs::create_dir_all(&tmp_dir).map_err(|error| format!("create .tmp failed: {error}"))?;

    let path = launcher_path(repo_root, &node);
    let mut command = Command::new(node);
    command
        .arg(repo_root.join("tools").join("tauri-dev.mjs"))
        .arg("--restart")
        .current_dir(repo_root)
        .env("PATH", path)
        .env("AGENTWATCHER_DEV_SILENT", "1")
        .env("AGENTWATCHER_DEBUG_SILENT", "1")
        .env(
            "WEBVIEW2_USER_DATA_FOLDER",
            repo_root.join(".tmp").join("tauri-dev-webview2"),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    command
        .spawn()
        .map_err(|error| format!("spawn dev runner failed: {error}"))?;
    Ok(())
}

fn find_node(repo_root: &Path) -> Result<PathBuf, String> {
    let user_profile = env::var("USERPROFILE").unwrap_or_default();
    let candidates = [
        PathBuf::from(r"C:\Program Files\nodejs\node.exe"),
        PathBuf::from(&user_profile).join(
            r".cache\codex-runtimes\codex-primary-runtime\dependencies\node\bin\node.exe",
        ),
        repo_root.join(r"node\node.exe"),
    ];
    candidates
        .iter()
        .find(|path| path.is_file())
        .cloned()
        .ok_or_else(|| "Node.js not found".to_string())
}

fn launcher_path(repo_root: &Path, node: &Path) -> String {
    let user_profile = env::var("USERPROFILE").unwrap_or_default();
    let mut parts = vec![
        node.parent()
            .unwrap_or_else(|| Path::new(r"C:\Program Files\nodejs"))
            .to_string_lossy()
            .to_string(),
        PathBuf::from(&user_profile)
            .join(r".cargo\bin")
            .to_string_lossy()
            .to_string(),
        PathBuf::from(&user_profile)
            .join(r".unrealworkflow\bin")
            .to_string_lossy()
            .to_string(),
        PathBuf::from(&user_profile)
            .join(r".unrealdevflow\bin")
            .to_string_lossy()
            .to_string(),
        repo_root
            .join("node_modules")
            .join(".bin")
            .to_string_lossy()
            .to_string(),
    ];
    if let Ok(existing) = env::var("PATH") {
        parts.push(existing);
    }
    parts.join(";")
}

fn write_error(message: &str) -> Result<(), std::io::Error> {
    let exe = env::current_exe()?;
    let repo_root = exe
        .parent()
        .and_then(Path::parent)
        .unwrap_or_else(|| Path::new("."));
    let tmp_dir = repo_root.join(".tmp");
    fs::create_dir_all(&tmp_dir)?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(tmp_dir.join("agentwatcher-dev-launcher-error.log"))?;
    writeln!(file, "{message}")
}
