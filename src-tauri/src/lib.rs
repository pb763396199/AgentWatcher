use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use tauri::Manager;

const DEFAULT_MAX_ACTIVE_SESSIONS: usize = 80;
const DEFAULT_ACTIVE_SESSION_DAYS: u64 = 7;
const COPILOT_TITLE_SAMPLE_LINES: usize = 240;
const CLAUDE_TITLE_SAMPLE_LINES: usize = 240;
const JSONL_HEAD_SAMPLE_BYTES: usize = 128 * 1024;
const JSONL_TAIL_SAMPLE_BYTES: usize = 256 * 1024;
const STATUS_RUNNING_WINDOW_MS: u64 = 20_000;
const STATUS_WAITING_WINDOW_MS: u64 = 10 * 60_000;
const STATUS_FILE_WAITING_WINDOW_MS: u64 = 3 * 60_000;
const BRIDGE_EXTENSION_FOLDER_PREFIX: &str = "agentwatcher.agentwatcher-bridge-";
const BRIDGE_EXTENSION_URI_AUTHORITY: &str = "agentwatcher.agentwatcher-bridge";
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
    session_resource: Option<String>,
    status: String,
    time_label: String,
    updated_ms: u64,
    message_count: u32,
    branch: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenSessionRequest {
    id: Option<String>,
    provider: Option<String>,
    workspace_path: Option<String>,
    session_resource: Option<String>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivitySource {
    ContentTimestamp,
    FileModified,
}

#[derive(Debug, Clone, Copy)]
struct JsonlFingerprint {
    len: u64,
    modified_ms: u64,
}

#[derive(Debug, Clone)]
struct CachedJsonlSummary<T> {
    len: u64,
    modified_ms: u64,
    summary: T,
}

#[derive(Debug, Clone)]
struct CopilotSessionSummary {
    session_id: String,
    title: Option<String>,
    latest_timestamp_ms: Option<u64>,
    message_count: u32,
    archived: bool,
}

#[derive(Debug, Clone)]
struct ClaudeSessionSummary {
    title: Option<String>,
    workspace_path: Option<String>,
    branch: Option<String>,
    latest_timestamp_ms: Option<u64>,
    message_count: u32,
    archived: bool,
}

static COPILOT_SESSION_CACHE: OnceLock<Mutex<HashMap<String, CachedJsonlSummary<CopilotSessionSummary>>>> =
    OnceLock::new();
static CLAUDE_SESSION_CACHE: OnceLock<Mutex<HashMap<String, CachedJsonlSummary<ClaudeSessionSummary>>>> =
    OnceLock::new();

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
    if bridge_extension_installed() {
        if let Some(bridge_link) = session_bridge_link(&session) {
            match open_session_with_bridge(session.workspace_path.as_deref(), &bridge_link) {
                Ok(()) => return Ok(()),
                Err(bridge_error) => {
                    if session_deep_link(&session).is_none() {
                        return open_agents_page(session.workspace_path.as_deref()).map_err(|fallback_error| {
                            format!(
                                "Bridge session launch failed ({}); fallback failed ({})",
                                bridge_error, fallback_error
                            )
                        });
                    }
                }
            }
        }
    }

    if let Some(deep_link) = session_deep_link(&session) {
        match open_vscode_deep_link(&deep_link) {
            Ok(()) => return Ok(()),
            Err(exact_error) => {
                return open_agents_page(session.workspace_path.as_deref()).map_err(|fallback_error| {
                    format!(
                        "Exact session launch failed ({}); fallback failed ({})",
                        exact_error, fallback_error
                    )
                });
            }
        }
    }

    open_agents_page(session.workspace_path.as_deref())
}

fn open_agents_page(workspace_path: Option<&str>) -> Result<(), String> {
    let workspace_path = clean_option(workspace_path);
    let mut arguments = Vec::new();
    if let Some(workspace_path) = workspace_path {
        arguments.push("--agents".to_string());
        arguments.push(workspace_path.to_string());
    } else {
        arguments.push("--agents".to_string());
    }

    spawn_code_cli(&arguments)
}

fn session_deep_link(session: &OpenSessionRequest) -> Option<String> {
    let workspace_path = clean_option(session.workspace_path.as_deref())?;
    let session_resource = request_session_resource(session)?;
    let workspace_url_path = vscode_file_url_path(workspace_path)?;
    let session_query = percent_encode_query_component(&session_resource);

    Some(format!(
        "vscode://file{}?windowId=_blank&session={}",
        workspace_url_path, session_query
    ))
}

fn session_bridge_link(session: &OpenSessionRequest) -> Option<String> {
    let session_resource = request_session_resource(session)?;
    Some(format!(
        "vscode://{}/open?target=editor&resource={}",
        BRIDGE_EXTENSION_URI_AUTHORITY,
        percent_encode_query_component(&session_resource)
    ))
}

fn request_session_resource(session: &OpenSessionRequest) -> Option<String> {
    clean_option(session.session_resource.as_deref())
        .map(str::to_string)
        .or_else(|| session_resource_from_request(session))
}

