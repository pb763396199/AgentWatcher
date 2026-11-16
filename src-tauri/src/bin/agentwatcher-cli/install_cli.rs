// install-cli：把 agentwatcher-cli 自身复制进 PATH 可达目录。
// 技能文档（SKILL.md）教 agent 用裸命令 `agentwatcher-cli ...`，源码树构建产物
// 在 target/ 下不在 PATH，macOS/Linux 上没有 Windows 发布包「与 AgentWatcher.exe
// 同目录」的兜底——本命令补上这个缺口：装完后技能与 CLI 配套成立。

use crate::output;
use std::path::{Path, PathBuf};

pub fn install_cli(dir: Option<&str>) {
    let Some(source) = std::env::current_exe().ok() else {
        output::finish_failure(
            "install-cli",
            "locate_failed",
            "无法定位当前可执行文件，安装中止".to_string(),
        )
    };

    let target_dir = match dir.map(str::trim).filter(|value| !value.is_empty()) {
        Some(custom) => PathBuf::from(custom),
        None => default_bin_dir(),
    };
    if let Err(error) = std::fs::create_dir_all(&target_dir) {
        output::finish_failure(
            "install-cli",
            "mkdir_failed",
            format!("无法创建目录 {}: {error}", target_dir.display()),
        );
    }
    let target = target_dir.join(cli_file_name());
    if target.exists() && is_same_file(&source, &target) {
        // 重装到自身（--dir 指向当前 exe 所在目录）：Windows 无法覆盖运行中的 exe，
        // Unix 覆盖运行中文件虽可行但没意义，直接按已安装收尾。
        finish(&source, &target);
    }
    if let Err(error) = std::fs::copy(&source, &target) {
        output::finish_failure(
            "install-cli",
            "copy_failed",
            format!(
                "无法复制 {} -> {}: {error}",
                source.display(),
                target.display()
            ),
        );
    }
    make_executable(&target);
    finish(&source, &target);
}

/// 默认安装目录：AGENTWATCHER_CLI_BIN 显式覆盖，否则统一用专用目录
/// `~/.agentwatcher/bin`（不与 ~/.local/bin、nvm 等第三方目录耦合；不在 PATH 时
/// 由 install-cli 自动幂等写 rc）。macOS 实测用户的交互 PATH 常常不含 ~/.local/bin，
/// 「装进惯例目录然后只给提示」并不能让裸命令可用——专用目录 + 自动 rc 才闭环。
fn default_bin_dir() -> PathBuf {
    if let Some(custom) = std::env::var_os("AGENTWATCHER_CLI_BIN") {
        let custom = PathBuf::from(custom);
        if !custom.as_os_str().is_empty() {
            return custom;
        }
    }
    let Some(home) = user_home() else {
        // 无 HOME 极端环境：退回当前目录，让失败信息可见而不是 panic。
        return PathBuf::from(".");
    };
    home.join(".agentwatcher").join("bin")
}

fn cli_file_name() -> &'static str {
    if cfg!(windows) {
        "agentwatcher-cli.exe"
    } else {
        "agentwatcher-cli"
    }
}

fn user_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn is_same_file(left: &Path, right: &Path) -> bool {
    std::fs::canonicalize(left).ok() == std::fs::canonicalize(right).ok()
}

fn make_executable(target: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(target) {
            let mut permissions = meta.permissions();
            permissions.set_mode(permissions.mode() | 0o755);
            let _ = std::fs::set_permissions(target, permissions);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = target;
    }
}

fn path_entries() -> Vec<PathBuf> {
    let separator = if cfg!(windows) { ';' } else { ':' };
    std::env::var("PATH")
        .map(|value| value.split(separator).map(PathBuf::from).collect())
        .unwrap_or_default()
}

fn in_path(dir: &Path) -> bool {
    let dir_canonical = std::fs::canonicalize(dir).ok();
    path_entries().iter().any(|entry| {
        entry == dir
            || std::fs::canonicalize(entry)
                .map(|canonical| Some(canonical) == dir_canonical)
                .unwrap_or(false)
    })
}

fn finish(source: &Path, target: &Path) {
    let target_dir = target.parent().unwrap_or_else(|| Path::new("."));
    let dir_display = target_dir.display().to_string();
    // 进程 PATH 往往比交互 shell 的短；rc 写入按「文件里是否已有本目录」幂等判断，
    // 不依赖进程 PATH 是否可见。
    let rc_updated = if in_path(target_dir) {
        false
    } else if cfg!(windows) {
        // Windows 改用户 PATH 走注册表/系统设置，不自动动——给出指引即可。
        false
    } else {
        append_path_export_to_rc(&dir_display)
    };
    let in_path = in_path(target_dir);
    let (path_hint, hint_text) = if in_path {
        (None, "已包含安装目录".to_string())
    } else if cfg!(windows) {
        (
            Some(format!("把 {dir_display} 加入用户 PATH")),
            format!("当前 PATH 不含 {dir_display}——在系统设置里把它加入用户 PATH 后重开终端（或让 AgentWatcher 安装目录整体入 PATH）"),
        )
    } else if rc_updated {
        (
            None,
            format!("已把 {dir_display} 写进 shell 配置，重开终端（或 source 对应 rc）后裸命令生效"),
        )
    } else {
        (
            Some(format!("export PATH=\"{dir_display}:$PATH\"")),
            format!("自动写 rc 未生效——把 pathHint 这行手动加进 ~/.zshrc 或 ~/.bashrc 后重开终端"),
        )
    };
    output::finish_success(
        "install-cli",
        serde_json::json!({
            "target": target.display().to_string(),
            "source": source.display().to_string(),
            "inPath": in_path,
            "rcUpdated": rc_updated,
            "pathHint": path_hint,
        }),
        Some(format!(
            "installed: {}\nPATH: {hint_text}\n",
            target.display()
        )),
    )
}

/// 把 `export PATH="<dir>:$PATH"` 幂等追加进用户 shell 配置。
/// 目标选择：已存在的 ~/.zshrc → ~/.zprofile → ~/.bashrc → ~/.profile，
/// 都不存在时按平台创建 ~/.zshrc（macOS 默认 zsh）或 ~/.profile。
/// 返回是否真的写入了（已含该目录时返回 false，视为无需变更）。
fn append_path_export_to_rc(dir_display: &str) -> bool {
    let Some(home) = user_home() else {
        return false;
    };
    let candidates = [
        home.join(".zshrc"),
        home.join(".zprofile"),
        home.join(".bashrc"),
        home.join(".profile"),
    ];
    let existing = candidates.iter().find(|path| path.is_file());
    let target = match existing {
        Some(path) => path.clone(),
        None => {
            let fallback = if cfg!(target_os = "macos") {
                home.join(".zshrc")
            } else {
                home.join(".profile")
            };
            fallback
        }
    };
    let existing_content = std::fs::read_to_string(&target).unwrap_or_default();
    if existing_content.contains(dir_display) {
        // 目录已在 rc 里（可能 PATH 环境没加载到本进程），无需重复写。
        return false;
    }
    let line = format!("\nexport PATH=\"{dir_display}:$PATH\"\n");
    let mut to_write = existing_content;
    if !to_write.ends_with('\n') && !to_write.is_empty() {
        to_write.push('\n');
    }
    to_write.push_str(&line);
    std::fs::write(&target, to_write).is_ok()
}
