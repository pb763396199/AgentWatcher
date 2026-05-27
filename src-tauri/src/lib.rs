use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::Manager;

const DEFAULT_MAX_ACTIVE_SESSIONS: usize = 80;
const DEFAULT_ACTIVE_SESSION_DAYS: u64 = 7;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentSession {
    id: String,
    provider: String,
    provider_label: String,
    title: String,
    workspace: String,
    workspace_path: Option<String>,
    session_path: Option<String>,
    status: String,
    time_label: String,
    updated_ms: u64,
    message_count: u32,
    branch: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenSessionRequest {
    workspace_path: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScanOptions {
    max_sessions: Option<usize>,
    active_window_days: Option<u64>,
    hide_archived: Option<bool>,
    include_copilot: Option<bool>,
    include_claude: Option<bool>,
}

#[derive(Debug, Clone)]
struct ResolvedScanOptions {
    max_sessions: usize,
    active_window_ms: u64,
    hide_archived: bool,
    include_copilot: bool,
    include_claude: bool,
}

#[tauri::command]
fn scan_sessions(options: Option<ScanOptions>) -> Vec<AgentSession> {
    let scan_time_ms = current_time_ms();
    let options = resolve_scan_options(options);
    let mut sessions = Vec::new();

    if options.include_copilot {
        sessions.extend(scan_copilot_sessions(scan_time_ms, &options));
    }
    if options.include_claude {
        sessions.extend(scan_claude_sessions(scan_time_ms, &options));
    }

    sessions.sort_by(|left_session, right_session| {
        status_rank(&left_session.status)
            .cmp(&status_rank(&right_session.status))
            .then_with(|| right_session.updated_ms.cmp(&left_session.updated_ms))
    });
    sessions.truncate(options.max_sessions);
    sessions
}

#[tauri::command]
fn open_session(session: OpenSessionRequest) -> Result<(), String> {
    let workspace_path = session
        .workspace_path
        .as_deref()
        .map(str::trim)
        .filter(|path_text| !path_text.is_empty());

    let mut arguments = Vec::new();
    if let Some(workspace_path) = workspace_path {
        arguments.push("--reuse-window".to_string());
        arguments.push("--agents".to_string());
        arguments.push(workspace_path.to_string());
    } else {
        arguments.push("--agents".to_string());
    }

    spawn_code_cli(&arguments)
}

fn scan_copilot_sessions(scan_time_ms: u64, options: &ResolvedScanOptions) -> Vec<AgentSession> {
    let Some(appdata_path) = env::var_os("APPDATA") else {
        return Vec::new();
    };

    let storage_root = PathBuf::from(appdata_path)
        .join("Code")
        .join("User")
        .join("workspaceStorage");

    read_directory(&storage_root)
        .into_iter()
        .filter_map(|storage_entry| {
            let storage_path = storage_entry.path();
            if !storage_path.is_dir() {
                return None;
            }

            let workspace_info = read_workspace_info(&storage_path.join("workspace.json"));
            Some((storage_path, workspace_info))
        })
        .flat_map(|(storage_path, workspace_info)| {
            let chat_sessions_path = storage_path.join("chatSessions");
            read_directory(&chat_sessions_path)
                .into_iter()
                .filter_map(move |session_entry| {
                    let session_path = session_entry.path();
                    if !has_extension(&session_path, "jsonl") {
                        return None;
                    }

                    let updated_ms = modified_time_ms(&session_path).unwrap_or(scan_time_ms);
                    if !is_active_session(updated_ms, scan_time_ms, options) {
                        return None;
                    }

                    read_copilot_session(&session_path, &workspace_info, scan_time_ms, updated_ms, options)
                })
        })
        .collect()
}

fn read_copilot_session(
    session_path: &Path,
    workspace_info: &WorkspaceInfo,
    scan_time_ms: u64,
    updated_ms: u64,
    options: &ResolvedScanOptions,
) -> Option<AgentSession> {
    let session_file = File::open(session_path).ok()?;
    let session_reader = BufReader::new(session_file);
    let mut session_id = file_stem(session_path).unwrap_or_else(|| "unknown".to_string());
    let mut title = None;
    let mut message_count = 0u32;

    for line_result in session_reader.lines().take(80) {
        let Ok(line_text) = line_result else {
            continue;
        };

        if line_text.trim().is_empty() {
            continue;
        }

        let Ok(json_value) = serde_json::from_str::<Value>(&line_text) else {
            continue;
        };

        if options.hide_archived && is_archived_json(&json_value) {
            return None;
        }

        if let Some(next_session_id) = string_at(&json_value, &["/v/sessionId", "/sessionId"]) {
            session_id = clean_label(next_session_id, 96);
        }

        let input_text = input_text_from_copilot_event(&json_value);
        if input_text.is_some() || json_kind(&json_value) == Some(1) {
            message_count = message_count.saturating_add(1);
        }

        if title.is_none() {
            if let Some(input_text) = input_text.filter(|text_value| !text_value.trim().is_empty()) {
                title = Some(clean_label(input_text, 80));
            }
        }
    }

    let status = status_from_updated_ms(updated_ms, scan_time_ms);
    let title = title.unwrap_or_else(|| format!("Copilot {}", short_id(&session_id)));

    Some(AgentSession {
        id: format!("copilot:{}", session_id),
        provider: "copilot".to_string(),
        provider_label: "GH".to_string(),
        title,
        workspace: workspace_info.display_name.clone(),
        workspace_path: workspace_info.path_text.clone(),
        session_path: Some(path_to_string(session_path)),
        status,
        time_label: time_label(updated_ms, scan_time_ms),
        updated_ms,
        message_count,
        branch: None,
    })
}

fn scan_claude_sessions(scan_time_ms: u64, options: &ResolvedScanOptions) -> Vec<AgentSession> {
    let Some(user_profile_path) = env::var_os("USERPROFILE") else {
        return Vec::new();
    };

    let projects_root = PathBuf::from(user_profile_path).join(".claude").join("projects");

    read_directory(&projects_root)
        .into_iter()
        .filter_map(|project_entry| {
            let project_path = project_entry.path();
            if !project_path.is_dir() {
                return None;
            }

            Some(project_path.join("sessions-index.json"))
        })
        .flat_map(|index_path| read_claude_index(&index_path, scan_time_ms, options))
        .collect()
}

    fn read_claude_index(index_path: &Path, scan_time_ms: u64, options: &ResolvedScanOptions) -> Vec<AgentSession> {
    let Some(index_json) = read_json_file(index_path) else {
        return Vec::new();
    };

    let Some(entries) = index_json
        .get("entries")
        .and_then(Value::as_array)
        .or_else(|| index_json.as_array())
    else {
        return Vec::new();
    };

    entries
        .iter()
        .filter_map(|entry_json| read_claude_entry(entry_json, index_path, scan_time_ms, options))
        .collect()
}

fn read_claude_entry(
    entry_json: &Value,
    index_path: &Path,
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
) -> Option<AgentSession> {
    if options.hide_archived && is_archived_json(entry_json) {
        return None;
    }

    let raw_session_id = string_at(entry_json, &["/sessionId", "/id"])
        .map(|session_id| clean_label(session_id, 96));
    let session_path = string_at(entry_json, &["/fullPath", "/path"]).map(str::to_string);
    let workspace_path = string_at(entry_json, &["/projectPath", "/cwd"]).map(str::to_string);
    let session_id = raw_session_id.unwrap_or_else(|| {
        session_path
            .as_deref()
            .and_then(|path_text| file_stem(Path::new(path_text)))
            .unwrap_or_else(|| "unknown".to_string())
    });

    let updated_ms = entry_json
        .get("fileMtime")
        .and_then(millis_from_value)
        .or_else(|| session_path.as_deref().and_then(|path_text| modified_time_ms(Path::new(path_text))))
        .unwrap_or_else(|| modified_time_ms(index_path).unwrap_or(scan_time_ms));
    if !is_active_session(updated_ms, scan_time_ms, options) {
        return None;
    }

    let status = status_from_updated_ms(updated_ms, scan_time_ms);
    let workspace = workspace_path
        .as_deref()
        .map(Path::new)
        .and_then(display_name_from_path)
        .or_else(|| index_path.parent().and_then(display_name_from_path))
        .unwrap_or_else(|| "Claude".to_string());
    let title = string_at(entry_json, &["/firstPrompt", "/title"])
        .filter(|title_text| !title_text.trim().is_empty())
        .map(|title_text| clean_label(title_text, 80))
        .unwrap_or_else(|| format!("Claude {}", short_id(&session_id)));
    let message_count = entry_json
        .get("messageCount")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u64::from(u32::MAX)) as u32;
    let branch = string_at(entry_json, &["/gitBranch", "/branch"])
        .filter(|branch_text| !branch_text.trim().is_empty())
        .map(|branch_text| clean_label(branch_text, 48));

    Some(AgentSession {
        id: format!("claude:{}", session_id),
        provider: "claude".to_string(),
        provider_label: "C".to_string(),
        title,
        workspace,
        workspace_path: workspace_path.filter(|path_text| !path_text.trim().is_empty()),
        session_path: session_path.filter(|path_text| !path_text.trim().is_empty()),
        status,
        time_label: time_label(updated_ms, scan_time_ms),
        updated_ms,
        message_count,
        branch,
    })
}

#[derive(Debug, Clone)]
struct WorkspaceInfo {
    display_name: String,
    path_text: Option<String>,
}

fn read_workspace_info(workspace_json_path: &Path) -> WorkspaceInfo {
    let workspace_path = read_json_file(workspace_json_path)
        .and_then(|workspace_json| string_at(&workspace_json, &["/folder"]).map(str::to_string))
        .and_then(|folder_uri| decode_file_uri(&folder_uri));
    let display_name = workspace_path
        .as_deref()
        .and_then(display_name_from_path)
        .unwrap_or_else(|| "Unknown".to_string());
    let path_text = workspace_path.as_deref().map(path_to_string);

    WorkspaceInfo {
        display_name,
        path_text,
    }
}

fn resolve_scan_options(options: Option<ScanOptions>) -> ResolvedScanOptions {
    let options = options.unwrap_or(ScanOptions {
        max_sessions: None,
        active_window_days: None,
        hide_archived: None,
        include_copilot: None,
        include_claude: None,
    });
    let active_days = options
        .active_window_days
        .unwrap_or(DEFAULT_ACTIVE_SESSION_DAYS)
        .clamp(1, 30);

    ResolvedScanOptions {
        max_sessions: options
            .max_sessions
            .unwrap_or(DEFAULT_MAX_ACTIVE_SESSIONS)
            .clamp(10, 300),
        active_window_ms: active_days.saturating_mul(24 * 60 * 60 * 1000),
        hide_archived: options.hide_archived.unwrap_or(true),
        include_copilot: options.include_copilot.unwrap_or(true),
        include_claude: options.include_claude.unwrap_or(true),
    }
}

fn read_directory(directory_path: &Path) -> Vec<fs::DirEntry> {
    fs::read_dir(directory_path)
        .map(|entries| entries.filter_map(Result::ok).collect())
        .unwrap_or_default()
}

fn read_json_file(json_path: &Path) -> Option<Value> {
    let json_text = fs::read_to_string(json_path).ok()?;
    serde_json::from_str(&json_text).ok()
}

fn string_at<'json>(json_value: &'json Value, pointers: &[&str]) -> Option<&'json str> {
    pointers
        .iter()
        .find_map(|pointer| json_value.pointer(pointer).and_then(Value::as_str))
}