fn open_session_with_bridge(workspace_path: Option<&str>, bridge_link: &str) -> Result<(), String> {
    if let Some(workspace_path) = clean_option(workspace_path) {
        open_workspace(workspace_path)?;
    }

    let bridge_link = bridge_link.to_string();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(900));
        let _ = open_vscode_deep_link(&bridge_link);
    });

    Ok(())
}

fn open_workspace(workspace_path: &str) -> Result<(), String> {
    let arguments = vec![workspace_path.to_string()];
    spawn_code_cli(&arguments)
}

fn bridge_extension_installed() -> bool {
    let Some(user_profile_path) = env::var_os("USERPROFILE") else {
        return false;
    };

    let user_profile_path = PathBuf::from(user_profile_path);
    bridge_extension_exists_in(&user_profile_path.join(".vscode").join("extensions"))
        || bridge_extension_exists_in(&user_profile_path.join(".vscode-insiders").join("extensions"))
}

fn bridge_extension_exists_in(extensions_path: &Path) -> bool {
    read_directory(extensions_path).into_iter().any(|entry| {
        entry
            .file_name()
            .to_string_lossy()
            .starts_with(BRIDGE_EXTENSION_FOLDER_PREFIX)
    })
}

fn open_vscode_deep_link(deep_link: &str) -> Result<(), String> {
    let mut protocol_command = Command::new("rundll32.exe");
    protocol_command.args(["url.dll,FileProtocolHandler", deep_link]);

    match spawn_hidden(protocol_command) {
        Ok(_) => Ok(()),
        Err(protocol_error) => {
            let arguments = vec!["--open-url".to_string(), deep_link.to_string()];
            spawn_code_cli(&arguments).map_err(|code_error| {
                format!(
                    "Windows protocol launch failed ({}); VS Code CLI launch failed ({})",
                    protocol_error, code_error
                )
            })
        }
    }
}

fn session_resource_from_request(session: &OpenSessionRequest) -> Option<String> {
    let provider = clean_option(session.provider.as_deref())?;
    let session_id = raw_request_session_id(session.id.as_deref())?;

    match provider {
        "copilot" => Some(copilot_session_resource(session_id)),
        "claude" => Some(provider_session_resource("claude-code", session_id)),
        _ => None,
    }
}

fn raw_request_session_id(id: Option<&str>) -> Option<&str> {
    let id = clean_option(id)?;
    clean_option(Some(
        id.split_once(':').map(|(_, raw_id)| raw_id).unwrap_or(id),
    ))
}

fn copilot_session_resource(session_id: &str) -> String {
    format!(
        "vscode-chat-session://local/{}",
        base64_url_no_padding(session_id.as_bytes())
    )
}

fn provider_session_resource(provider_type: &str, session_id: &str) -> String {
    format!(
        "{}:/{}",
        provider_type,
        percent_encode_path_segment(session_id)
    )
}

fn vscode_file_url_path(workspace_path: &str) -> Option<String> {
    let mut path_text = workspace_path.trim().replace('\\', "/");
    if path_text.is_empty() {
        return None;
    }

    if is_windows_drive_path(&path_text) {
        path_text.insert(0, '/');
    } else if !path_text.starts_with('/') {
        path_text.insert(0, '/');
    }

    Some(percent_encode_url_path(&path_text))
}

fn is_windows_drive_path(path_text: &str) -> bool {
    let path_bytes = path_text.as_bytes();
    path_bytes.len() >= 2 && path_bytes[1] == b':' && path_bytes[0].is_ascii_alphabetic()
}

fn base64_url_no_padding(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut index = 0usize;

    while index + 3 <= bytes.len() {
        let first = bytes[index];
        let second = bytes[index + 1];
        let third = bytes[index + 2];
        output.push(ALPHABET[(first >> 2) as usize] as char);
        output.push(ALPHABET[(((first << 4) | (second >> 4)) & 63) as usize] as char);
        output.push(ALPHABET[(((second << 2) | (third >> 6)) & 63) as usize] as char);
        output.push(ALPHABET[(third & 63) as usize] as char);
        index += 3;
    }

    match bytes.len() - index {
        1 => {
            let first = bytes[index];
            output.push(ALPHABET[(first >> 2) as usize] as char);
            output.push(ALPHABET[((first << 4) & 63) as usize] as char);
        }
        2 => {
            let first = bytes[index];
            let second = bytes[index + 1];
            output.push(ALPHABET[(first >> 2) as usize] as char);
            output.push(ALPHABET[(((first << 4) | (second >> 4)) & 63) as usize] as char);
            output.push(ALPHABET[((second << 2) & 63) as usize] as char);
        }
        _ => {}
    }

    output
}

fn percent_encode_url_path(path_text: &str) -> String {
    percent_encode_with(path_text, |byte_value| {
        byte_value.is_ascii_alphanumeric()
            || matches!(byte_value, b'/' | b':' | b'-' | b'_' | b'.' | b'~')
    })
}

