// skill 分发：把内嵌的用法文档装进本机 AI 宿主的技能目录。
// 状态判定按文件内容逐字节比对，重装即升级；自动化测试只经 --dir 打临时目录。

use crate::output;
use serde::Serialize;
use std::path::{Path, PathBuf};

const SKILL_DOC: &str = include_str!("skill/SKILL.md");
const SKILL_DIR_NAME: &str = "agentwatcher";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HostStatus {
    host: String,
    path: String,
    state: String,
}

fn candidate_roots() -> Vec<(&'static str, PathBuf)> {
    let Some(home) = std::env::var_os("USERPROFILE") else {
        return Vec::new();
    };
    let home = PathBuf::from(home);
    vec![
        ("zcode", home.join(".zcode").join("skills")),
        ("claude-code", home.join(".claude").join("skills")),
        ("agents", home.join(".agents").join("skills")),
        ("codex", home.join(".codex").join("skills")),
        ("opencode", home.join(".config").join("opencode").join("skill")),
    ]
}

fn skill_file(root: &Path) -> PathBuf {
    root.join(SKILL_DIR_NAME).join("SKILL.md")
}

fn state_of(root: &Path) -> &'static str {
    if !root.is_dir() {
        return "host-absent";
    }
    match std::fs::read_to_string(skill_file(root)) {
        Ok(content) if content == SKILL_DOC => "installed",
        Ok(_) => "outdated",
        Err(_) => "not-installed",
    }
}

/// --dir 给了就只作用于那个目录（不存在也照建），否则取所有目录已存在的候选宿主。
fn resolve_targets(dir: Option<&str>) -> Result<Vec<(String, PathBuf)>, String> {
    if let Some(custom) = dir.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(vec![("custom".to_string(), PathBuf::from(custom))]);
    }
    let targets: Vec<(String, PathBuf)> = candidate_roots()
        .into_iter()
        .filter(|(_, root)| root.is_dir())
        .map(|(host, root)| (host.to_string(), root))
        .collect();
    if targets.is_empty() {
        return Err("没找到任何已安装宿主的技能目录，用 --dir 显式指定目标目录".to_string());
    }
    Ok(targets)
}

pub fn skill_install(dir: Option<&str>) {
    let targets = match resolve_targets(dir) {
        Ok(targets) => targets,
        Err(message) => output::finish_failure("skill install", "no_host_found", message),
    };
    let mut installed = Vec::new();
    let mut human = String::new();
    for (_, root) in &targets {
        let target_dir = root.join(SKILL_DIR_NAME);
        if let Err(error) = std::fs::create_dir_all(&target_dir) {
            output::finish_failure(
                "skill install",
                "install_failed",
                format!("无法创建目录 {}: {}", target_dir.display(), error),
            );
        }
        if let Err(error) = std::fs::write(skill_file(root), SKILL_DOC) {
            output::finish_failure(
                "skill install",
                "install_failed",
                format!("无法写入技能文件: {}", error),
            );
        }
        installed.push(skill_file(root).to_string_lossy().into_owned());
        human.push_str(&format!(
            "installed: {}\n",
            skill_file(root).to_string_lossy()
        ));
    }
    let data = serde_json::json!({ "installed": installed });
    output::finish_success("skill install", data, Some(human));
}

pub fn skill_list(dir: Option<&str>) {
    let entries: Vec<(String, PathBuf)> = if let Some(custom) = dir.map(str::trim).filter(|value| !value.is_empty()) {
        vec![("custom".to_string(), PathBuf::from(custom))]
    } else {
        candidate_roots()
            .into_iter()
            .map(|(host, root)| (host.to_string(), root))
            .collect()
    };
    let hosts: Vec<HostStatus> = entries
        .iter()
        .map(|(host, root)| HostStatus {
            host: host.clone(),
            path: skill_file(root).to_string_lossy().into_owned(),
            state: state_of(root).to_string(),
        })
        .collect();
    let mut human = String::new();
    human.push_str(&format!("{:<12}{:<12}{}\n", "HOST", "STATE", "PATH"));
    for status in &hosts {
        human.push_str(&format!(
            "{:<12}{:<12}{}\n",
            status.host, status.state, status.path
        ));
    }
    let data = serde_json::json!({ "hosts": serde_json::to_value(&hosts).expect("status serializes") });
    output::finish_success("skill list", data, Some(human));
}

pub fn skill_remove(dir: Option<&str>) {
    let targets = match resolve_targets(dir) {
        Ok(targets) => targets,
        Err(message) => output::finish_failure("skill remove", "no_host_found", message),
    };
    let mut removed = Vec::new();
    let mut missing = Vec::new();
    let mut human = String::new();
    for (_, root) in &targets {
        let target_file = skill_file(root);
        if target_file.exists() {
            if let Err(error) = std::fs::remove_file(&target_file) {
                output::finish_failure(
                    "skill remove",
                    "remove_failed",
                    format!("无法删除 {}: {}", target_file.display(), error),
                );
            }
            let target_dir = root.join(SKILL_DIR_NAME);
            if dir_is_empty(&target_dir) {
                let _ = std::fs::remove_dir(&target_dir);
            }
            removed.push(target_file.to_string_lossy().into_owned());
            human.push_str(&format!("removed: {}\n", target_file.to_string_lossy()));
        } else {
            missing.push(target_file.to_string_lossy().into_owned());
        }
    }
    let data = serde_json::json!({ "removed": removed, "missing": missing });
    output::finish_success("skill remove", data, Some(human));
}

fn dir_is_empty(path: &Path) -> bool {
    path.read_dir().map(|mut entries| entries.next().is_none()).unwrap_or(false)
}