fn input_text_from_copilot_event(json_value: &Value) -> Option<&str> {
    if let Some(input_text) = string_at(
        json_value,
        &[
            "/inputState/inputText",
            "/data/inputState/inputText",
            "/v/inputState/inputText",
        ],
    ) {
        return Some(input_text);
    }

    let key_path = json_value
        .get("k")
        .or_else(|| json_value.pointer("/data/k"))
        .and_then(Value::as_array)?;
    let is_user_text_patch = json_path_ends_with(key_path, &["inputState", "inputText"])
        || json_path_ends_with(key_path, &["message", "text"]);

    if is_user_text_patch {
        json_value
            .get("v")
            .or_else(|| json_value.pointer("/data/v"))
            .and_then(Value::as_str)
    } else {
        None
    }
}

fn json_path_ends_with(key_path: &[Value], suffix: &[&str]) -> bool {
    key_path.len() >= suffix.len()
        && key_path[key_path.len() - suffix.len()..]
            .iter()
            .zip(suffix.iter())
            .all(|(path_part, expected)| path_part.as_str() == Some(*expected))
}

fn json_kind(json_value: &Value) -> Option<i64> {
    json_value
        .get("kind")
        .or_else(|| json_value.pointer("/data/kind"))
        .and_then(Value::as_i64)
}

