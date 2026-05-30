use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::{Emitter, Manager};

const DEFAULT_MAX_ACTIVE_SESSIONS: usize = 80;
const DEFAULT_ACTIVE_SESSION_DAYS: u64 = 7;
const COPILOT_TITLE_SAMPLE_LINES: usize = 240;
const CLAUDE_TITLE_SAMPLE_LINES: usize = 240;
const JSONL_HEAD_SAMPLE_BYTES: usize = 128 * 1024;
const JSONL_TAIL_SAMPLE_BYTES: usize = 256 * 1024;
const COPILOT_PROMPT_SCAN_BYTES: usize = 16 * 1024 * 1024;
const COPILOT_TRANSCRIPT_SCAN_BYTES: usize = 2 * 1024 * 1024;
const CLAUDE_PROMPT_SCAN_BYTES: usize = 8 * 1024 * 1024;
const STATUS_RUNNING_HINT_WINDOW_MS: u64 = 2 * 60 * 60_000;
const STATUS_RECENT_CONTENT_WINDOW_MS: u64 = 10 * 60_000;
const STATUS_RECENT_FILE_WINDOW_MS: u64 = 3 * 60_000;
const COPILOT_UNSTARTED_ASK_WAITING_WINDOW_MS: u64 = 24 * 60 * 60_000;
const SESSION_PREVIEW_SOURCE_MAX_CHARS: usize = 64 * 1024;
const USER_PREVIEW_MAX_CHARS: usize = 400;
const AI_PREVIEW_MAX_CHARS: usize = 800;
const SESSIONS_CHANGED_EVENT: &str = "agentwatcher-sessions-changed";
const BRIDGE_EXTENSION_FOLDER_PREFIX: &str = "agentwatcher.agentwatcher-bridge-";
const BRIDGE_EXTENSION_URI_AUTHORITY: &str = "agentwatcher.agentwatcher-bridge";
const RUNTIME_ICON_SIZE: u32 = 256;
const RUNTIME_ICON_RGBA: &[u8] = include_bytes!("../icons/icon-runtime-256.rgba");
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
    last_user_message: Option<String>,
    last_user_message_truncated: bool,
    last_ai_message: Option<String>,
    last_ai_message_truncated: bool,
    last_ai_message_excerpt_kind: Option<String>,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BridgeStatus {
    installed: bool,
    needs_update: bool,
    local_version: String,
    installed_version: Option<String>,
    code_path: Option<String>,
    message: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionStatusHint {
    Running,
    Waiting,
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
    status_hint: Option<SessionStatusHint>,
    latest_timestamp_ms: Option<u64>,
    message_count: u32,
    last_user_message: Option<String>,
    last_user_message_truncated: bool,
    last_ai_message: Option<String>,
    last_ai_message_truncated: bool,
    last_ai_message_excerpt_kind: Option<String>,
    archived: bool,
}

#[derive(Debug, Clone, Default)]
struct CopilotTranscriptOverlay {
    status_hint: Option<SessionStatusHint>,
    latest_timestamp_ms: Option<u64>,
    unstarted_ask_timestamp_ms: Option<u64>,
    last_user_message: Option<String>,
    last_ai_message: Option<String>,
}

#[derive(Debug, Clone)]
struct ClaudeSessionSummary {
    title: Option<String>,
    workspace_path: Option<String>,
    branch: Option<String>,
    status_hint: Option<SessionStatusHint>,
    latest_timestamp_ms: Option<u64>,
    message_count: u32,
    last_user_message: Option<String>,
    last_user_message_truncated: bool,
    last_ai_message: Option<String>,
    last_ai_message_truncated: bool,
    last_ai_message_excerpt_kind: Option<String>,
    archived: bool,
}

static COPILOT_SESSION_CACHE: OnceLock<Mutex<HashMap<String, CachedJsonlSummary<CopilotSessionSummary>>>> =
    OnceLock::new();
static COPILOT_TRANSCRIPT_CACHE: OnceLock<Mutex<HashMap<String, CachedJsonlSummary<CopilotTranscriptOverlay>>>> =
    OnceLock::new();
static CLAUDE_SESSION_CACHE: OnceLock<Mutex<HashMap<String, CachedJsonlSummary<ClaudeSessionSummary>>>> =
    OnceLock::new();

fn start_session_file_watcher(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        let watch_roots = session_watch_roots();
        if watch_roots.is_empty() {
            return;
        }

        let (event_sender, event_receiver) = mpsc::channel();
        let mut watcher = match RecommendedWatcher::new(
            move |result| {
                let _ = event_sender.send(result);
            },
            Config::default(),
        ) {
            Ok(watcher) => watcher,
            Err(_) => return,
        };

        for root in watch_roots {
            let _ = watcher.watch(&root, RecursiveMode::Recursive);
        }

        let mut pending_emit = false;
        let mut last_emit_ms = 0;
        loop {
            match event_receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(Ok(event)) if is_session_file_event(&event) => {
                    let now_ms = current_time_ms();
                    if now_ms.saturating_sub(last_emit_ms) >= 250 {
                        emit_sessions_changed(&app_handle, now_ms);
                        last_emit_ms = now_ms;
                        pending_emit = false;
                    } else {
                        pending_emit = true;
                    }
                }
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if pending_emit {
                        let now_ms = current_time_ms();
                        emit_sessions_changed(&app_handle, now_ms);
                        last_emit_ms = now_ms;
                        pending_emit = false;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

fn session_watch_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(appdata_path) = env::var_os("APPDATA") {
        let appdata_path = PathBuf::from(appdata_path);
        for product_folder in ["Code", "Code - Insiders"] {
            push_existing_path(
                &mut roots,
                appdata_path
                    .join(product_folder)
                    .join("User")
                    .join("workspaceStorage"),
            );
        }
    }
    if let Some(user_profile_path) = env::var_os("USERPROFILE") {
        push_existing_path(
            &mut roots,
            PathBuf::from(user_profile_path).join(".claude").join("projects"),
        );
    }
    roots
}

fn push_existing_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if path.exists() && !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn is_session_file_event(event: &Event) -> bool {
    if matches!(event.kind, EventKind::Access(_)) {
        return false;
    }

    event.paths.iter().any(|path| {
        has_extension(path, "jsonl")
            || path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .map(|file_name| file_name.eq_ignore_ascii_case("sessions-index.json"))
                .unwrap_or(false)
    })
}

fn emit_sessions_changed(app_handle: &tauri::AppHandle, emitted_ms: u64) {
    let _ = app_handle.emit(SESSIONS_CHANGED_EVENT, emitted_ms);
}

#[tauri::command]
fn get_bridge_status() -> BridgeStatus {
    let local_version = get_local_bridge_version();
    let installed_version = get_installed_bridge_version();
    let code_path = find_code_cli_path();

    let installed = bridge_extension_installed();
    let needs_update = if let (Some(local), Some(installed)) = (parse_version(&local_version), parse_version(&installed_version.clone().unwrap_or_default())) {
        local > installed
    } else {
        false
    };

    let message = if !installed {
        "Bridge extension not installed".to_string()
    } else if needs_update {
        "Update available".to_string()
    } else {
        "Bridge installed and up to date".to_string()
    };

    BridgeStatus {
        installed,
        needs_update,
        local_version,
        installed_version,
        code_path,
        message,
    }
}

#[tauri::command]
fn install_bridge() -> Result<String, String> {
    let vscode_bridge_dir = find_bridge_source_dir()
        .ok_or_else(|| "Bridge source directory not found".to_string())?;
    let local_version = get_local_bridge_version();
    let vsix_path = find_bridge_vsix(&vscode_bridge_dir, &local_version)
        .map(Ok)
        .unwrap_or_else(|| package_bridge_vsix(&vscode_bridge_dir, &local_version))?;

    let code_path = find_code_cli_path().ok_or("VS Code CLI not found")?;
    let mut install_cmd = Command::new(&code_path);
    install_cmd
        .args(["--install-extension"])
        .arg(&vsix_path)
        .arg("--force");

    let install_output = spawn_hidden_with_output(install_cmd)
        .map_err(|e| format!("Failed to install bridge extension: {}", e))?;

    if !install_output.status.success() {
        return Err(format!(
            "Bridge installation failed: {}",
            String::from_utf8_lossy(&install_output.stderr)
        ));
    }

    Ok("Bridge extension installed successfully".to_string())
}

#[tauri::command]
fn set_window_always_on_top(window: tauri::WebviewWindow, always_on_top: bool) -> Result<(), String> {
    window
        .set_always_on_top(always_on_top)
        .map_err(|error| error.to_string())?;

    #[cfg(target_os = "windows")]
    apply_native_always_on_top(&window, always_on_top);

    Ok(())
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

    let mut candidates: Vec<(PathBuf, WorkspaceInfo, u64)> = Vec::new();
    for storage_entry in read_directory(&storage_root) {
        let storage_path = storage_entry.path();
        if !storage_path.is_dir() {
            continue;
        }

        let workspace_info = read_workspace_info(&storage_path.join("workspace.json"));
        let chat_sessions_path = storage_path.join("chatSessions");
        for session_entry in read_directory(&chat_sessions_path) {
            let session_path = session_entry.path();
            if !has_extension(&session_path, "jsonl") {
                continue;
            }
            let modified_ms = dir_entry_modified_ms(&session_entry);
            if !is_active_session(fallback_updated_ms_from_modified(modified_ms, scan_time_ms), scan_time_ms, options) {
                continue;
            }
            candidates.push((session_path, workspace_info.clone(), modified_ms));
        }
    }

    candidates.sort_by(|left, right| right.2.cmp(&left.2));
    candidates
        .into_iter()
        .take(candidate_scan_limit(options))
        .filter_map(|(session_path, workspace_info, _)| {
            read_copilot_session(&session_path, &workspace_info, scan_time_ms, options)
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
        mut status_hint,
        mut latest_timestamp_ms,
        message_count,
        mut last_user_message,
        mut last_user_message_truncated,
        mut last_ai_message,
        mut last_ai_message_truncated,
        mut last_ai_message_excerpt_kind,
        archived,
    } = summary;
    if options.hide_archived && archived {
        return None;
    }

    if let Some(overlay) = read_copilot_transcript_overlay(session_path) {
        latest_timestamp_ms = newest_timestamp_ms(latest_timestamp_ms, overlay.latest_timestamp_ms);
        let overlay_status_hint = if overlay.status_hint == Some(SessionStatusHint::Waiting)
            || copilot_unstarted_ask_is_recent(&overlay, scan_time_ms)
        {
            Some(SessionStatusHint::Waiting)
        } else {
            overlay.status_hint
        };
        if overlay_status_hint.is_some() {
            status_hint = overlay_status_hint;
        }
        if overlay.last_user_message.is_some() {
            let (message, truncated) = apply_user_preview_budget(overlay.last_user_message);
            last_user_message = message;
            last_user_message_truncated = truncated;
        }
        if overlay.last_ai_message.is_some() {
            let (message, truncated, excerpt_kind) = apply_ai_preview_budget(overlay.last_ai_message);
            last_ai_message = message;
            last_ai_message_truncated = truncated;
            last_ai_message_excerpt_kind = excerpt_kind;
        }
    }

    let (updated_ms, activity_source) = latest_activity_ms(latest_timestamp_ms, fallback_updated_ms);
    if !is_active_session(updated_ms, scan_time_ms, options) {
        return None;
    }

    let status = status_from_activity(updated_ms, scan_time_ms, activity_source, status_hint);
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
        last_user_message,
        last_user_message_truncated,
        last_ai_message,
        last_ai_message_truncated,
        last_ai_message_excerpt_kind,
        branch: None,
    })
}

fn scan_claude_sessions(scan_time_ms: u64, options: &ResolvedScanOptions) -> Vec<AgentSession> {
    let Some(user_profile_path) = env::var_os("USERPROFILE") else {
        return Vec::new();
    };

    let projects_root = PathBuf::from(user_profile_path).join(".claude").join("projects");

    let mut project_paths = Vec::new();
    let mut candidates: Vec<(PathBuf, u64)> = Vec::new();
    for project_entry in read_directory(&projects_root) {
        let project_path = project_entry.path();
        if !project_path.is_dir() {
            continue;
        }
        project_paths.push(project_path.clone());
        for session_entry in read_directory(&project_path) {
            let session_path = session_entry.path();
            if !has_extension(&session_path, "jsonl") {
                continue;
            }
            let modified_ms = dir_entry_modified_ms(&session_entry);
            if !is_active_session(fallback_updated_ms_from_modified(modified_ms, scan_time_ms), scan_time_ms, options) {
                continue;
            }
            candidates.push((session_path, modified_ms));
        }
    }

    candidates.sort_by(|left, right| right.1.cmp(&left.1));
    let mut sessions: Vec<AgentSession> = candidates
        .into_iter()
        .take(candidate_scan_limit(options))
        .filter_map(|(session_path, _)| read_claude_jsonl_session(&session_path, scan_time_ms, options))
        .collect();

    let mut known_session_paths: HashSet<String> = sessions
        .iter()
        .filter_map(|session| session.session_path.clone())
        .collect();
    for project_path in project_paths {
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
        status_hint,
        latest_timestamp_ms,
        message_count,
        last_user_message,
        last_user_message_truncated,
        last_ai_message,
        last_ai_message_truncated,
        last_ai_message_excerpt_kind,
        archived,
    } = summary;
    if options.hide_archived && archived {
        return None;
    }

    let (updated_ms, activity_source) = latest_activity_ms(latest_timestamp_ms, fallback_updated_ms);
    if !is_active_session(updated_ms, scan_time_ms, options) {
        return None;
    }

    let status = status_from_activity(updated_ms, scan_time_ms, activity_source, status_hint);
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
        last_user_message,
        last_user_message_truncated,
        last_ai_message,
        last_ai_message_truncated,
        last_ai_message_excerpt_kind,
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
    let mut status_hint = None;
    let mut archived = false;
    let mut latest_timestamp_ms = None;
    let mut last_user_message = None;
    let mut last_ai_message = None;
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
        let next_user = copilot_interactive_user_text(json_value)
            .or_else(|| copilot_request_user_text(json_value));
        if let Some(next_user_message) = next_user {
            if !is_user_system_error_text(&next_user_message) {
                last_user_message = Some(next_user_message);
            }
        }
        if let Some(next_ai_message) = copilot_response_preview_text(json_value) {
            if !is_ai_model_noise_text(&next_ai_message) {
                last_ai_message = Some(next_ai_message);
            }
        }
        update_copilot_status_hint(&mut status_hint, json_value);
        if let Some(next_title) = copilot_alias_title_text(json_value).and_then(title_candidate_from_text) {
            alias_title = Some(next_title);
        }
    });

    if last_user_message.is_none() {
        last_user_message = read_latest_copilot_user_message(session_path, fingerprint.len);
    }
    if last_ai_message.is_none() {
        last_ai_message = read_latest_copilot_ai_message(session_path, fingerprint.len);
    }

    let (last_user_message, last_user_message_truncated) = apply_user_preview_budget(last_user_message);
    let (last_ai_message, last_ai_message_truncated, last_ai_message_excerpt_kind) = apply_ai_preview_budget(last_ai_message);
    Some(CopilotSessionSummary {
        session_id,
        title: alias_title.or(prompt_title),
        status_hint,
        latest_timestamp_ms,
        message_count: sampled_message_count(head_message_count, tail_message_count, tail_covers_file),
        last_user_message,
        last_user_message_truncated,
        last_ai_message,
        last_ai_message_truncated,
        last_ai_message_excerpt_kind,
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
    let mut status_hint = None;
    let mut first_user_message = None;
    let mut last_user_message = None;
    let mut last_ai_message = None;
    let mut pending_ask_user_question_ids = HashSet::new();
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
        if event_type == Some("user") {
            if let Some(clean_text) = claude_clean_user_text(json_value) {
                if prompt_title.is_none() {
                    prompt_title = title_candidate_from_text(&clean_text);
                }
                if first_user_message.is_none() {
                    first_user_message = Some(clean_text);
                }
            }
        } else if event_type == Some("last-prompt") {
            if let Some(lp_text) = string_at(json_value, &["/lastPrompt"]) {
                if prompt_title.is_none() {
                    prompt_title = title_candidate_from_text(lp_text);
                }
            }
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
        match string_at(json_value, &["/type"]) {
            Some("user") => {
                let next = claude_interactive_user_text(json_value)
                    .or_else(|| claude_clean_user_text(json_value));
                if let Some(next_user_message) = next {
                    if !is_user_system_error_text(&next_user_message) {
                        last_user_message = Some(next_user_message);
                    }
                }
            }
            Some("assistant") => {
                if let Some(next_ai_message) = claude_assistant_text(json_value) {
                    if !is_ai_model_noise_text(&next_ai_message) {
                        last_ai_message = Some(next_ai_message);
                    }
                }
            }
            Some("last-prompt") => {
                // Only used for title/fallback; do not overwrite real user messages here.
            }
            _ => {}
        }
        update_claude_status_hint(&mut status_hint, &mut pending_ask_user_question_ids, json_value);
        if let Some(next_title) = claude_alias_title_text(json_value).and_then(title_candidate_from_text) {
            alias_title = Some(next_title);
        } else if slug_title.is_none() {
            slug_title = string_at(json_value, &["/slug"]).and_then(title_candidate_from_text);
        }
    });

    if last_user_message.is_none() {
        last_user_message = read_latest_claude_user_message(session_path, fingerprint.len);
    }
    if last_ai_message.is_none() {
        last_ai_message = read_latest_claude_ai_message(session_path, fingerprint.len);
    }

    let (last_user_message, last_user_message_truncated) = apply_user_preview_budget(last_user_message.or(first_user_message));
    let (last_ai_message, last_ai_message_truncated, last_ai_message_excerpt_kind) = apply_ai_preview_budget(last_ai_message);
    Some(ClaudeSessionSummary {
        title: alias_title.or(slug_title).or(prompt_title),
        workspace_path,
        branch,
        status_hint,
        latest_timestamp_ms,
        message_count: sampled_message_count(head_message_count, tail_message_count, tail_covers_file),
        last_user_message,
        last_user_message_truncated,
        last_ai_message,
        last_ai_message_truncated,
        last_ai_message_excerpt_kind,
        archived,
    })
}

fn read_latest_copilot_user_message(session_path: &Path, file_len: u64) -> Option<String> {
    let text = read_file_suffix_text(session_path, file_len, COPILOT_PROMPT_SCAN_BYTES)?;
    for line_text in text.lines().rev() {
        if let Some(json_value) = json_value_from_jsonl_line(line_text) {
            if let Some(msg) = copilot_interactive_user_text(&json_value) {
                if !is_user_system_error_text(&msg) {
                    return Some(msg);
                }
                continue;
            }
            if let Some(user_message) = copilot_request_user_text(&json_value) {
                if !is_user_system_error_text(&user_message) {
                    return Some(user_message);
                }
            }
        }
    }
    None
}

fn read_latest_copilot_ai_message(session_path: &Path, file_len: u64) -> Option<String> {
    let text = read_file_suffix_text(session_path, file_len, COPILOT_PROMPT_SCAN_BYTES)?;
    for line_text in text.lines().rev() {
        if let Some(json_value) = json_value_from_jsonl_line(line_text) {
            if let Some(ai_message) = copilot_response_preview_text(&json_value) {
                if !is_ai_model_noise_text(&ai_message) {
                    return Some(ai_message);
                }
            }
        }
    }
    None
}

fn read_copilot_transcript_overlay(session_path: &Path) -> Option<CopilotTranscriptOverlay> {
    let transcript_path = copilot_transcript_path(session_path)?;
    let fingerprint = jsonl_fingerprint(&transcript_path)?;
    let path_key = path_to_string(&transcript_path);
    if let Some(overlay) = cached_jsonl_summary(&COPILOT_TRANSCRIPT_CACHE, &path_key, &fingerprint) {
        return Some(overlay);
    }
    let text = read_file_suffix_text(&transcript_path, fingerprint.len, COPILOT_TRANSCRIPT_SCAN_BYTES)?;
    let overlay = copilot_transcript_overlay_from_text(&text)?;
    store_jsonl_summary(&COPILOT_TRANSCRIPT_CACHE, path_key, &fingerprint, overlay.clone());
    Some(overlay)
}

fn copilot_unstarted_ask_is_recent(overlay: &CopilotTranscriptOverlay, scan_time_ms: u64) -> bool {
    overlay
        .unstarted_ask_timestamp_ms
        .map(|timestamp_ms| scan_time_ms.saturating_sub(timestamp_ms) <= COPILOT_UNSTARTED_ASK_WAITING_WINDOW_MS)
        .unwrap_or(false)
}

fn copilot_transcript_path(session_path: &Path) -> Option<PathBuf> {
    let session_id = file_stem(session_path)?;
    let storage_path = session_path.parent()?.parent()?;
    Some(
        storage_path
            .join("GitHub.copilot-chat")
            .join("transcripts")
            .join(format!("{}.jsonl", session_id)),
    )
}

fn copilot_transcript_overlay_from_text(text: &str) -> Option<CopilotTranscriptOverlay> {
    let mut overlay = CopilotTranscriptOverlay::default();
    let mut started_ask_tool_ids: HashSet<String> = HashSet::new();
    let mut unstarted_ask_tool_ids: HashSet<String> = HashSet::new();
    let mut unstarted_ask_timestamp_ms: Option<u64> = None;
    let mut saw_activity = false;

    for line_text in text.lines() {
        let Some(json_value) = json_value_from_jsonl_line(line_text) else { continue; };
        overlay.latest_timestamp_ms = newest_timestamp_ms(overlay.latest_timestamp_ms, timestamp_ms_from_json(&json_value));

        match string_at(&json_value, &["/type"]) {
            Some("user.message") => {
                started_ask_tool_ids.clear();
                unstarted_ask_tool_ids.clear();
                unstarted_ask_timestamp_ms = None;
                if let Some(content) = string_at(&json_value, &["/data/content"]).and_then(clean_preview_text) {
                    overlay.last_user_message = Some(content);
                    saw_activity = true;
                }
            }
            Some("assistant.message") => {
                started_ask_tool_ids.clear();
                unstarted_ask_tool_ids.clear();
                unstarted_ask_timestamp_ms = None;
                let mut parts = Vec::new();
                if let Some(content) = string_at(&json_value, &["/data/content"]).and_then(clean_preview_text) {
                    parts.push(content);
                }
                if collect_copilot_transcript_ask_requests(
                    json_value.pointer("/data/toolRequests"),
                    &mut unstarted_ask_tool_ids,
                    &mut parts,
                ) {
                    unstarted_ask_timestamp_ms = timestamp_ms_from_json(&json_value);
                }
                if let Some(message) = clean_preview_text(&parts.join("\n")) {
                    overlay.last_ai_message = Some(message);
                    saw_activity = true;
                }
            }
            Some("tool.execution_start") => {
                if copilot_transcript_tool_name(&json_value) == Some("vscode_askQuestions") {
                    if let Some(tool_call_id) = string_at(&json_value, &["/data/toolCallId"]) {
                        unstarted_ask_tool_ids.remove(tool_call_id);
                        started_ask_tool_ids.insert(tool_call_id.to_string());
                    }
                    if let Some(message) = question_tool_preview_text(json_value.pointer("/data/arguments").unwrap_or(&json_value)) {
                        overlay.last_ai_message = Some(message);
                    }
                }
                saw_activity = true;
            }
            Some("tool.execution_complete") => {
                if let Some(tool_call_id) = string_at(&json_value, &["/data/toolCallId"]) {
                    started_ask_tool_ids.remove(tool_call_id);
                    unstarted_ask_tool_ids.remove(tool_call_id);
                }
                saw_activity = true;
            }
            Some("assistant.message_delta") | Some("assistant.turn_end") | Some("assistant.turn_start") => {
                saw_activity = true;
            }
            _ => {}
        }
    }

    overlay.status_hint = if started_ask_tool_ids.is_empty() {
        saw_activity.then_some(SessionStatusHint::Running)
    } else {
        Some(SessionStatusHint::Waiting)
    };
    if !unstarted_ask_tool_ids.is_empty() {
        overlay.unstarted_ask_timestamp_ms = unstarted_ask_timestamp_ms;
    }

    if overlay.latest_timestamp_ms.is_some()
        || overlay.last_user_message.is_some()
        || overlay.last_ai_message.is_some()
        || overlay.status_hint.is_some()
    {
        Some(overlay)
    } else {
        None
    }
}

fn collect_copilot_transcript_ask_requests(
    tool_requests: Option<&Value>,
    unstarted_ask_tool_ids: &mut HashSet<String>,
    parts: &mut Vec<String>,
) -> bool {
    let Some(tool_requests) = tool_requests.and_then(Value::as_array) else { return false; };
    let mut found_ask_request = false;
    for request in tool_requests {
        if string_at(request, &["/name"]) != Some("vscode_askQuestions") {
            continue;
        }
        found_ask_request = true;
        if let Some(tool_call_id) = string_at(request, &["/toolCallId"]) {
            unstarted_ask_tool_ids.insert(tool_call_id.to_string());
        }
        if let Some(message) = question_tool_preview_text(request.pointer("/arguments").unwrap_or(request)) {
            parts.push(message);
        }
    }
    found_ask_request
}

fn copilot_transcript_tool_name(json_value: &Value) -> Option<&str> {
    string_at(json_value, &["/data/toolName"])
}

fn read_latest_claude_user_message(session_path: &Path, file_len: u64) -> Option<String> {
    let text = read_file_suffix_text(session_path, file_len, CLAUDE_PROMPT_SCAN_BYTES)?;
    let mut last_prompt_fallback: Option<String> = None;
    for line_text in text.lines().rev() {
        if let Some(json_value) = json_value_from_jsonl_line(line_text) {
            match string_at(&json_value, &["/type"]) {
                Some("last-prompt") => {
                    if last_prompt_fallback.is_none() {
                        if let Some(lp_text) = string_at(&json_value, &["/lastPrompt"]) {
                            last_prompt_fallback = clean_preview_text(lp_text);
                        }
                    }
                }
                Some("user") => {
                    let found = claude_interactive_user_text(&json_value)
                        .or_else(|| claude_clean_user_text(&json_value));
                    if let Some(clean) = found {
                        if !is_user_system_error_text(&clean) {
                            return Some(clean);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    last_prompt_fallback
}

fn read_latest_claude_ai_message(session_path: &Path, file_len: u64) -> Option<String> {
    let text = read_file_suffix_text(session_path, file_len, CLAUDE_PROMPT_SCAN_BYTES)?;
    for line_text in text.lines().rev() {
        if let Some(json_value) = json_value_from_jsonl_line(line_text) {
            if string_at(&json_value, &["/type"]) != Some("assistant") {
                continue;
            }
            if let Some(ai_message) = claude_assistant_text(&json_value) {
                if !is_ai_model_noise_text(&ai_message) {
                    return Some(ai_message);
                }
            }
        }
    }
    None
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
    fallback_updated_ms_from_modified(fingerprint.modified_ms, scan_time_ms)
}

fn fallback_updated_ms_from_modified(modified_ms: u64, scan_time_ms: u64) -> u64 {
    if modified_ms == 0 {
        scan_time_ms
    } else {
        modified_ms
    }
}

fn dir_entry_modified_ms(entry: &fs::DirEntry) -> u64 {
    entry
        .metadata()
        .ok()
        .and_then(|metadata| metadata.modified().ok())
        .and_then(system_time_ms)
        .unwrap_or(0)
}

fn candidate_scan_limit(options: &ResolvedScanOptions) -> usize {
    options.max_sessions.saturating_mul(4).max(options.max_sessions + 20).max(120)
}

fn latest_activity_ms(latest_timestamp_ms: Option<u64>, fallback_updated_ms: u64) -> (u64, ActivitySource) {
    match latest_timestamp_ms {
        Some(timestamp_ms) if timestamp_ms >= fallback_updated_ms => {
            (timestamp_ms, ActivitySource::ContentTimestamp)
        }
        _ => (fallback_updated_ms, ActivitySource::FileModified),
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

fn update_copilot_status_hint(status_hint: &mut Option<SessionStatusHint>, json_value: &Value) {
    if let Some(response_hint) = copilot_response_status_hint(json_value) {
        *status_hint = Some(response_hint);
    } else if copilot_request_finished_status_hint(json_value) {
        *status_hint = Some(SessionStatusHint::Running);
    } else if input_text_from_copilot_event(json_value).is_some() {
        *status_hint = Some(SessionStatusHint::Running);
    }
}

fn copilot_request_finished_status_hint(json_value: &Value) -> bool {
    if key_path_ends_with(json_value, &["result"])
        || key_path_ends_with(json_value, &["followups"])
        || key_path_ends_with(json_value, &["elapsedMs"])
    {
        return true;
    }

    if !key_path_ends_with(json_value, &["modelState"]) {
        return false;
    }

    json_value
        .get("v")
        .or_else(|| json_value.pointer("/data/v"))
        .and_then(|model_state| model_state.pointer("/completedAt"))
        .is_some()
}

fn copilot_response_status_hint(json_value: &Value) -> Option<SessionStatusHint> {
    if !key_path_ends_with(json_value, &["response"]) {
        return None;
    }

    let response_items = json_value
        .get("v")
        .or_else(|| json_value.pointer("/data/v"))
        .and_then(Value::as_array)?;
    let mut saw_activity = false;
    let mut has_waiting_carousel = false;
    let mut has_unresolved_ask_tool = false;
    let mut has_resolved_question = false;
    for response_item in response_items {
        let Some(kind) = string_at(response_item, &["/kind"]) else {
            continue;
        };
        if kind == "hook" {
            continue;
        }

        if kind == "questionCarousel" {
            if copilot_question_carousel_is_resolved(response_item) {
                saw_activity = true;
                has_resolved_question = true;
            } else {
                has_waiting_carousel = true;
            }
        } else if kind == "toolInvocationSerialized"
            && string_at(response_item, &["/toolId"]) == Some("vscode_askQuestions")
            && !bool_at(response_item, &["/isComplete"]).unwrap_or(false)
        {
            if is_skipped_json(response_item) {
                saw_activity = true;
                has_resolved_question = true;
            } else {
                has_unresolved_ask_tool = true;
            }
        } else {
            saw_activity = true;
        }
    }

    if has_waiting_carousel || (has_unresolved_ask_tool && !has_resolved_question) {
        Some(SessionStatusHint::Waiting)
    } else if saw_activity || has_resolved_question {
        Some(SessionStatusHint::Running)
    } else {
        None
    }
}

fn copilot_question_carousel_is_resolved(response_item: &Value) -> bool {
    bool_at(response_item, &["/isUsed"]) == Some(true)
        || is_skipped_json(response_item)
        || response_item.get("data").is_some()
}

fn copilot_response_preview_text(json_value: &Value) -> Option<String> {
    if key_path_ends_with(json_value, &["response"]) {
        let response_items = json_value
            .get("v")
            .or_else(|| json_value.pointer("/data/v"))
            .and_then(Value::as_array)?;
        return copilot_visible_response_text(response_items.iter());
    }

    let snapshot_requests = json_value.pointer("/v/requests")?.as_array()?;
    for request in snapshot_requests.iter().rev() {
        if let Some(response_items) = request.pointer("/response").and_then(Value::as_array) {
            if let Some(text) = copilot_visible_response_text(response_items.iter()) {
                return Some(text);
            }
        }
    }

    None
}

fn copilot_visible_response_text<'a>(response_items: impl Iterator<Item = &'a Value>) -> Option<String> {
    let mut text_parts = Vec::new();

    for response_item in response_items {
        if text_parts
            .iter()
            .map(|text_part: &String| text_part.chars().count())
            .sum::<usize>()
            >= SESSION_PREVIEW_SOURCE_MAX_CHARS
        {
            break;
        }

        if let Some(text) = copilot_visible_response_item_text(response_item) {
            text_parts.push(text);
        }
    }

    clean_preview_text(&text_parts.join("\n"))
}

fn copilot_visible_response_item_text(response_item: &Value) -> Option<String> {
    match string_at(response_item, &["/kind"]) {
        None | Some("") => copilot_visible_markdown_value(response_item),
        Some("text") | Some("markdownContent") | Some("content") | Some("message") => {
            copilot_direct_response_text(response_item)
        }
        Some("toolInvocationSerialized") if is_copilot_question_tool_id(response_item) => {
            question_tool_preview_text(response_item)
        }
        Some("questionCarousel") => question_tool_preview_text(response_item),
        _ => None,
    }
}

fn is_copilot_question_tool_id(response_item: &Value) -> bool {
    matches!(string_at(response_item, &["/toolId"]), Some("vscode_askQuestions"))
}

fn copilot_direct_response_text(response_item: &Value) -> Option<String> {
    for path in ["/value", "/text", "/markdown", "/content", "/message"] {
        if let Some(text) = string_at(response_item, &[path]) {
            return clean_copilot_visible_reply_text(text);
        }
    }
    None
}

fn copilot_visible_markdown_value(response_item: &Value) -> Option<String> {
    let has_markdown_shape = response_item.get("value").is_some()
        && (response_item.get("supportThemeIcons").is_some()
            || response_item.get("supportHtml").is_some()
            || response_item.get("baseUri").is_some()
            || response_item.get("isTrusted").is_some());
    if !has_markdown_shape {
        return None;
    }
    let text = string_at(response_item, &["/value"])?;
    clean_copilot_visible_reply_text(text)
}

fn clean_copilot_visible_reply_text(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() || is_pure_markdown_fence(trimmed) || is_ai_model_noise_text(trimmed) {
        return None;
    }
    clean_preview_text(trimmed)
}

fn question_tool_preview_text(value: &Value) -> Option<String> {
    let mut parts = Vec::new();
    collect_question_tool_preview_parts(value, &mut parts);
    let mut deduped_parts = Vec::new();
    for part in parts {
        if !deduped_parts.iter().any(|existing| existing == &part) {
            deduped_parts.push(part);
        }
    }
    clean_preview_text(&deduped_parts.join("\n"))
}

fn collect_question_tool_preview_parts(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            collect_question_tool_preview_parts_from_str(text, parts);
        }
        Value::Array(items) => {
            for item in items {
                collect_question_tool_preview_parts(item, parts);
            }
        }
        Value::Object(object) => {
            append_question_field(object, "header", parts);
            append_question_field(object, "title", parts);
            append_question_field(object, "question", parts);
            append_question_field(object, "prompt", parts);
            append_question_field(object, "message", parts);
            append_question_options(object.get("options").or_else(|| object.get("choices")), parts);

            for key in [
                "questions",
                "input",
                "args",
                "arguments",
                "toolInput",
                "tool_input",
                "toolSpecificData",
                "data",
                "parameters",
                "params",
            ] {
                if let Some(child) = object.get(key) {
                    collect_question_tool_preview_parts(child, parts);
                }
            }
        }
        _ => {}
    }
}

fn collect_question_tool_preview_parts_from_str(text: &str, parts: &mut Vec<String>) {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
        collect_question_tool_preview_parts(&parsed, parts);
    }
}

fn append_question_field(object: &serde_json::Map<String, Value>, key: &str, parts: &mut Vec<String>) {
    if let Some(text) = object.get(key).and_then(Value::as_str) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }
}

fn append_question_options(options: Option<&Value>, parts: &mut Vec<String>) {
    let Some(options) = options else { return; };
    let mut option_parts = Vec::new();
    match options {
        Value::Array(items) => {
            for item in items {
                if let Some(option_text) = question_option_text(item) {
                    option_parts.push(option_text);
                }
            }
        }
        Value::Object(_) | Value::String(_) => {
            if let Some(option_text) = question_option_text(options) {
                option_parts.push(option_text);
            }
        }
        _ => {}
    }
    if !option_parts.is_empty() {
        parts.push(format!("Options: {}", option_parts.join("; ")));
    }
}

fn question_option_text(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Object(object) => {
            let label = object
                .get("label")
                .or_else(|| object.get("value"))
                .or_else(|| object.get("text"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty());
            let description = object
                .get("description")
                .or_else(|| object.get("detail"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|text| !text.is_empty());
            match (label, description) {
                (Some(label), Some(description)) => Some(format!("{} - {}", label, description)),
                (Some(label), None) => Some(label.to_string()),
                (None, Some(description)) => Some(description.to_string()),
                _ => None,
            }
        }
        _ => None,
    }
}

fn is_pure_markdown_fence(text: &str) -> bool {
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    !lines.is_empty() && lines.len() <= 2 && lines.iter().all(|line| line.starts_with("```"))
}

fn update_claude_status_hint(
    status_hint: &mut Option<SessionStatusHint>,
    pending_ask_user_question_ids: &mut HashSet<String>,
    json_value: &Value,
) {
    let tool_use_result_resolved = json_value
        .pointer("/toolUseResult")
        .map(is_skipped_or_empty_answer_json)
        .unwrap_or(false);
    if tool_use_result_resolved {
        pending_ask_user_question_ids.clear();
    }

    let mut saw_regular_activity = tool_use_result_resolved;
    if let Some(content_blocks) = json_array_at(json_value, &["/message/content"]) {
        for content_block in content_blocks {
            match string_at(content_block, &["/type"]) {
                Some("tool_use") if string_at(content_block, &["/name"]) == Some("AskUserQuestion") => {
                    if let Some(tool_use_id) = string_at(content_block, &["/id"]) {
                        pending_ask_user_question_ids.insert(tool_use_id.to_string());
                    }
                }
                Some("tool_result") => {
                    if let Some(tool_use_id) = string_at(content_block, &["/tool_use_id"]) {
                        pending_ask_user_question_ids.remove(tool_use_id);
                    }
                    saw_regular_activity = true;
                }
                Some("tool_use") | Some("text") | Some("thinking") => {
                    saw_regular_activity = true;
                }
                _ => {}
            }
        }
    } else if is_claude_message_event(json_value) {
        saw_regular_activity = true;
    }

    if pending_ask_user_question_ids.is_empty() {
        if saw_regular_activity {
            *status_hint = Some(SessionStatusHint::Running);
        }
    } else {
        *status_hint = Some(SessionStatusHint::Waiting);
    }
}

fn is_copilot_message_event(json_value: &Value) -> bool {
    input_text_from_copilot_event(json_value).is_some() || json_kind(json_value) == Some(1)
}

fn is_claude_message_event(json_value: &Value) -> bool {
    matches!(string_at(json_value, &["/type"]), Some("user") | Some("assistant"))
}

const CLAUDE_CONTEXT_PREFIXES: &[&str] = &[
    "<ide_",
    "<environment_context>",
    "<system-reminder>",
    "<command-message>",
    "Base directory for this skill:",
];

fn is_claude_context_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    CLAUDE_CONTEXT_PREFIXES.iter().any(|prefix| trimmed.starts_with(prefix))
}

fn claude_interactive_user_text(json_value: &Value) -> Option<String> {
    if string_at(json_value, &["/type"]) != Some("user") {
        return None;
    }
    let content_value = json_value
        .pointer("/message/content")
        .or_else(|| json_value.pointer("/content"))?;
    let content_blocks = content_value.as_array()?;
    let mut parts: Vec<String> = Vec::new();
    for block in content_blocks {
        if string_at(block, &["/type"]) != Some("tool_result") {
            continue;
        }
        if bool_at(json_value, &["/toolUseResult/skipped", "/toolUseResult/isSkipped"]) == Some(true) {
            continue;
        }
        if let Some(answers) = json_value.pointer("/toolUseResult/answers") {
            collect_interactive_answer_strings(answers, &mut parts);
        }
    }
    if parts.is_empty() {
        return None;
    }
    clean_preview_text(&parts.join("\n"))
}

fn is_skip_sentinel(s: &str) -> bool {
    let normalized = s
        .trim()
        .trim_matches(|ch: char| ch == '_' || ch == '-' || ch == ':' || ch == '.')
        .to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "skip"
            | "skipped"
            | "skip question"
            | "skipped question"
            | "skip_question"
            | "skip-question"
            | "no answer"
            | "no_answer"
            | "跳过"
            | "已跳过"
            | "略过"
            | "不回答"
            | "无回答"
    )
}

fn is_skipped_json(value: &Value) -> bool {
    bool_at(
        value,
        &[
            "/skipped",
            "/isSkipped",
            "/data/skipped",
            "/data/isSkipped",
            "/toolUseResult/skipped",
            "/toolUseResult/isSkipped",
            "/toolSpecificData/skipped",
            "/toolSpecificData/isSkipped",
            "/resultDetails/skipped",
            "/resultDetails/isSkipped",
        ],
    ) == Some(true)
}

fn is_skipped_or_empty_answer_json(value: &Value) -> bool {
    if is_skipped_json(value) {
        return true;
    }

    let Some(answers) = value
        .pointer("/answers")
        .or_else(|| value.pointer("/toolUseResult/answers"))
    else {
        return false;
    };

    let mut answer_parts = Vec::new();
    collect_interactive_answer_strings(answers, &mut answer_parts);
    answer_parts.is_empty()
}

fn collect_interactive_answer_strings(value: &Value, parts: &mut Vec<String>) {
    match value {
        Value::String(s) => {
            let trimmed = s.trim();
            if !trimmed.is_empty() && !is_skip_sentinel(trimmed) {
                parts.push(trimmed.to_string());
            }
        }
        Value::Array(arr) => {
            for item in arr {
                collect_interactive_answer_strings(item, parts);
            }
        }
        Value::Object(obj) => {
            if is_skipped_json(value) {
                return;
            }
            for (_key, val) in obj {
                collect_interactive_answer_strings(val, parts);
            }
        }
        _ => {}
    }
}

fn claude_clean_user_text(json_value: &Value) -> Option<String> {
    if bool_at(json_value, &["/isMeta"]) == Some(true) {
        return None;
    }
    let content_value = json_value
        .pointer("/message/content")
        .or_else(|| json_value.pointer("/content"))?;
    if let Some(content_str) = content_value.as_str() {
        if is_claude_context_text(content_str) {
            return None;
        }
        return clean_preview_text(content_str);
    }
    let content_blocks = content_value.as_array()?;
    let has_tool_result = content_blocks
        .iter()
        .any(|block| string_at(block, &["/type"]) == Some("tool_result"));
    if has_tool_result {
        return None;
    }
    let mut texts: Vec<&str> = Vec::new();
    for block in content_blocks {
        if string_at(block, &["/type"]) != Some("text") {
            continue;
        }
        if let Some(text) = string_at(block, &["/text"]) {
            if !text.trim().is_empty() && !is_claude_context_text(text) {
                texts.push(text);
            }
        }
    }
    if texts.is_empty() {
        return None;
    }
    clean_preview_text(&texts.join("\n"))
}

fn claude_assistant_text(json_value: &Value) -> Option<String> {
    let content_value = json_value
        .pointer("/message/content")
        .or_else(|| json_value.pointer("/content"))?;
    if let Some(content_str) = content_value.as_str() {
        return clean_preview_text(content_str);
    }
    let content_blocks = content_value.as_array()?;
    let mut texts: Vec<String> = Vec::new();
    for block in content_blocks {
        match string_at(block, &["/type"]) {
            Some("text") => {
                if let Some(text) = string_at(block, &["/text"]) {
                    if let Some(clean) = clean_preview_text(text) {
                        texts.push(clean);
                    }
                }
            }
            Some("tool_use") if is_claude_question_tool_name(block) => {
                if let Some(question_text) = question_tool_preview_text(block) {
                    texts.push(question_text);
                }
            }
            _ => {}
        }
    }
    if texts.is_empty() {
        return None;
    }
    clean_preview_text(&texts.join("\n"))
}

fn is_claude_question_tool_name(content_block: &Value) -> bool {
    let Some(tool_name) = string_at(content_block, &["/name"]) else { return false; };
    let normalized: String = tool_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(normalized.as_str(), "askuserquestion" | "askquestion" | "question")
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

    let status = status_from_activity(updated_ms, scan_time_ms, ActivitySource::FileModified, None);
    let workspace = workspace_path
        .as_deref()
        .map(Path::new)
        .and_then(display_name_from_path)
        .or_else(|| index_path.parent().and_then(display_name_from_path))
        .unwrap_or_else(|| "Claude".to_string());
    let first_prompt = string_at(entry_json, &["/firstPrompt"]).and_then(clean_preview_text);
    let title = string_at(entry_json, &["/aiTitle", "/customTitle", "/sessionTitle", "/title", "/slug"])
        .and_then(title_candidate_from_text)
        .or_else(|| first_prompt.as_deref().and_then(title_candidate_from_text))
        .unwrap_or_else(|| format!("Claude {}", short_id(&session_id)));
    let message_count = entry_json
        .get("messageCount")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u64::from(u32::MAX)) as u32;
    let branch = string_at(entry_json, &["/gitBranch", "/branch"])
        .filter(|branch_text| !branch_text.trim().is_empty())
        .map(|branch_text| clean_label(branch_text, 48));
    let first_prompt_preview = apply_user_preview_budget(first_prompt);

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
        last_user_message: first_prompt_preview.0,
        last_user_message_truncated: first_prompt_preview.1,
        last_ai_message: None,
        last_ai_message_truncated: false,
        last_ai_message_excerpt_kind: None,
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
            .clamp(1, 5000),
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

fn json_array_at<'json>(json_value: &'json Value, pointers: &[&str]) -> Option<&'json Vec<Value>> {
    pointers
        .iter()
        .find_map(|pointer| json_value.pointer(pointer).and_then(Value::as_array))
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

fn copilot_event_requests(json_value: &Value) -> Option<&Vec<Value>> {
    let key_path = json_value
        .get("k")
        .or_else(|| json_value.pointer("/data/k"))
        .and_then(Value::as_array);
    if let Some(key_path) = key_path {
        if json_path_ends_with(key_path, &["requests"]) {
            if let Some(arr) = json_value
                .get("v")
                .or_else(|| json_value.pointer("/data/v"))
                .and_then(Value::as_array)
            {
                return Some(arr);
            }
        }
    }
    json_array_at(json_value, &["/v/requests", "/requests"])
}

fn is_copilot_system_request(request: &Value) -> bool {
    bool_at(request, &["/isSystemInitiated"]) == Some(true)
        || string_at(request, &["/systemInitiatedLabel"])
            .map(|s| !s.is_empty())
            .unwrap_or(false)
        || string_at(request, &["/terminalExecutionId"])
            .map(|s| !s.is_empty())
            .unwrap_or(false)
}

fn copilot_request_message_text(request: &Value) -> Option<&str> {
    if let Some(text) = string_at(request, &["/message/text"]) {
        if !text.trim().is_empty() {
            return Some(text);
        }
    }
    if let Some(parts) = json_array_at(request, &["/message/parts"]) {
        for part in parts {
            if let Some(text) = string_at(part, &["/text"]) {
                if !text.trim().is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}

fn copilot_interactive_user_text(json_value: &Value) -> Option<String> {
    if !key_path_ends_with(json_value, &["response"]) {
        return None;
    }
    let response_items = json_value
        .get("v")
        .or_else(|| json_value.pointer("/data/v"))
        .and_then(Value::as_array)?;
    let mut answer_parts: Vec<String> = Vec::new();
    for response_item in response_items {
        if string_at(response_item, &["/kind"]) != Some("questionCarousel") {
            continue;
        }
        if bool_at(response_item, &["/isUsed"]) != Some(true) {
            continue;
        }
        if is_skipped_json(response_item) {
            continue;
        }
        let Some(data) = response_item.get("data") else {
            continue;
        };
        if let Some(data_object) = data.as_object() {
            for answer_value in data_object.values() {
                collect_copilot_carousel_answer(response_item, answer_value, &mut answer_parts);
            }
        } else {
            collect_copilot_carousel_answer(response_item, data, &mut answer_parts);
        }
    }
    if answer_parts.is_empty() {
        return None;
    }
    clean_preview_text(&answer_parts.join(", "))
}

fn collect_copilot_carousel_answer(carousel_item: &Value, answer_value: &Value, answer_parts: &mut Vec<String>) {
    if is_skipped_json(answer_value) {
        return;
    }
    if let Some(freeform) = string_at(answer_value, &["/freeformValue"]) {
        let trimmed = freeform.trim();
        if !trimmed.is_empty() && !is_skip_sentinel(trimmed) {
            answer_parts.push(trimmed.to_string());
        }
    }

    if let Some(selected_id) = answer_value.get("selectedValue").and_then(Value::as_str) {
        if let Some(label) = resolve_carousel_option_label(carousel_item, selected_id) {
            if !is_skip_sentinel(&label) {
                answer_parts.push(label);
            }
        }
    }

    if let Some(selected_vals) = answer_value.get("selectedValues").and_then(Value::as_array) {
        for selected_val in selected_vals {
            if let Some(selected_id) = selected_val.as_str() {
                if let Some(label) = resolve_carousel_option_label(carousel_item, selected_id) {
                    if !is_skip_sentinel(&label) {
                        answer_parts.push(label);
                    }
                }
            }
        }
    }
}

fn resolve_carousel_option_label(carousel_item: &Value, selected_id: &str) -> Option<String> {
    let trimmed = selected_id.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(questions) = carousel_item.get("questions").and_then(Value::as_array) {
        for question in questions {
            if let Some(options) = question.get("options").and_then(Value::as_array) {
                for option in options {
                    let option_id = string_at(option, &["/id"]).unwrap_or("");
                    let option_value = string_at(option, &["/value"]).unwrap_or("");
                    if option_id == trimmed || option_value == trimmed {
                        return Some(
                            string_at(option, &["/label"])
                                .or_else(|| string_at(option, &["/value"]))
                                .unwrap_or(trimmed)
                                .to_string(),
                        );
                    }
                }
            }
        }
    }
    Some(trimmed.to_string())
}

fn copilot_request_user_text(json_value: &Value) -> Option<String> {
    let requests = copilot_event_requests(json_value)?;
    let mut last_text: Option<&str> = None;
    for request in requests {
        if is_copilot_system_request(request) {
            continue;
        }
        if let Some(text) = copilot_request_message_text(request) {
            last_text = Some(text);
        }
    }
    last_text.and_then(clean_preview_text)
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

fn key_path_ends_with(json_value: &Value, suffix: &[&str]) -> bool {
    json_value
        .get("k")
        .or_else(|| json_value.pointer("/data/k"))
        .and_then(Value::as_array)
        .map(|key_path| json_path_ends_with(key_path, suffix))
        .unwrap_or(false)
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

fn clean_preview_text(raw_text: &str) -> Option<String> {
    let raw_text = raw_text.trim();
    let raw_char_count = raw_text.chars().count();
    let source_was_truncated = raw_char_count > SESSION_PREVIEW_SOURCE_MAX_CHARS;
    let source_text = if source_was_truncated {
        let start_char = raw_char_count - SESSION_PREVIEW_SOURCE_MAX_CHARS;
        let start_byte = raw_text
            .char_indices()
            .nth(start_char)
            .map(|(index, _)| index)
            .unwrap_or(0);
        &raw_text[start_byte..]
    } else {
        raw_text
    };

    let mut output = String::new();
    for text_char in source_text.chars() {
        if text_char == '\r' {
            continue;
        }
        if text_char.is_control() && text_char != '\n' && text_char != '\t' {
            continue;
        }

        output.push(if text_char == '\t' { ' ' } else { text_char });
    }

    let mut output = output.trim().to_string();
    if output.is_empty() {
        return None;
    }
    if source_was_truncated {
        output.insert_str(0, "...\n");
    }

    Some(output)
}

const USER_SYSTEM_ERROR_SUBSTRINGS: &[&str] = &["Prompt is too long", "Context is too long"];

// Patterns that identify model-level/system-level noise, not real AI prose responses.
const AI_MODEL_NOISE_SUBSTRINGS: &[&str] = &[
    "there's an issue with the selected model",
    "selected model",
    "may not exist or you may not have access",
    "does not exist or you do not have access",
    "run --model",
    "prompt is too long",
    "context is too long",
    "api error",
    "rate limit",
    "overloaded",
];

fn is_user_system_error_text(text: &str) -> bool {
    USER_SYSTEM_ERROR_SUBSTRINGS.iter().any(|s| text.contains(s))
}

fn is_ai_model_noise_text(text: &str) -> bool {
    let lower_text = text.to_ascii_lowercase();
    AI_MODEL_NOISE_SUBSTRINGS.iter().any(|s| lower_text.contains(s))
}

fn apply_tail_budget(cleaned: &str, max_chars: usize) -> (String, bool) {
    let text = cleaned.trim();
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return (text.to_string(), false);
    }
    let start_char = char_count - max_chars;
    let start_byte = text.char_indices().nth(start_char).map(|(i, _)| i).unwrap_or(0);
    let raw_tail = &text[start_byte..];
    let tail = if let Some(newline_index) = raw_tail.find('\n') {
        if newline_index <= 80 {
            &raw_tail[newline_index + 1..]
        } else {
            raw_tail
        }
    } else {
        raw_tail
    };
    let tail = tail.trim_start();
    (format!("...{}{}", if tail.contains('\n') { "\n" } else { "" }, tail), true)
}

fn apply_user_preview_budget(msg: Option<String>) -> (Option<String>, bool) {
    let Some(text) = msg else { return (None, false); };
    let (result, trunc) = apply_tail_budget(&text, USER_PREVIEW_MAX_CHARS);
    if result.is_empty() { (None, false) } else { (Some(result), trunc) }
}

fn apply_ai_preview_budget(msg: Option<String>) -> (Option<String>, bool, Option<String>) {
    let Some(text) = msg else { return (None, false, None); };
    let clean = text.trim();
    // Tail-first: show the most recent content of the AI response.
    // No error-keyword prioritization — model/system errors are filtered upstream.
    let (result, trunc) = apply_tail_budget(clean, AI_PREVIEW_MAX_CHARS);
    let excerpt_kind = if trunc { Some("tail".to_string()) } else { None };
    if result.is_empty() { (None, false, None) } else { (Some(result), trunc, excerpt_kind) }
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

fn status_from_activity(
    updated_ms: u64,
    scan_time_ms: u64,
    activity_source: ActivitySource,
    status_hint: Option<SessionStatusHint>,
) -> String {
    if status_hint == Some(SessionStatusHint::Waiting) {
        return "waiting".to_string();
    }

    let age_ms = scan_time_ms.saturating_sub(updated_ms);
    if (status_hint == Some(SessionStatusHint::Running) && age_ms <= STATUS_RUNNING_HINT_WINDOW_MS)
        || age_ms <= recent_activity_window_ms(activity_source)
    {
        "running".to_string()
    } else {
        "idle".to_string()
    }
}

fn recent_activity_window_ms(activity_source: ActivitySource) -> u64 {
    match activity_source {
        ActivitySource::ContentTimestamp => STATUS_RECENT_CONTENT_WINDOW_MS,
        ActivitySource::FileModified => STATUS_RECENT_FILE_WINDOW_MS,
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

fn spawn_hidden_with_output(mut command: Command) -> std::io::Result<std::process::Output> {
    #[cfg(target_os = "windows")]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }

    command.output()
}

fn get_local_bridge_version() -> String {
    if let Some(package_json_path) = find_bridge_source_dir().map(|dir| dir.join("package.json")) {
        if let Some(json_value) = read_json_file(&package_json_path) {
            if let Some(version) = string_at(&json_value, &["/version"]) {
                return version.to_string();
            }
        }
    }

    "0.0.1".to_string()
}

fn get_installed_bridge_version() -> Option<String> {
    let user_profile_path = env::var_os("USERPROFILE")?;
    let user_profile_path = PathBuf::from(user_profile_path);

    for extensions_dir in [
        user_profile_path.join(".vscode").join("extensions"),
        user_profile_path.join(".vscode-insiders").join("extensions"),
    ] {
        for entry in read_directory(&extensions_dir) {
            let folder_name = entry.file_name().to_string_lossy().to_string();
            if folder_name.starts_with(BRIDGE_EXTENSION_FOLDER_PREFIX) {
                let package_json_path = entry.path().join("package.json");
                if let Some(json_value) = read_json_file(&package_json_path) {
                    if let Some(version) = string_at(&json_value, &["/version"]) {
                        return Some(version.to_string());
                    }
                }
                return Some("unknown".to_string());
            }
        }
    }

    None
}

fn find_bridge_source_dir() -> Option<PathBuf> {
    bridge_base_dirs()
        .into_iter()
        .map(|base_dir| base_dir.join("vscode-agentwatcher-bridge"))
        .find(|bridge_dir| bridge_dir.join("package.json").exists())
}

fn find_bridge_vsix(bridge_dir: &Path, local_version: &str) -> Option<PathBuf> {
    let mut fallback_vsix: Option<PathBuf> = None;
    let version_token = safe_file_name_token(local_version);

    for entry in read_directory(bridge_dir) {
        let path = entry.path();
        if !has_extension(&path, "vsix") {
            continue;
        }

        let file_name = entry.file_name().to_string_lossy().to_string();
        if !version_token.is_empty() && file_name.contains(&version_token) {
            return Some(path);
        }

        fallback_vsix = Some(match fallback_vsix {
            Some(existing_path)
                if modified_time_ms(&existing_path).unwrap_or(0) >= modified_time_ms(&path).unwrap_or(0) =>
            {
                existing_path
            }
            _ => path,
        });
    }

    fallback_vsix
}

fn package_bridge_vsix(bridge_dir: &Path, local_version: &str) -> Result<PathBuf, String> {
    let npx_path = find_npx_cli_path().ok_or_else(|| "npx CLI not found".to_string())?;
    let package_dir = env::temp_dir().join("AgentWatcher");
    fs::create_dir_all(&package_dir)
        .map_err(|error| format!("Failed to prepare bridge package directory: {}", error))?;

    let vsix_path = package_dir.join(format!(
        "agentwatcher-bridge-{}.vsix",
        safe_file_name_token(local_version)
    ));

    let mut package_cmd = Command::new(npx_path);
    package_cmd
        .args(["--yes", "@vscode/vsce", "package", "--skip-license", "--out"])
        .arg(&vsix_path)
        .current_dir(bridge_dir);

    let package_output = spawn_hidden_with_output(package_cmd)
        .map_err(|error| format!("Failed to package bridge extension: {}", error))?;

    if !package_output.status.success() {
        return Err(format!(
            "Bridge packaging failed: {}",
            String::from_utf8_lossy(&package_output.stderr)
        ));
    }

    Ok(vsix_path)
}

fn safe_file_name_token(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn bridge_base_dirs() -> Vec<PathBuf> {
    let mut base_dirs = Vec::new();

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            push_unique_path(&mut base_dirs, exe_dir.to_path_buf());
            for ancestor in exe_dir.ancestors().take(5).skip(1) {
                push_unique_path(&mut base_dirs, ancestor.to_path_buf());
            }
        }
    }

    if let Ok(current_dir) = std::env::current_dir() {
        push_unique_path(&mut base_dirs, current_dir);
    }

    base_dirs
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing_path| existing_path == &path) {
        paths.push(path);
    }
}

fn find_code_cli_path() -> Option<String> {
    // Try PATH first
    for candidate in ["code", "code.cmd", "code.exe"] {
        let mut test_cmd = Command::new(candidate);
        test_cmd.arg("--version");
        if spawn_hidden_with_output(test_cmd).is_ok() {
            return Some(candidate.to_string());
        }
    }

    // Try common Windows locations
    let common_paths = vec![
        env::var_os("LOCALAPPDATA").map(|p| {
            PathBuf::from(p)
                .join("Programs")
                .join("Microsoft VS Code")
                .join("bin")
                .join("code.cmd")
        }),
        env::var_os("LOCALAPPDATA").map(|p| {
            PathBuf::from(p)
                .join("Programs")
                .join("Microsoft VS Code")
                .join("Code.exe")
        }),
        Some(PathBuf::from("C:\\Program Files\\Microsoft VS Code\\bin\\code.cmd")),
        Some(PathBuf::from("C:\\Program Files\\Microsoft VS Code\\Code.exe")),
        Some(PathBuf::from("C:\\Program Files (x86)\\Microsoft VS Code\\bin\\code.cmd")),
        Some(PathBuf::from("C:\\Program Files (x86)\\Microsoft VS Code\\Code.exe")),
    ];

    for path_option in common_paths {
        if let Some(path) = path_option {
            if path.exists() {
                return Some(path_to_string(&path));
            }
        }
    }

    None
}

fn find_npx_cli_path() -> Option<String> {
    for candidate in ["npx", "npx.cmd", "npx.exe"] {
        let mut test_cmd = Command::new(candidate);
        test_cmd.arg("--version");
        if spawn_hidden_with_output(test_cmd)
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            return Some(candidate.to_string());
        }
    }

    None
}

fn parse_version(version_str: &str) -> Option<Vec<u32>> {
    version_str
        .split('.')
        .map(|part| part.parse::<u32>().ok())
        .collect()
}

fn auto_install_bridge_on_startup() {
    std::thread::spawn(|| {
        let status = get_bridge_status();
        if !status.installed || status.needs_update {
            let _ = install_bridge();
        }
    });
}

fn runtime_window_icon() -> Option<tauri::image::Image<'static>> {
    let expected_bytes = (RUNTIME_ICON_SIZE * RUNTIME_ICON_SIZE * 4) as usize;
    if RUNTIME_ICON_RGBA.len() != expected_bytes {
        return None;
    }

    Some(tauri::image::Image::new(
        RUNTIME_ICON_RGBA,
        RUNTIME_ICON_SIZE,
        RUNTIME_ICON_SIZE,
    ))
}

#[cfg(target_os = "windows")]
fn apply_native_window_icons<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>) {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetAncestor, GetWindowThreadProcessId, SetClassLongPtrW, HICON,
        PrivateExtractIconsW, SendMessageW, GA_ROOT, GCLP_HICON, GCLP_HICONSM, ICON_BIG,
        ICON_SMALL, ICON_SMALL2, WM_SETICON,
    };

    let Ok(raw_hwnd) = window.hwnd() else {
        return;
    };

    let Some(exe_path) = std::env::current_exe().ok() else {
        return;
    };

    let exe_path_wide: Vec<u16> = exe_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    fn extract_icon(exe_path_wide: &[u16], icon_size: i32) -> Option<HICON> {
        let mut icon: HICON = std::ptr::null_mut();
        let mut icon_id = 0u32;
        let count = unsafe {
            PrivateExtractIconsW(
                exe_path_wide.as_ptr(),
                0,
                icon_size,
                icon_size,
                &mut icon,
                &mut icon_id,
                1,
                0,
            )
        };

        if count == 0 || icon.is_null() {
            None
        } else {
            Some(icon)
        }
    }

    fn add_hwnd(hwnds: &mut Vec<HWND>, hwnd: HWND) {
        if hwnd.is_null() || hwnds.iter().any(|existing| *existing == hwnd) {
            return;
        }

        hwnds.push(hwnd);
    }

    unsafe extern "system" fn enum_process_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let hwnds = &mut *(lparam as *mut Vec<HWND>);
        let mut process_id = 0u32;
        let _ = GetWindowThreadProcessId(hwnd, &mut process_id);

        if process_id == std::process::id() {
            add_hwnd(hwnds, hwnd);
        }

        1
    }

    unsafe {
        let apply_icons_to = |target_hwnd: HWND| {
            if let Some(big_icon) = extract_icon(&exe_path_wide, 256) {
                let _ = SendMessageW(target_hwnd, WM_SETICON, ICON_BIG as usize, big_icon as isize);
                let _ = SetClassLongPtrW(target_hwnd, GCLP_HICON, big_icon as isize);
            }

            if let Some(small_icon) = extract_icon(&exe_path_wide, 32) {
                let _ = SendMessageW(target_hwnd, WM_SETICON, ICON_SMALL as usize, small_icon as isize);
                let _ = SendMessageW(target_hwnd, WM_SETICON, ICON_SMALL2 as usize, small_icon as isize);
                let _ = SetClassLongPtrW(target_hwnd, GCLP_HICONSM, small_icon as isize);
            }
        };

        let hwnd = raw_hwnd.0 as HWND;
        let root_hwnd = GetAncestor(hwnd, GA_ROOT);
        let mut hwnds = Vec::new();
        add_hwnd(&mut hwnds, hwnd);
        add_hwnd(&mut hwnds, root_hwnd);

        let _ = EnumWindows(Some(enum_process_windows), &mut hwnds as *mut Vec<HWND> as LPARAM);

        for target_hwnd in hwnds {
            apply_icons_to(target_hwnd);
        }
    }
}

#[cfg(target_os = "windows")]
fn apply_native_always_on_top<R: tauri::Runtime>(window: &tauri::WebviewWindow<R>, always_on_top: bool) {
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{HWND, LPARAM};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetAncestor, GetWindowThreadProcessId, SetWindowPos, GA_ROOT, HWND_NOTOPMOST,
        HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    let Ok(raw_hwnd) = window.hwnd() else {
        return;
    };

    fn add_hwnd(hwnds: &mut Vec<HWND>, hwnd: HWND) {
        if hwnd.is_null() || hwnds.iter().any(|existing| *existing == hwnd) {
            return;
        }

        hwnds.push(hwnd);
    }

    unsafe extern "system" fn enum_process_windows(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let hwnds = &mut *(lparam as *mut Vec<HWND>);
        let mut process_id = 0u32;
        let _ = GetWindowThreadProcessId(hwnd, &mut process_id);

        if process_id == std::process::id() {
            add_hwnd(hwnds, hwnd);
        }

        1
    }

    unsafe {
        let hwnd = raw_hwnd.0 as HWND;
        let root_hwnd = GetAncestor(hwnd, GA_ROOT);
        let mut hwnds = Vec::new();
        add_hwnd(&mut hwnds, hwnd);
        add_hwnd(&mut hwnds, root_hwnd);

        let _ = EnumWindows(Some(enum_process_windows), &mut hwnds as *mut Vec<HWND> as LPARAM);

        let insert_after = if always_on_top { HWND_TOPMOST } else { HWND_NOTOPMOST };
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE;
        for target_hwnd in hwnds {
            let _ = SetWindowPos(target_hwnd, insert_after, 0, 0, 0, 0, flags);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn copilot_unanswered_question_carousel_is_waiting() {
        let event = json!({
            "k": ["requests", 0, "response"],
            "v": [{ "kind": "questionCarousel" }]
        });

        assert_eq!(copilot_response_status_hint(&event), Some(SessionStatusHint::Waiting));
    }

    #[test]
    fn copilot_used_question_carousel_resolves_unfinished_ask_tool() {
        let event = json!({
            "k": ["requests", 0, "response"],
            "v": [
                {
                    "kind": "toolInvocationSerialized",
                    "toolId": "vscode_askQuestions",
                    "isComplete": false
                },
                {
                    "kind": "questionCarousel",
                    "isUsed": true,
                    "data": { "answer": { "isSkipped": true } }
                }
            ]
        });

        assert_eq!(copilot_response_status_hint(&event), Some(SessionStatusHint::Running));
    }

    #[test]
    fn copilot_completed_request_clears_prior_question_waiting() {
        let mut status_hint = None;
        let ask_event = json!({
            "k": ["requests", 0, "response"],
            "v": [{ "kind": "questionCarousel" }]
        });
        let done_event = json!({
            "k": ["requests", 0, "modelState"],
            "v": { "value": 2, "completedAt": 1779513991274_u64 }
        });

        update_copilot_status_hint(&mut status_hint, &ask_event);
        assert_eq!(status_hint, Some(SessionStatusHint::Waiting));

        update_copilot_status_hint(&mut status_hint, &done_event);
        assert_eq!(status_hint, Some(SessionStatusHint::Running));
    }

    #[test]
    fn copilot_ask_questions_tool_is_part_of_ai_preview() {
        let event = json!({
            "k": ["requests", 0, "response"],
            "v": [
                { "kind": "text", "value": "I need one more choice." },
                {
                    "kind": "toolInvocationSerialized",
                    "toolId": "vscode_askQuestions",
                    "input": {
                        "questions": [
                            {
                                "header": "Deploy target",
                                "question": "Which environment should I use?",
                                "message": "Pick the target for this run.",
                                "options": [
                                    { "label": "Staging", "description": "safer dry run" },
                                    { "label": "Production" }
                                ]
                            }
                        ]
                    }
                }
            ]
        });

        let preview = copilot_response_preview_text(&event).unwrap();

        assert!(preview.contains("I need one more choice."));
        assert!(preview.contains("Deploy target"));
        assert!(preview.contains("Which environment should I use?"));
        assert!(preview.contains("Options: Staging - safer dry run; Production"));
    }

    #[test]
    fn copilot_transcript_ask_questions_marks_waiting_preview() {
        let transcript = r#"
{"type":"user.message","data":{"content":"Please continue."},"timestamp":"2026-05-30T03:51:00.000Z"}
{"type":"tool.execution_start","data":{"toolCallId":"ask-1","toolName":"vscode_askQuestions","arguments":{"questions":[{"header":"Reminder test","question":"Did the card appear quickly?","options":[{"label":"Yes"},{"label":"No"}]}]}},"timestamp":"2026-05-30T03:51:01.000Z"}
"#;

        let overlay = copilot_transcript_overlay_from_text(transcript).unwrap();

        assert_eq!(overlay.status_hint, Some(SessionStatusHint::Waiting));
        assert!(overlay.last_user_message.unwrap().contains("Please continue."));
        let ai_message = overlay.last_ai_message.unwrap();
        assert!(ai_message.contains("Reminder test"));
        assert!(ai_message.contains("Did the card appear quickly?"));
        assert!(ai_message.contains("Options: Yes; No"));
    }

    #[test]
    fn copilot_transcript_completed_ask_questions_is_not_waiting() {
        let transcript = r#"
{"type":"assistant.message","data":{"content":"Please choose.","toolRequests":[{"toolCallId":"ask-1","name":"vscode_askQuestions","arguments":{"questions":[{"header":"Reminder test","question":"Did the card appear quickly?","options":[{"label":"Yes"},{"label":"No"}]}]}}]},"timestamp":"2026-05-30T03:51:00.000Z"}
{"type":"tool.execution_start","data":{"toolCallId":"ask-1","toolName":"vscode_askQuestions","arguments":{"questions":[{"header":"Reminder test","question":"Did the card appear quickly?"}]}},"timestamp":"2026-05-30T03:51:01.000Z"}
{"type":"tool.execution_complete","data":{"toolCallId":"ask-1","success":true},"timestamp":"2026-05-30T03:51:02.000Z"}
"#;

        let overlay = copilot_transcript_overlay_from_text(transcript).unwrap();

        assert_eq!(overlay.status_hint, Some(SessionStatusHint::Running));
        assert!(overlay.last_ai_message.unwrap().contains("Reminder test"));
    }

    #[test]
    fn copilot_transcript_assistant_tool_request_without_execution_start_is_not_waiting() {
        let transcript = r#"
{"type":"assistant.message","data":{"content":"Please choose.","toolRequests":[{"toolCallId":"ask-1","name":"vscode_askQuestions","arguments":{"questions":[{"header":"New question","question":"This should appear before execution_start."}]}}]},"timestamp":"2026-05-30T03:51:00.000Z"}
{"type":"assistant.turn_end","data":{"turnId":"175"},"timestamp":"2026-05-30T03:51:00.000Z"}
{"type":"assistant.turn_start","data":{"turnId":"176"},"timestamp":"2026-05-30T03:51:00.000Z"}
"#;

        let overlay = copilot_transcript_overlay_from_text(transcript).unwrap();

        assert_eq!(overlay.status_hint, Some(SessionStatusHint::Running));
        assert_eq!(overlay.unstarted_ask_timestamp_ms, Some(1780113060000));
        assert!(copilot_unstarted_ask_is_recent(
            &overlay,
            1780113060000 + COPILOT_UNSTARTED_ASK_WAITING_WINDOW_MS - 1
        ));
        assert!(!copilot_unstarted_ask_is_recent(
            &overlay,
            1780113060000 + COPILOT_UNSTARTED_ASK_WAITING_WINDOW_MS + 1
        ));
        assert!(overlay.last_ai_message.unwrap().contains("New question"));
    }

    #[test]
    fn copilot_transcript_assistant_tool_request_waits_after_execution_start() {
        let transcript = r#"
{"type":"assistant.message","data":{"content":"Please choose.","toolRequests":[{"toolCallId":"ask-1","name":"vscode_askQuestions","arguments":{"questions":[{"header":"New question","question":"This should wait after execution_start."}]}}]},"timestamp":"2026-05-30T03:51:00.000Z"}
{"type":"tool.execution_start","data":{"toolCallId":"ask-1","toolName":"vscode_askQuestions","arguments":{"questions":[{"header":"New question","question":"This should wait after execution_start."}]}}}
"#;

        let overlay = copilot_transcript_overlay_from_text(transcript).unwrap();

        assert_eq!(overlay.status_hint, Some(SessionStatusHint::Waiting));
        assert_eq!(overlay.unstarted_ask_timestamp_ms, None);
        assert!(overlay.last_ai_message.unwrap().contains("New question"));
    }

    #[test]
    fn copilot_transcript_later_user_message_clears_stale_question() {
        let transcript = r#"
{"type":"assistant.message","data":{"content":"Please choose.","toolRequests":[{"toolCallId":"ask-1","name":"vscode_askQuestions","arguments":{"questions":[{"header":"Old question","question":"This was skipped earlier."}]}}]},"timestamp":"2026-05-30T03:51:00.000Z"}
{"type":"user.message","data":{"content":"Continue after skipping."},"timestamp":"2026-05-30T03:52:00.000Z"}
"#;

        let overlay = copilot_transcript_overlay_from_text(transcript).unwrap();

        assert_eq!(overlay.status_hint, Some(SessionStatusHint::Running));
        assert_eq!(overlay.last_user_message.as_deref(), Some("Continue after skipping."));
    }

    #[test]
    fn copilot_transcript_later_assistant_message_clears_stale_question() {
        let transcript = r#"
{"type":"assistant.message","data":{"content":"Please choose.","toolRequests":[{"toolCallId":"ask-1","name":"vscode_askQuestions","arguments":{"questions":[{"header":"Old question","question":"This was skipped earlier."}]}}]},"timestamp":"2026-05-30T03:51:00.000Z"}
{"type":"assistant.message","data":{"content":"Continuing after that choice."},"timestamp":"2026-05-30T03:52:00.000Z"}
"#;

        let overlay = copilot_transcript_overlay_from_text(transcript).unwrap();

        assert_eq!(overlay.status_hint, Some(SessionStatusHint::Running));
        assert_eq!(overlay.last_ai_message.as_deref(), Some("Continuing after that choice."));
    }

    #[test]
    fn claude_ask_user_question_tool_is_part_of_ai_preview() {
        let event = json!({
            "type": "assistant",
            "message": {
                "content": [
                    { "type": "text", "text": "I need confirmation before continuing." },
                    {
                        "type": "tool_use",
                        "id": "toolu_ask_1",
                        "name": "AskUserQuestion",
                        "input": {
                            "question": "Should I restart AgentWatcher now?",
                            "options": ["Restart", "Wait"]
                        }
                    }
                ]
            }
        });

        let preview = claude_assistant_text(&event).unwrap();

        assert!(preview.contains("I need confirmation before continuing."));
        assert!(preview.contains("Should I restart AgentWatcher now?"));
        assert!(preview.contains("Options: Restart; Wait"));
    }

    #[test]
    fn latest_activity_uses_file_modified_when_content_timestamp_lags() {
        assert_eq!(latest_activity_ms(Some(100), 200), (200, ActivitySource::FileModified));
        assert_eq!(latest_activity_ms(Some(300), 200), (300, ActivitySource::ContentTimestamp));
        assert_eq!(latest_activity_ms(None, 200), (200, ActivitySource::FileModified));
    }

    #[test]
    fn claude_empty_tool_use_result_answers_clear_pending_ask() {
        let mut status_hint = Some(SessionStatusHint::Waiting);
        let mut pending_ask_ids = HashSet::new();
        pending_ask_ids.insert("ask-1".to_string());
        let event = json!({
            "type": "user",
            "toolUseResult": { "answers": [] }
        });

        update_claude_status_hint(&mut status_hint, &mut pending_ask_ids, &event);

        assert!(pending_ask_ids.is_empty());
        assert_eq!(status_hint, Some(SessionStatusHint::Running));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            scan_sessions,
            open_session,
            get_bridge_status,
            install_bridge,
            set_window_always_on_top
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                if let Some(icon) = runtime_window_icon() {
                    let _ = window.set_icon(icon);
                }
                #[cfg(target_os = "windows")]
                apply_native_window_icons(&window);
                #[cfg(target_os = "windows")]
                {
                    let icon_refresh_window = window.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(800));
                        apply_native_window_icons(&icon_refresh_window);
                    });
                }
                let _ = window.set_always_on_top(true);
                let _ = window.set_decorations(false);
                let _ = window.set_resizable(true);
            }

            auto_install_bridge_on_startup();
            start_session_file_watcher(app.handle().clone());

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running AgentWatcher");
}