fn percent_encode_path_segment(path_segment: &str) -> String {
    percent_encode_with(path_segment, |byte_value| {
        byte_value.is_ascii_alphanumeric() || matches!(byte_value, b'-' | b'_' | b'.' | b'~')
    })
}

fn percent_encode_query_component(query_text: &str) -> String {
    percent_encode_with(query_text, |byte_value| {
        byte_value.is_ascii_alphanumeric() || matches!(byte_value, b'-' | b'_' | b'.' | b'~')
    })
}

fn percent_encode_with<F>(input_text: &str, is_allowed: F) -> String
where
    F: Fn(u8) -> bool,
{
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut output = String::new();

    for byte_value in input_text.as_bytes() {
        if is_allowed(*byte_value) {
            output.push(*byte_value as char);
        } else {
            output.push('%');
            output.push(HEX[(byte_value >> 4) as usize] as char);
            output.push(HEX[(byte_value & 15) as usize] as char);
        }
    }

    output
}

fn clean_option(text: Option<&str>) -> Option<&str> {
    text.map(str::trim).filter(|text_value| !text_value.is_empty())
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

                    read_copilot_session(&session_path, &workspace_info, scan_time_ms, options)
                })
        })
        .collect()
}

fn read_copilot_session(
    session_path: &Path,
    workspace_info: &WorkspaceInfo,
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
) -> Option<AgentSession> {
    let fingerprint = jsonl_fingerprint(session_path)?;
    let fallback_updated_ms = fallback_updated_ms(&fingerprint, scan_time_ms);
    if !is_active_session(fallback_updated_ms, scan_time_ms, options) {
        return None;
    }

    let summary = cached_or_read_copilot_session(session_path, &fingerprint)?;
    let CopilotSessionSummary {
        session_id,
        title,
        latest_timestamp_ms,
        message_count,
        archived,
    } = summary;
    if options.hide_archived && archived {
        return None;
    }

    let (updated_ms, activity_source) = latest_timestamp_ms
        .map(|timestamp_ms| (timestamp_ms, ActivitySource::ContentTimestamp))
        .unwrap_or((fallback_updated_ms, ActivitySource::FileModified));
    if !is_active_session(updated_ms, scan_time_ms, options) {
        return None;
    }

    let status = status_from_activity(updated_ms, scan_time_ms, activity_source);
    let title = title.unwrap_or_else(|| fallback_session_title("Copilot", &workspace_info.display_name, &session_id));
    let session_resource = copilot_session_resource(&session_id);

    Some(AgentSession {
        id: format!("copilot:{}", session_id),
        provider: "copilot".to_string(),
        provider_label: "GH".to_string(),
        title,
        workspace: workspace_info.display_name.clone(),
        workspace_path: workspace_info.path_text.clone(),
        session_path: Some(path_to_string(session_path)),
        session_resource: Some(session_resource),
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
        .flat_map(|project_entry| read_claude_project(&project_entry.path(), scan_time_ms, options))
        .collect()
}

fn read_claude_project(
    project_path: &Path,
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
) -> Vec<AgentSession> {
    if !project_path.is_dir() {
        return Vec::new();
    }

    let mut sessions: Vec<AgentSession> = read_directory(project_path)
        .into_iter()
        .filter_map(|session_entry| {
            let session_path = session_entry.path();
            if has_extension(&session_path, "jsonl") {
                read_claude_jsonl_session(&session_path, scan_time_ms, options)
            } else {
                None
            }
        })
        .collect();

    let mut known_session_paths: HashSet<String> = sessions
        .iter()
        .filter_map(|session| session.session_path.clone())
        .collect();
    for indexed_session in read_claude_index(&project_path.join("sessions-index.json"), scan_time_ms, options) {
        let is_known = indexed_session
            .session_path
            .as_ref()
            .map(|session_path| known_session_paths.contains(session_path))
            .unwrap_or(false);
        if !is_known {
            if let Some(session_path) = indexed_session.session_path.clone() {
                known_session_paths.insert(session_path);
            }
            sessions.push(indexed_session);
        }
    }

    sessions
}

fn read_claude_jsonl_session(
    session_path: &Path,
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
) -> Option<AgentSession> {
    let fingerprint = jsonl_fingerprint(session_path)?;
    let fallback_updated_ms = fallback_updated_ms(&fingerprint, scan_time_ms);
    if !is_active_session(fallback_updated_ms, scan_time_ms, options) {
        return None;
    }

    let summary = cached_or_read_claude_session(session_path, &fingerprint)?;
    let session_id = file_stem(session_path).unwrap_or_else(|| "unknown".to_string());
    let ClaudeSessionSummary {
        title,
        workspace_path,
        branch,
        latest_timestamp_ms,
        message_count,
        archived,
    } = summary;
    if options.hide_archived && archived {
        return None;
    }

    let (updated_ms, activity_source) = latest_timestamp_ms
        .map(|timestamp_ms| (timestamp_ms, ActivitySource::ContentTimestamp))
        .unwrap_or((fallback_updated_ms, ActivitySource::FileModified));
    if !is_active_session(updated_ms, scan_time_ms, options) {
        return None;
    }

    let status = status_from_activity(updated_ms, scan_time_ms, activity_source);
    let workspace = workspace_path
        .as_deref()
        .map(Path::new)
        .and_then(display_name_from_path)
        .or_else(|| session_path.parent().and_then(display_name_from_path))
        .unwrap_or_else(|| "Claude".to_string());
    let title = title.unwrap_or_else(|| fallback_session_title("Claude", &workspace, &session_id));

    Some(AgentSession {
        id: format!("claude:{}", session_id),
        provider: "claude".to_string(),
        provider_label: "C".to_string(),
        title,
        workspace,
        workspace_path: workspace_path.filter(|path_text| !path_text.trim().is_empty()),
        session_path: Some(path_to_string(session_path)),
        session_resource: Some(provider_session_resource("claude-code", &session_id)),
        status,
        time_label: time_label(updated_ms, scan_time_ms),
        updated_ms,
        message_count,
        branch,
    })
}

fn cached_or_read_copilot_session(
    session_path: &Path,
    fingerprint: &JsonlFingerprint,
) -> Option<CopilotSessionSummary> {
    let path_key = path_to_string(session_path);
    if let Some(summary) = cached_jsonl_summary(&COPILOT_SESSION_CACHE, &path_key, fingerprint) {
        return Some(summary);
    }

    let summary = read_copilot_session_summary(session_path, fingerprint)?;
    store_jsonl_summary(&COPILOT_SESSION_CACHE, path_key, fingerprint, summary.clone());
    Some(summary)
}

fn read_copilot_session_summary(
    session_path: &Path,
    fingerprint: &JsonlFingerprint,
) -> Option<CopilotSessionSummary> {
    let mut session_id = file_stem(session_path).unwrap_or_else(|| "unknown".to_string());
    let mut alias_title = None;
    let mut prompt_title = None;
    let mut archived = false;
    let mut latest_timestamp_ms = None;
    let mut head_message_count = 0u32;
    let mut tail_message_count = 0u32;

    visit_jsonl_head_values(session_path, fingerprint.len, COPILOT_TITLE_SAMPLE_LINES, |_, json_value| {
        if is_archived_json(json_value) {
            archived = true;
        }
        if let Some(next_session_id) = string_at(json_value, &["/v/sessionId", "/sessionId"]) {
            session_id = clean_label(next_session_id, 96);
        }
        if is_copilot_message_event(json_value) {
            head_message_count = head_message_count.saturating_add(1);
        }
        if let Some(next_title) = copilot_alias_title_text(json_value).and_then(title_candidate_from_text) {
            alias_title = Some(next_title);
        }
        if prompt_title.is_none() {
            prompt_title = copilot_title_text(json_value).and_then(title_candidate_from_text);
        }
    })?;

    let tail_covers_file = tail_sample_covers_file(fingerprint.len);
    let _ = visit_jsonl_tail_values(session_path, fingerprint.len, |json_value| {
        if is_archived_json(json_value) {
            archived = true;
        }
        latest_timestamp_ms = newest_timestamp_ms(latest_timestamp_ms, timestamp_ms_from_json(json_value));
        if is_copilot_message_event(json_value) {
            tail_message_count = tail_message_count.saturating_add(1);
        }
        if let Some(next_title) = copilot_alias_title_text(json_value).and_then(title_candidate_from_text) {
            alias_title = Some(next_title);
        }
    });

    Some(CopilotSessionSummary {
        session_id,
        title: alias_title.or(prompt_title),
        latest_timestamp_ms,
        message_count: sampled_message_count(head_message_count, tail_message_count, tail_covers_file),
        archived,
    })
}

fn cached_or_read_claude_session(
    session_path: &Path,
    fingerprint: &JsonlFingerprint,
) -> Option<ClaudeSessionSummary> {
    let path_key = path_to_string(session_path);
    if let Some(summary) = cached_jsonl_summary(&CLAUDE_SESSION_CACHE, &path_key, fingerprint) {
        return Some(summary);
    }

    let summary = read_claude_session_summary(session_path, fingerprint)?;
    store_jsonl_summary(&CLAUDE_SESSION_CACHE, path_key, fingerprint, summary.clone());
    Some(summary)
}

fn read_claude_session_summary(
    session_path: &Path,
    fingerprint: &JsonlFingerprint,
) -> Option<ClaudeSessionSummary> {
    let mut alias_title = None;
    let mut slug_title = None;
    let mut prompt_title = None;
    let mut workspace_path = None;
    let mut branch = None;
    let mut archived = false;
    let mut latest_timestamp_ms = None;
    let mut head_message_count = 0u32;
    let mut tail_message_count = 0u32;

    visit_jsonl_head_values(session_path, fingerprint.len, CLAUDE_TITLE_SAMPLE_LINES, |_, json_value| {
        if is_archived_json(json_value) {
            archived = true;
        }
        if workspace_path.is_none() {
            workspace_path = string_at(json_value, &["/cwd"]).map(str::to_string);
        }
        if branch.is_none() {
            branch = string_at(json_value, &["/gitBranch", "/branch"])
                .filter(|branch_text| !branch_text.trim().is_empty())
                .map(|branch_text| clean_label(branch_text, 48));
        }

        let event_type = string_at(json_value, &["/type"]);
        if is_claude_message_event(json_value) {
            head_message_count = head_message_count.saturating_add(1);
        }
        if let Some(next_title) = claude_alias_title_text(json_value).and_then(title_candidate_from_text) {
            alias_title = Some(next_title);
        } else if slug_title.is_none() {
            slug_title = string_at(json_value, &["/slug"]).and_then(title_candidate_from_text);
        }
        if prompt_title.is_none() && event_type == Some("user") {
            prompt_title = claude_message_text(json_value).and_then(title_candidate_from_text);
        }
    })?;

    let tail_covers_file = tail_sample_covers_file(fingerprint.len);
    let _ = visit_jsonl_tail_values(session_path, fingerprint.len, |json_value| {
        if is_archived_json(json_value) {
            archived = true;
        }
        latest_timestamp_ms = newest_timestamp_ms(latest_timestamp_ms, timestamp_ms_from_json(json_value));
        if is_claude_message_event(json_value) {
            tail_message_count = tail_message_count.saturating_add(1);
        }
        if let Some(next_title) = claude_alias_title_text(json_value).and_then(title_candidate_from_text) {
            alias_title = Some(next_title);
        } else if slug_title.is_none() {
            slug_title = string_at(json_value, &["/slug"]).and_then(title_candidate_from_text);
        }
    });

    Some(ClaudeSessionSummary {
        title: alias_title.or(slug_title).or(prompt_title),
        workspace_path,
        branch,
        latest_timestamp_ms,
        message_count: sampled_message_count(head_message_count, tail_message_count, tail_covers_file),
        archived,
    })
}

fn cached_jsonl_summary<T: Clone>(
    cache_cell: &OnceLock<Mutex<HashMap<String, CachedJsonlSummary<T>>>>,
    path_key: &str,
    fingerprint: &JsonlFingerprint,
) -> Option<T> {
    let cache = cache_cell.get_or_init(|| Mutex::new(HashMap::new()));
    let cache_guard = cache.lock().ok()?;
    cache_guard
        .get(path_key)
        .filter(|cached_summary| {
            cached_summary.len == fingerprint.len && cached_summary.modified_ms == fingerprint.modified_ms
        })
        .map(|cached_summary| cached_summary.summary.clone())
}

fn store_jsonl_summary<T>(
    cache_cell: &OnceLock<Mutex<HashMap<String, CachedJsonlSummary<T>>>>,
    path_key: String,
    fingerprint: &JsonlFingerprint,
    summary: T,
) {
    let cache = cache_cell.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut cache_guard) = cache.lock() {
        cache_guard.insert(
            path_key,
            CachedJsonlSummary {
                len: fingerprint.len,
                modified_ms: fingerprint.modified_ms,
                summary,
            },
        );
    }
}

fn jsonl_fingerprint(session_path: &Path) -> Option<JsonlFingerprint> {
    let metadata = fs::metadata(session_path).ok()?;
    let modified_ms = metadata.modified().ok().and_then(system_time_ms).unwrap_or(0);

    Some(JsonlFingerprint {
        len: metadata.len(),
        modified_ms,
    })
}

fn fallback_updated_ms(fingerprint: &JsonlFingerprint, scan_time_ms: u64) -> u64 {
    if fingerprint.modified_ms == 0 {
        scan_time_ms
    } else {
        fingerprint.modified_ms
    }
}

fn visit_jsonl_head_values<F>(
    session_path: &Path,
    file_len: u64,
    max_lines: usize,
    mut visit: F,
) -> Option<()>
where
    F: FnMut(usize, &Value),
{
    let head_text = read_file_prefix_text(session_path, file_len, JSONL_HEAD_SAMPLE_BYTES)?;
    for (line_number, line_text) in head_text.lines().take(max_lines).enumerate() {
        if let Some(json_value) = json_value_from_jsonl_line(line_text) {
            visit(line_number, &json_value);
        }
    }

    Some(())
}

fn visit_jsonl_tail_values<F>(session_path: &Path, file_len: u64, mut visit: F) -> Option<()>
where
    F: FnMut(&Value),
{
    let tail_text = read_file_suffix_text(session_path, file_len, JSONL_TAIL_SAMPLE_BYTES)?;
    for line_text in tail_text.lines() {
        if let Some(json_value) = json_value_from_jsonl_line(line_text) {
            visit(&json_value);
        }
    }

    Some(())
}

fn read_file_prefix_text(session_path: &Path, file_len: u64, max_bytes: usize) -> Option<String> {
    let mut file = File::open(session_path).ok()?;
    let read_len = file_len.min(max_bytes as u64);
    let mut bytes = Vec::with_capacity(read_len as usize);
    file.by_ref().take(read_len).read_to_end(&mut bytes).ok()?;

    if file_len > max_bytes as u64 && !bytes.ends_with(b"\n") {
        truncate_after_last_newline(&mut bytes);
    }

    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_file_suffix_text(session_path: &Path, file_len: u64, max_bytes: usize) -> Option<String> {
    let mut file = File::open(session_path).ok()?;
    let read_len = file_len.min(max_bytes as u64);
    let start_offset = file_len.saturating_sub(read_len);
    file.seek(SeekFrom::Start(start_offset)).ok()?;

    let mut bytes = Vec::with_capacity(read_len as usize);
    file.by_ref().take(read_len).read_to_end(&mut bytes).ok()?;

    if start_offset > 0 {
        drop_partial_first_line(&mut bytes);
    }

    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn truncate_after_last_newline(bytes: &mut Vec<u8>) {
    if let Some(newline_index) = bytes.iter().rposition(|byte_value| *byte_value == b'\n') {
        bytes.truncate(newline_index + 1);
    } else {
        bytes.clear();
    }
}

fn drop_partial_first_line(bytes: &mut Vec<u8>) {
    if let Some(newline_index) = bytes.iter().position(|byte_value| *byte_value == b'\n') {
        bytes.drain(..=newline_index);
    } else {
        bytes.clear();
    }
}

fn json_value_from_jsonl_line(line_text: &str) -> Option<Value> {
    let trimmed_line = line_text.trim();
    if trimmed_line.is_empty() {
        return None;
    }

    serde_json::from_str::<Value>(trimmed_line).ok()
}

fn tail_sample_covers_file(file_len: u64) -> bool {
    file_len <= JSONL_TAIL_SAMPLE_BYTES as u64
}

fn sampled_message_count(head_message_count: u32, tail_message_count: u32, tail_covers_file: bool) -> u32 {
    if tail_covers_file {
        tail_message_count
    } else {
        head_message_count.saturating_add(tail_message_count)
    }
}

fn is_copilot_message_event(json_value: &Value) -> bool {
    input_text_from_copilot_event(json_value).is_some() || json_kind(json_value) == Some(1)
}

fn is_claude_message_event(json_value: &Value) -> bool {
    matches!(string_at(json_value, &["/type"]), Some("user") | Some("assistant"))
}

fn claude_message_text(json_value: &Value) -> Option<&str> {
    if let Some(content_text) = string_at(json_value, &["/message/content", "/message/text", "/content", "/text"]) {
        return Some(content_text);
    }

    text_from_content_blocks(json_value.pointer("/message/content"))
        .or_else(|| text_from_content_blocks(json_value.pointer("/content")))
}

fn text_from_content_blocks(content_value: Option<&Value>) -> Option<&str> {
    content_value.and_then(Value::as_array).and_then(|content_blocks| {
        content_blocks.iter().find_map(|content_block| {
            string_at(content_block, &["/text"]).filter(|text_value| !text_value.trim().is_empty())
        })
    })
}

fn read_claude_index(
    index_path: &Path,
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
) -> Vec<AgentSession> {
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

    let status = status_from_activity(updated_ms, scan_time_ms, ActivitySource::FileModified);
    let workspace = workspace_path
        .as_deref()
        .map(Path::new)
        .and_then(display_name_from_path)
        .or_else(|| index_path.parent().and_then(display_name_from_path))
        .unwrap_or_else(|| "Claude".to_string());
    let title = string_at(entry_json, &["/aiTitle", "/customTitle", "/sessionTitle", "/title", "/slug"])
        .and_then(title_candidate_from_text)
        .or_else(|| string_at(entry_json, &["/firstPrompt"]).and_then(title_candidate_from_text))
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
        session_resource: Some(provider_session_resource("claude-code", &session_id)),
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

fn newest_timestamp_ms(current_timestamp_ms: Option<u64>, candidate_timestamp_ms: Option<u64>) -> Option<u64> {
    match (current_timestamp_ms, candidate_timestamp_ms) {
        (Some(current_timestamp_ms), Some(candidate_timestamp_ms)) => {
            Some(current_timestamp_ms.max(candidate_timestamp_ms))
        }
        (Some(current_timestamp_ms), None) => Some(current_timestamp_ms),
        (None, Some(candidate_timestamp_ms)) => Some(candidate_timestamp_ms),
        (None, None) => None,
    }
}

fn timestamp_ms_from_json(json_value: &Value) -> Option<u64> {
    [
        "/timestamp",
        "/createdAt",
        "/updatedAt",
        "/lastActivityAt",
        "/time",
        "/message/timestamp",
        "/message/createdAt",
        "/message/updatedAt",
        "/data/timestamp",
        "/data/createdAt",
        "/data/updatedAt",
        "/data/lastActivityAt",
        "/data/time",
        "/v/timestamp",
        "/v/createdAt",
        "/v/updatedAt",
        "/v/lastActivityAt",
        "/v/time",
    ]
    .iter()
    .find_map(|pointer| json_value.pointer(pointer).and_then(timestamp_ms_from_value))
}

fn copilot_alias_title_text(json_value: &Value) -> Option<&str> {
    if let Some(title_text) = string_at(
        json_value,
        &[
            "/v/customTitle",
            "/v/title",
            "/v/sessionTitle",
            "/v/generatedTitle",
            "/customTitle",
            "/title",
            "/sessionTitle",
            "/generatedTitle",
            "/data/v/customTitle",
            "/data/v/title",
            "/data/v/sessionTitle",
            "/data/v/generatedTitle",
        ],
    ) {
        return Some(title_text);
    }

    let key_path = json_value
        .get("k")
        .or_else(|| json_value.pointer("/data/k"))
        .and_then(Value::as_array)?;
    if !json_path_is_direct_title_key(key_path) {
        return None;
    }

    json_value
        .get("v")
        .or_else(|| json_value.pointer("/data/v"))
        .and_then(Value::as_str)
}

fn claude_alias_title_text(json_value: &Value) -> Option<&str> {
    string_at(
        json_value,
        &["/aiTitle", "/customTitle", "/sessionTitle", "/title"],
    )
}

fn copilot_title_text(json_value: &Value) -> Option<&str> {
    if let Some(input_text) = input_text_from_copilot_event(json_value) {
        return Some(input_text);
    }

    if let Some(message_text) = string_at(
        json_value,
        &[
            "/message/text",
            "/message/content",
            "/request/message/text",
            "/request/message/content",
            "/data/message/text",
            "/data/message/content",
            "/data/request/message/text",
            "/data/request/message/content",
            "/v/message/text",
            "/v/message/content",
            "/v/request/message/text",
            "/v/request/message/content",
        ],
    ) {
        return Some(message_text);
    }

    text_from_content_blocks(json_value.pointer("/message/content"))
        .or_else(|| text_from_content_blocks(json_value.pointer("/request/message/content")))
        .or_else(|| text_from_content_blocks(json_value.pointer("/data/message/content")))
        .or_else(|| text_from_content_blocks(json_value.pointer("/data/request/message/content")))
        .or_else(|| text_from_content_blocks(json_value.pointer("/v/message/content")))
        .or_else(|| text_from_content_blocks(json_value.pointer("/v/request/message/content")))
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

fn json_path_is_direct_title_key(key_path: &[Value]) -> bool {
    if key_path.len() != 1 {
        return false;
    }

    matches!(
        key_path[0].as_str(),
        Some("customTitle" | "title" | "sessionTitle" | "generatedTitle")
    )
}

fn json_kind(json_value: &Value) -> Option<i64> {
    json_value
        .get("kind")
        .or_else(|| json_value.pointer("/data/kind"))
        .and_then(Value::as_i64)
}

fn title_candidate_from_text(title_text: &str) -> Option<String> {
    let clean_title = clean_label(title_text, 80);
    if is_meaningful_title(&clean_title) {
        Some(clean_title)
    } else {
        None
    }
}

fn is_meaningful_title(title_text: &str) -> bool {
    let trimmed_title = title_text.trim();
    if trimmed_title.chars().count() < 4 {
        return false;
    }

    let meaningful_chars = trimmed_title
        .chars()
        .filter(|title_char| title_char.is_alphanumeric())
        .count();
    if meaningful_chars < 3 {
        return false;
    }

    !looks_like_identifier(trimmed_title)
}

fn looks_like_identifier(title_text: &str) -> bool {
    let mut compact_text = String::new();
    let mut separator_count = 0usize;
    let total_chars = title_text.chars().count();
    for title_char in title_text.chars() {
        if title_char.is_ascii_hexdigit() {
            compact_text.push(title_char);
        } else if matches!(title_char, '-' | '_' | ':' | '/') {
            separator_count = separator_count.saturating_add(1);
        } else {
            return false;
        }
    }

    compact_text.len() >= 8 && (separator_count > 0 || compact_text.len() == total_chars)
}

fn fallback_session_title(provider_name: &str, workspace_name: &str, session_id: &str) -> String {
    let workspace_title = clean_label(workspace_name, 60);
    if !workspace_title.is_empty() && workspace_title != "Unknown" && workspace_title != provider_name {
        format!("{} {}", provider_name, workspace_title)
    } else {
        format!("{} {}", provider_name, short_id(session_id))
    }
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

fn timestamp_ms_from_value(json_value: &Value) -> Option<u64> {
    millis_from_value(json_value).or_else(|| json_value.as_str().and_then(iso_timestamp_ms))
}

fn iso_timestamp_ms(timestamp_text: &str) -> Option<u64> {
    let trimmed_timestamp = timestamp_text.trim();
    let separator_index = trimmed_timestamp.find('T').or_else(|| trimmed_timestamp.find(' '))?;
    let date_text = &trimmed_timestamp[..separator_index];
    let time_text = &trimmed_timestamp[separator_index + 1..];

    if date_text.len() != 10 || time_text.len() < 8 {
        return None;
    }

    let year = parse_i32_slice(date_text, 0, 4)?;
    let month = parse_u32_slice(date_text, 5, 7)?;
    let day = parse_u32_slice(date_text, 8, 10)?;
    let hour = parse_u32_slice(time_text, 0, 2)?;
    let minute = parse_u32_slice(time_text, 3, 5)?;
    let second = parse_u32_slice(time_text, 6, 8)?;

    if date_text.as_bytes().get(4) != Some(&b'-')
        || date_text.as_bytes().get(7) != Some(&b'-')
        || time_text.as_bytes().get(2) != Some(&b':')
        || time_text.as_bytes().get(5) != Some(&b':')
        || hour > 23
        || minute > 59
        || second > 60
    {
        return None;
    }

    let remainder_text = &time_text[8..];
    let (fractional_ms, timezone_text) = split_fractional_ms(remainder_text)?;
    let offset_seconds = timezone_offset_seconds(timezone_text)?;
    let day_count = days_from_civil(year, month, day)?;
    let local_seconds = day_count
        .saturating_mul(86_400)
        .saturating_add(i64::from(hour) * 3_600)
        .saturating_add(i64::from(minute) * 60)
        .saturating_add(i64::from(second));
    let utc_seconds = local_seconds.saturating_sub(i64::from(offset_seconds));
    if utc_seconds < 0 {
        return None;
    }

    Some((utc_seconds as u64).saturating_mul(1000).saturating_add(fractional_ms))
}

fn split_fractional_ms(remainder_text: &str) -> Option<(u64, &str)> {
    let Some(fraction_text) = remainder_text.strip_prefix('.') else {
        return Some((0, remainder_text));
    };

    let digit_count = fraction_text
        .chars()
        .take_while(|fraction_char| fraction_char.is_ascii_digit())
        .count();
    if digit_count == 0 {
        return None;
    }

    let fraction_digits = &fraction_text[..digit_count];
    let timezone_text = &fraction_text[digit_count..];
    Some((fraction_to_millis(fraction_digits), timezone_text))
}

fn fraction_to_millis(fraction_digits: &str) -> u64 {
    let mut millis_text = String::new();
    for fraction_char in fraction_digits.chars().take(3) {
        millis_text.push(fraction_char);
    }
    while millis_text.len() < 3 {
        millis_text.push('0');
    }

    millis_text.parse::<u64>().unwrap_or(0)
}

fn timezone_offset_seconds(timezone_text: &str) -> Option<i32> {
    if timezone_text.is_empty() || timezone_text.eq_ignore_ascii_case("Z") {
        return Some(0);
    }

    let sign_multiplier = if timezone_text.starts_with('+') {
        1
    } else if timezone_text.starts_with('-') {
        -1
    } else {
        return None;
    };
    if timezone_text.len() != 6 || timezone_text.as_bytes().get(3) != Some(&b':') {
        return None;
    }

    let hour_offset = parse_i32_slice(timezone_text, 1, 3)?;
    let minute_offset = parse_i32_slice(timezone_text, 4, 6)?;
    if hour_offset > 23 || minute_offset > 59 {
        return None;
    }

    Some(sign_multiplier * (hour_offset * 3_600 + minute_offset * 60))
}

fn parse_i32_slice(text_value: &str, start_index: usize, end_index: usize) -> Option<i32> {
    text_value.get(start_index..end_index)?.parse::<i32>().ok()
}

fn parse_u32_slice(text_value: &str, start_index: usize, end_index: usize) -> Option<u32> {
    text_value.get(start_index..end_index)?.parse::<u32>().ok()
}

fn days_from_civil(year: i32, month: u32, day: u32) -> Option<i64> {
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }

    let adjusted_year = year - i32::from(month <= 2);
    let era = if adjusted_year >= 0 {
        adjusted_year
    } else {
        adjusted_year - 399
    } / 400;
    let year_of_era = adjusted_year - era * 400;
    let adjusted_month = month as i32 + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day as i32 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;

    Some(i64::from(era * 146_097 + day_of_era - 719_468))
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn status_from_activity(updated_ms: u64, scan_time_ms: u64, activity_source: ActivitySource) -> String {
    let age_ms = scan_time_ms.saturating_sub(updated_ms);
    if age_ms <= STATUS_RUNNING_WINDOW_MS {
        "running".to_string()
    } else if age_ms <= waiting_window_ms(activity_source) {
        "waiting".to_string()
    } else {
        "idle".to_string()
    }
}

fn waiting_window_ms(activity_source: ActivitySource) -> u64 {
    match activity_source {
        ActivitySource::ContentTimestamp => STATUS_WAITING_WINDOW_MS,
        ActivitySource::FileModified => STATUS_FILE_WAITING_WINDOW_MS,
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
        let mut code_command = Command::new(&candidate);
        code_command.args(arguments);

        match spawn_hidden(code_command) {
            Ok(_) => return Ok(()),
            Err(error) => last_error = Some(format!("{}: {}", candidate, error)),
        }
    }

    Err(last_error.unwrap_or_else(|| "Unable to launch VS Code CLI".to_string()))
}

fn spawn_hidden(mut command: Command) -> std::io::Result<Child> {
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command.spawn()
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