fn is_active_session(updated_ms: u64, scan_time_ms: u64, options: &ResolvedScanOptions) -> bool {
    scan_time_ms.saturating_sub(updated_ms) <= options.active_window_ms
}

fn is_archived_json(json_value: &Value) -> bool {
    if bool_at(
        json_value,
        &[
            "/archived",
            "/isArchived",
            "/isArchivedSession",
            "/isDeleted",
            "/isHidden",
            "/deleted",
            "/hidden",
            "/v/archived",
            "/v/isArchived",
            "/v/isArchivedSession",
            "/v/isDeleted",
            "/v/isHidden",
            "/v/deleted",
            "/v/hidden",
        ],
    ) == Some(true)
    {
        return true;
    }

    if string_at(json_value, &["/status", "/state", "/v/status", "/v/state"])
        .map(|status_text| status_text.eq_ignore_ascii_case("archived"))
        .unwrap_or(false)
    {
        return true;
    }

    let Some(key_path) = json_value
        .get("k")
        .or_else(|| json_value.pointer("/data/k"))
        .and_then(Value::as_array)
    else {
        return false;
    };

    let is_archive_patch = [
        "archived",
        "isArchived",
        "isArchivedSession",
        "isDeleted",
        "isHidden",
        "deleted",
        "hidden",
    ]
    .iter()
    .any(|field_name| json_path_ends_with(key_path, &[*field_name]));

    is_archive_patch
        && json_value
            .get("v")
            .or_else(|| json_value.pointer("/data/v"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
}

fn bool_at(json_value: &Value, pointers: &[&str]) -> Option<bool> {
    pointers
        .iter()
        .find_map(|pointer| json_value.pointer(pointer).and_then(Value::as_bool))
}

fn decode_file_uri(uri_text: &str) -> Option<PathBuf> {
    let encoded_path = uri_text.strip_prefix("file://")?;
    let decoded_path = percent_decode(encoded_path);
    let mut path_text = decoded_path.replace('/', "\\");

    if path_text.starts_with('\\')
        && path_text.len() > 2
        && path_text.as_bytes().get(2).copied() == Some(b':')
    {
        path_text.remove(0);
    } else if !decoded_path.starts_with('/') && decoded_path.contains('/') {
        path_text = format!("\\\\{}", path_text);
    }

    Some(PathBuf::from(path_text))
}

fn percent_decode(encoded_text: &str) -> String {
    let encoded_bytes = encoded_text.as_bytes();
    let mut decoded_bytes = Vec::with_capacity(encoded_bytes.len());
    let mut index = 0usize;

    while index < encoded_bytes.len() {
        if encoded_bytes[index] == b'%' && index + 2 < encoded_bytes.len() {
            if let (Some(high_hex), Some(low_hex)) = (
                hex_value(encoded_bytes[index + 1]),
                hex_value(encoded_bytes[index + 2]),
            ) {
                decoded_bytes.push((high_hex << 4) | low_hex);
                index += 3;
                continue;
            }
        }

        decoded_bytes.push(encoded_bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded_bytes).into_owned()
}

fn hex_value(byte_value: u8) -> Option<u8> {
    match byte_value {
        b'0'..=b'9' => Some(byte_value - b'0'),
        b'a'..=b'f' => Some(byte_value - b'a' + 10),
        b'A'..=b'F' => Some(byte_value - b'A' + 10),
        _ => None,
    }
}

fn clean_label(label_text: &str, max_chars: usize) -> String {
    let mut output = String::new();
    let mut previous_was_space = false;
    let mut char_count = 0usize;
    let mut was_truncated = false;

    for label_char in label_text.trim().chars() {
        if char_count >= max_chars {
            was_truncated = true;
            break;
        }

        if label_char.is_whitespace() {
            if !previous_was_space && !output.is_empty() {
                output.push(' ');
                previous_was_space = true;
                char_count += 1;
            }
        } else {
            output.push(label_char);
            previous_was_space = false;
            char_count += 1;
        }
    }

    if was_truncated {
        output.push_str("...");
    }

    output
}

fn short_id(session_id: &str) -> String {
    let short_text: String = session_id.chars().take(8).collect();
    if short_text.is_empty() {
        "unknown".to_string()
    } else {
        short_text
    }
}

fn current_time_ms() -> u64 {
    system_time_ms(SystemTime::now()).unwrap_or(0)
}

fn modified_time_ms(path: &Path) -> Option<u64> {
    let modified_time = fs::metadata(path).ok()?.modified().ok()?;
    system_time_ms(modified_time)
}

fn system_time_ms(time_value: SystemTime) -> Option<u64> {
    let duration = time_value.duration_since(UNIX_EPOCH).ok()?;
    Some(duration.as_millis().min(u128::from(u64::MAX)) as u64)
}

fn millis_from_value(json_value: &Value) -> Option<u64> {
    let raw_value = match json_value {
        Value::Number(number_value) => number_value.as_u64()?,
        Value::String(string_value) => string_value.parse::<u64>().ok()?,
        _ => return None,
    };

    if raw_value > 1_000_000_000_000 {
        Some(raw_value)
    } else if raw_value > 1_000_000_000 {
        Some(raw_value.saturating_mul(1000))
    } else {
        None
    }
}

fn status_from_updated_ms(updated_ms: u64, scan_time_ms: u64) -> String {
    let age_ms = scan_time_ms.saturating_sub(updated_ms);
    if age_ms <= 20_000 {
        "running".to_string()
    } else if age_ms <= 30 * 60_000 {
        "waiting".to_string()
    } else {
        "idle".to_string()
    }
}

fn status_rank(status: &str) -> u8 {
    match status {
        "waiting" => 0,
        "running" => 1,
        _ => 2,
    }
}

fn time_label(updated_ms: u64, scan_time_ms: u64) -> String {
    let age_seconds = scan_time_ms.saturating_sub(updated_ms) / 1000;

    if age_seconds <= 20 {
        "live".to_string()
    } else if age_seconds < 60 {
        format!("{}s", age_seconds)
    } else if age_seconds < 60 * 60 {
        format!("{}m", age_seconds / 60)
    } else if age_seconds < 24 * 60 * 60 {
        format!("{}h", age_seconds / (60 * 60))
    } else {
        format!("{}d", age_seconds / (24 * 60 * 60))
    }
}

fn display_name_from_path(path: &Path) -> Option<String> {
    path.file_name()
        .map(|file_name| file_name.to_string_lossy().into_owned())
        .filter(|display_name| !display_name.trim().is_empty())
}

fn file_stem(path: &Path) -> Option<String> {
    path.file_stem()
        .map(|file_name| file_name.to_string_lossy().into_owned())
        .filter(|file_name| !file_name.trim().is_empty())
}

fn has_extension(path: &Path, expected_extension: &str) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case(expected_extension))
        .unwrap_or(false)
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn spawn_code_cli(arguments: &[String]) -> Result<(), String> {
    let mut candidates = vec!["code".to_string(), "code.cmd".to_string(), "code.exe".to_string()];

    if let Some(local_appdata_path) = env::var_os("LOCALAPPDATA") {
        let local_appdata_path = PathBuf::from(local_appdata_path);
        candidates.push(path_to_string(
            &local_appdata_path
                .join("Programs")
                .join("Microsoft VS Code")
                .join("bin")
                .join("code.cmd"),
        ));
        candidates.push(path_to_string(
            &local_appdata_path
                .join("Programs")
                .join("Microsoft VS Code Insiders")
                .join("bin")
                .join("code-insiders.cmd"),
        ));
    }

    let mut last_error = None;
    for candidate in candidates {
        match Command::new(&candidate).args(arguments).spawn() {
            Ok(_) => return Ok(()),
            Err(error) => last_error = Some(format!("{}: {}", candidate, error)),
        }
    }

    Err(last_error.unwrap_or_else(|| "Unable to launch VS Code CLI".to_string()))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![scan_sessions, open_session])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_always_on_top(true);
                let _ = window.set_decorations(false);
                let _ = window.set_resizable(true);
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running AgentWatcher");
}