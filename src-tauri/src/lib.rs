use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{Emitter, Manager};

pub mod orchestration;
pub mod plugin_catalog;
pub mod plugin_state;
const DEFAULT_MAX_ACTIVE_SESSIONS: usize = 80;
const DEFAULT_ACTIVE_SESSION_DAYS: u64 = 7;
const COPILOT_TITLE_SAMPLE_LINES: usize = 240;
const CLAUDE_TITLE_SAMPLE_LINES: usize = 240;
const JSONL_HEAD_SAMPLE_BYTES: usize = 128 * 1024;
const JSONL_TAIL_SAMPLE_BYTES: usize = 256 * 1024;
const COPILOT_PROMPT_SCAN_BYTES: usize = 16 * 1024 * 1024;
const COPILOT_TRANSCRIPT_SCAN_BYTES: usize = 2 * 1024 * 1024;
const CLAUDE_PROMPT_SCAN_BYTES: usize = 8 * 1024 * 1024;
const CODEX_APP_SERVER_READY_TIMEOUT_MS: u64 = 8_000;
const CODEX_APP_SERVER_RPC_TIMEOUT_MS: u64 = 20_000;
const CODEX_APP_SERVER_START_ATTEMPTS: usize = 4;
const CODEX_TURN_FETCH_LIMIT: usize = 8;
const CODEX_SCAN_TURN_FETCH_BUDGET: usize = 3;
const OPENCODE_SESSION_SCAN_LIMIT_MAX: usize = 240;
const OPENCODE_PART_SCAN_LIMIT: usize = 24;
const OPENCODE_SUMMARY_FETCH_BUDGET: usize = 4;
const OPENCODE_SQLITE_BUSY_TIMEOUT_MS: u64 = 50;
const OPENCODE_SCAN_CACHE_TTL_MS: u64 = 10_000;
const OPENCODE_HANDOFF_PART_MAX_CHARS: usize = 64 * 1024;
const STATUS_RUNNING_HINT_WINDOW_MS: u64 = 2 * 60 * 60_000;
const STATUS_RECENT_CONTENT_WINDOW_MS: u64 = 10 * 60_000;
const STATUS_RECENT_FILE_WINDOW_MS: u64 = 3 * 60_000;
const COPILOT_UNSTARTED_ASK_WAITING_WINDOW_MS: u64 = 24 * 60 * 60_000;
const SESSION_PREVIEW_SOURCE_MAX_CHARS: usize = 64 * 1024;
const USER_PREVIEW_MAX_CHARS: usize = 400;
const AI_PREVIEW_MAX_CHARS: usize = 800;
const SESSIONS_CHANGED_EVENT: &str = "agentwatcher-sessions-changed";
const RUNTIME_ICON_SIZE: u32 = 256;
const RUNTIME_ICON_RGBA: &[u8] = include_bytes!("../icons/icon-runtime-256.rgba");
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;
const TODO_DISPATCH_HISTORY_LIMIT: usize = 5;
const TODO_PLUGIN_WORKFLOW_EVENT_LIMIT: usize = 12;
const TODO_LONG_TEXT_LIMIT: usize = 4_000;
const TODO_SHORT_TEXT_LIMIT: usize = 800;

fn bridge_extension_name() -> String {
    ["agentwatcher", "vscode", "session", "bridge"].join("-")
}

fn bridge_extension_id() -> String {
    format!("{}.{}", "agentwatcher", bridge_extension_name())
}

fn bridge_extension_folder_prefix() -> String {
    format!("{}-", bridge_extension_id())
}

fn bridge_source_dir_name() -> String {
    ["vscode", "agentwatcher", "bridge"].join("-")
}

fn bridge_vsix_file_name(local_version: &str) -> String {
    format!(
        "{}-{}.vsix",
        ["agentwatcher", "bridge"].join("-"),
        local_version
    )
}

fn legacy_bridge_extension_ids() -> Vec<String> {
    let base = format!("{}-{}", bridge_extension_id(), "safe");
    let mut ids = vec![base.clone()];
    ids.extend((1..=4).map(|index| format!("{base}{index}")));
    ids
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentSession {
    id: String,
    provider: String,
    provider_label: String,
    title: String,
    workspace: String,
    workspace_path: Option<String>,
    workspace_key: String,
    workspace_name: String,
    workspace_label: String,
    workspace_group: String,
    workspace_discriminator: String,
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
    todo_ids: Vec<String>,
    plugin_workflow: Option<PluginWorkflowSessionMarker>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct PluginWorkflowSessionMarker {
    plugin_name: String,
    workflow: String,
    provider: String,
    task_id: Option<String>,
    run_id: Option<String>,
    contract_version: Option<String>,
}

fn detect_plugin_workflow_session_marker(
    fallback_provider: &str,
    texts: &[Option<&str>],
) -> Option<PluginWorkflowSessionMarker> {
    let joined = texts
        .iter()
        .flatten()
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    if !joined
        .to_ascii_lowercase()
        .contains("[agentwatcher plugin workflow]")
    {
        return None;
    }
    Some(PluginWorkflowSessionMarker {
        plugin_name: extract_marker_value(&joined, &["plugin"])?,
        workflow: extract_marker_value(&joined, &["workflow"])?,
        provider: extract_marker_value(&joined, &["provider"]).unwrap_or_else(|| {
            match fallback_provider {
                "claude" => "claude-code",
                value => value,
            }
            .to_string()
        }),
        task_id: extract_marker_value(&joined, &["taskId", "task_id", "task-id"]),
        run_id: extract_marker_value(&joined, &["runId", "run_id", "run-id"]),
        contract_version: extract_marker_value(&joined, &["contractVersion", "contract_version"]),
    })
}

fn extract_marker_value(text: &str, keys: &[&str]) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    for key in keys {
        let key = key.to_ascii_lowercase();
        for separator in ["=", ":"] {
            let marker = format!("{key}{separator}");
            if let Some(index) = lower.find(&marker) {
                let rest = text[index + marker.len()..].trim_start();
                let value: String = rest
                    .chars()
                    .take_while(|ch| {
                        ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':')
                    })
                    .collect();
                if !value.is_empty() {
                    return Some(value);
                }
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenSessionRequest {
    id: Option<String>,
    provider: Option<String>,
    workspace_path: Option<String>,
    session_resource: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HandoffLaunchRequest {
    mode: String,
    workspace_path: Option<String>,
    prompt: Option<String>,
    wait_for_ack: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HandoffSourceContextRequest {
    id: Option<String>,
    provider: Option<String>,
    workspace_path: Option<String>,
    session_path: Option<String>,
    session_resource: Option<String>,
    title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct HandoffSourceContext {
    provider: String,
    session_id: Option<String>,
    primary_source_file: Option<String>,
    inline_summary: Option<String>,
    message_count: u32,
    warning: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HandoffAck {
    ok: bool,
    token: Option<String>,
    message: Option<String>,
}

#[derive(Debug, Clone)]
struct WorkspaceLaunchTarget {
    argument_path: String,
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

#[derive(Debug, Clone, Default)]
struct BridgeExtensionInventory {
    stable_version: Option<String>,
    legacy_ids: Vec<String>,
}

impl BridgeExtensionInventory {
    fn add_stable_version(&mut self, version: String) {
        self.stable_version = match self.stable_version.take() {
            Some(existing_version)
                if parse_version(&existing_version) >= parse_version(&version) =>
            {
                Some(existing_version)
            }
            _ => Some(version),
        };
    }

    fn add_legacy_id(&mut self, extension_id: &str) {
        if !self
            .legacy_ids
            .iter()
            .any(|existing_id| existing_id.eq_ignore_ascii_case(extension_id))
        {
            self.legacy_ids.push(extension_id.to_string());
        }
    }

    fn merge(&mut self, other: BridgeExtensionInventory) {
        if let Some(version) = other.stable_version {
            self.add_stable_version(version);
        }

        for legacy_id in other.legacy_ids {
            self.add_legacy_id(&legacy_id);
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScanOptions {
    max_sessions: Option<usize>,
    active_window_days: Option<u64>,
    hide_archived: Option<bool>,
    include_copilot: Option<bool>,
    include_copilot_cli: Option<bool>,
    include_claude: Option<bool>,
    include_codex: Option<bool>,
    include_open_code: Option<bool>,
    workspace_path_blacklist: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
struct ResolvedScanOptions {
    max_sessions: usize,
    active_window_ms: u64,
    hide_archived: bool,
    include_copilot: bool,
    include_copilot_cli: bool,
    include_claude: bool,
    include_codex: bool,
    include_open_code: bool,
    workspace_path_blacklist: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceProviderCounts {
    copilot: usize,
    copilot_cli: usize,
    claude: usize,
    codex: usize,
    opencode: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceScanSnapshot {
    updated_ms: u64,
    duration_ms: u64,
    total_sessions: usize,
    provider_counts: PerformanceProviderCounts,
    codex_turn_requests: u64,
    codex_turns_returned: u64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceSnapshotOptions {
    include_agent_clients: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceProcessTotals {
    cpu_percent: f64,
    working_set_mb: f64,
    private_mb: f64,
    process_count: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceProcessSnapshot {
    pid: u32,
    parent_pid: u32,
    name: String,
    role: String,
    root_pid: u32,
    root_name: String,
    cpu_percent: f64,
    working_set_mb: f64,
    private_mb: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct PerformanceSnapshot {
    captured_ms: u64,
    include_agent_clients: bool,
    process: PerformanceProcessTotals,
    processes: Vec<PerformanceProcessSnapshot>,
    scan: PerformanceScanSnapshot,
    gpu: Option<Value>,
    gpu_note: String,
}

#[derive(Debug, Clone)]
struct ProcessCpuSample {
    sampled_ms: u64,
    cpu_time_100ns: u64,
}

static PERFORMANCE_LAST_SCAN: OnceLock<Mutex<PerformanceScanSnapshot>> = OnceLock::new();
static PERFORMANCE_PROCESS_SAMPLES: OnceLock<Mutex<HashMap<u32, ProcessCpuSample>>> =
    OnceLock::new();
static PERFORMANCE_CODEX_TURN_REQUESTS: AtomicU64 = AtomicU64::new(0);
static PERFORMANCE_CODEX_TURNS_RETURNED: AtomicU64 = AtomicU64::new(0);

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
    todo_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct CopilotTranscriptOverlay {
    status_hint: Option<SessionStatusHint>,
    latest_timestamp_ms: Option<u64>,
    unstarted_ask_timestamp_ms: Option<u64>,
    last_user_message: Option<String>,
    last_ai_message: Option<String>,
    todo_ids: Vec<String>,
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
    todo_ids: Vec<String>,
}

#[derive(Debug)]
struct CodexAppServerProcess {
    port: u16,
    child: Child,
}

struct OpenCodeWebServerProcess {
    port: u16,
    child: Child,
}

#[derive(Debug, Clone, Default)]
struct CodexTurnSummary {
    thread_updated_ms: u64,
    fetched_ms: u64,
    last_user_message: Option<String>,
    last_ai_message: Option<String>,
    message_count: u32,
    latest_turn_status: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct CodexFileSessionSummary {
    session_id: String,
    is_subagent: bool,
    workspace_path: Option<String>,
    title: Option<String>,
    latest_timestamp_ms: Option<u64>,
    message_count: u32,
    last_user_message: Option<String>,
    last_user_message_truncated: bool,
    last_ai_message: Option<String>,
    last_ai_message_truncated: bool,
    last_ai_message_excerpt_kind: Option<String>,
    archived: bool,
    todo_ids: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct OpenCodeSessionSummary {
    last_user_message: Option<String>,
    last_ai_message: Option<String>,
    message_count: u32,
    latest_part_ms: u64,
    status_hint: Option<SessionStatusHint>,
    todo_ids: Vec<String>,
}

#[derive(Debug, Clone)]
struct CachedOpenCodeSummary {
    session_updated_ms: u64,
    summary: OpenCodeSessionSummary,
}

#[derive(Debug, Clone)]
struct CachedOpenCodeScan {
    captured_ms: u64,
    key: OpenCodeScanCacheKey,
    sessions: Vec<AgentSession>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenCodeScanCacheKey {
    max_sessions: usize,
    active_window_ms: u64,
    hide_archived: bool,
}

#[derive(Debug, Clone)]
struct OpenCodeSessionRow {
    id: String,
    title: String,
    directory: String,
    path: Option<String>,
    time_created: u64,
    time_updated: u64,
    time_archived: Option<u64>,
}

#[derive(Debug, Clone)]
struct OpenCodeTranscriptMessage {
    id: String,
    role: String,
    time_ms: u64,
    parts: Vec<OpenCodeTranscriptPart>,
}

#[derive(Debug, Clone)]
struct OpenCodeTranscriptPart {
    label: String,
    text: String,
}

static COPILOT_SESSION_CACHE: OnceLock<
    Mutex<HashMap<String, CachedJsonlSummary<CopilotSessionSummary>>>,
> = OnceLock::new();
static COPILOT_TRANSCRIPT_CACHE: OnceLock<
    Mutex<HashMap<String, CachedJsonlSummary<CopilotTranscriptOverlay>>>,
> = OnceLock::new();
static CLAUDE_SESSION_CACHE: OnceLock<
    Mutex<HashMap<String, CachedJsonlSummary<ClaudeSessionSummary>>>,
> = OnceLock::new();
static CODEX_TURN_SUMMARY_CACHE: OnceLock<Mutex<HashMap<String, CodexTurnSummary>>> =
    OnceLock::new();
static CODEX_FILE_SESSION_CACHE: OnceLock<
    Mutex<HashMap<String, CachedJsonlSummary<CodexFileSessionSummary>>>,
> = OnceLock::new();
static CODEX_APP_SERVER: OnceLock<Mutex<Option<CodexAppServerProcess>>> = OnceLock::new();
static OPENCODE_SESSION_SUMMARY_CACHE: OnceLock<Mutex<HashMap<String, CachedOpenCodeSummary>>> =
    OnceLock::new();
static OPENCODE_SCAN_CACHE: OnceLock<Mutex<Option<CachedOpenCodeScan>>> = OnceLock::new();
static OPENCODE_WEB_SERVER: OnceLock<Mutex<Option<OpenCodeWebServerProcess>>> = OnceLock::new();

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
            let copilot_cli_store = appdata_path
                .join(product_folder)
                .join("User")
                .join("globalStorage")
                .join("github.copilot-chat")
                .join("session-store.db");
            push_existing_path(&mut roots, copilot_cli_store.clone());
            push_existing_path(&mut roots, copilot_cli_store.with_extension("db-wal"));
        }
    }
    if let Some(user_profile_path) = env::var_os("USERPROFILE") {
        let user_profile_path = PathBuf::from(user_profile_path);
        let codex_home = user_profile_path.join(".codex");
        let opencode_data = user_profile_path
            .join(".local")
            .join("share")
            .join("opencode");
        push_existing_path(
            &mut roots,
            user_profile_path.join(".claude").join("projects"),
        );
        for codex_path in [
            codex_home.join("sessions"),
            codex_home.join("archived_sessions"),
            codex_home.join("session_index.jsonl"),
            codex_home.join("state_5.sqlite"),
            codex_home.join("state_5.sqlite-wal"),
        ] {
            push_existing_path(&mut roots, codex_path);
        }
        push_existing_path(&mut roots, opencode_data.join("opencode.db"));
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
            || has_extension(path, "sqlite")
            || has_extension(path, "db")
            || path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.eq_ignore_ascii_case("sqlite-wal"))
                .unwrap_or(false)
            || path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| extension.eq_ignore_ascii_case("db-wal"))
                .unwrap_or(false)
            || path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .map(|file_name| file_name.eq_ignore_ascii_case("sessions-index.json"))
                .unwrap_or(false)
    })
}

fn emit_sessions_changed(app_handle: &tauri::AppHandle, emitted_ms: u64) {
    clear_opencode_scan_cache();
    let _ = app_handle.emit(SESSIONS_CHANGED_EVENT, emitted_ms);
}

#[tauri::command]
fn get_bridge_status() -> BridgeStatus {
    let local_version = get_local_bridge_version();
    let code_path = find_code_cli_path();
    let inventory = get_bridge_extension_inventory(code_path.as_deref());
    let installed_version = inventory.stable_version.clone();

    let installed = installed_version.is_some();
    let needs_cleanup = !inventory.legacy_ids.is_empty();
    let needs_version_update = installed_version
        .as_deref()
        .map(|version| bridge_version_needs_update(&local_version, version))
        .unwrap_or(false);
    let needs_update = needs_cleanup || needs_version_update;

    let message = if needs_cleanup && installed {
        format!(
            "Bridge cleanup needed: {} legacy extension(s) remain",
            inventory.legacy_ids.len()
        )
    } else if needs_cleanup {
        format!(
            "Bridge stable extension not installed; {} legacy extension(s) need cleanup",
            inventory.legacy_ids.len()
        )
    } else if !installed {
        "Bridge extension not installed".to_string()
    } else if needs_version_update {
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
    let vscode_bridge_dir =
        find_bridge_source_dir().ok_or_else(|| "Bridge source directory not found".to_string())?;
    let local_version = get_local_bridge_version();
    let vsix_path = find_bridge_vsix(&vscode_bridge_dir, &local_version)
        .map(Ok)
        .unwrap_or_else(|| package_bridge_vsix(&vscode_bridge_dir, &local_version))?;

    let code_path = find_code_cli_path().ok_or("VS Code CLI not found")?;
    let cleanup_warnings = uninstall_legacy_bridge_extensions(&code_path);
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

    let inventory = get_bridge_extension_inventory(Some(code_path.as_str()));
    if inventory.stable_version.is_none() {
        return Err(
            "Bridge installation verification failed: stable extension was not found after install"
                .to_string(),
        );
    }

    if !inventory.legacy_ids.is_empty() {
        return Err(format!(
            "Bridge installation cleanup incomplete: {} legacy extension(s) still installed",
            inventory.legacy_ids.len()
        ));
    }

    if cleanup_warnings.is_empty() {
        Ok("Bridge extension installed successfully".to_string())
    } else {
        Ok(format!(
            "Bridge extension installed successfully; cleanup warnings: {}",
            cleanup_warnings.join("; ")
        ))
    }
}

#[tauri::command]
fn set_window_always_on_top(
    window: tauri::WebviewWindow,
    always_on_top: bool,
) -> Result<(), String> {
    window
        .set_always_on_top(always_on_top)
        .map_err(|error| error.to_string())?;

    #[cfg(target_os = "windows")]
    apply_native_always_on_top(&window, always_on_top);

    Ok(())
}

#[tauri::command]
async fn scan_sessions(options: Option<ScanOptions>) -> Result<Vec<AgentSession>, String> {
    tauri::async_runtime::spawn_blocking(move || scan_sessions_blocking(options))
        .await
        .map_err(|error| format!("session scan worker failed: {error}"))
}

fn scan_sessions_blocking(options: Option<ScanOptions>) -> Vec<AgentSession> {
    let scan_started = Instant::now();
    let scan_time_ms = current_time_ms();
    let options = resolve_scan_options(options);
    PERFORMANCE_CODEX_TURN_REQUESTS.store(0, Ordering::Relaxed);
    PERFORMANCE_CODEX_TURNS_RETURNED.store(0, Ordering::Relaxed);
    let mut sessions = Vec::new();

    if options.include_copilot {
        sessions.extend(scan_copilot_sessions(scan_time_ms, &options));
    }
    if options.include_copilot_cli {
        sessions.extend(scan_copilot_cli_sessions(scan_time_ms, &options));
    }
    if options.include_claude {
        sessions.extend(scan_claude_sessions(scan_time_ms, &options));
    }
    if options.include_codex {
        sessions.extend(scan_codex_sessions(scan_time_ms, &options));
    }
    if options.include_open_code {
        sessions.extend(scan_opencode_sessions(scan_time_ms, &options));
    }

    apply_workspace_path_blacklist(&mut sessions, &options.workspace_path_blacklist);
    sessions.sort_by(|left_session, right_session| {
        status_rank(&left_session.status)
            .cmp(&status_rank(&right_session.status))
            .then_with(|| right_session.updated_ms.cmp(&left_session.updated_ms))
    });
    apply_session_limit_per_provider(&mut sessions, options.max_sessions);
    update_performance_scan_snapshot(&sessions, scan_started.elapsed());
    sessions
}

fn apply_workspace_path_blacklist(sessions: &mut Vec<AgentSession>, blacklist: &[String]) {
    let normalized_blacklist: Vec<String> = blacklist
        .iter()
        .filter_map(|path| normalized_workspace_path_key(path))
        .collect();
    if normalized_blacklist.is_empty() {
        return;
    }

    sessions.retain(|session| {
        let Some(workspace_key) = session
            .workspace_path
            .as_deref()
            .and_then(normalized_workspace_path_key)
        else {
            return true;
        };
        !normalized_blacklist.iter().any(|blocked_key| {
            workspace_key == *blocked_key
                || workspace_key
                    .strip_prefix(blocked_key)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
    });
}

fn apply_session_limit_per_provider(sessions: &mut Vec<AgentSession>, limit: usize) {
    let mut provider_counts: HashMap<String, usize> = HashMap::new();
    sessions.retain(|session| {
        let count = provider_counts.entry(session.provider.clone()).or_default();
        if *count >= limit {
            return false;
        }
        *count += 1;
        true
    });
}

#[tauri::command]
fn get_performance_snapshot(options: Option<PerformanceSnapshotOptions>) -> PerformanceSnapshot {
    let captured_ms = current_time_ms();
    let include_agent_clients = options
        .as_ref()
        .and_then(|options| options.include_agent_clients)
        .unwrap_or(false);
    let mut processes = collect_performance_processes(captured_ms, include_agent_clients);
    processes.sort_by(|left, right| {
        right
            .cpu_percent
            .partial_cmp(&left.cpu_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                right
                    .working_set_mb
                    .partial_cmp(&left.working_set_mb)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    let process = PerformanceProcessTotals {
        cpu_percent: round_metric(processes.iter().map(|process| process.cpu_percent).sum()),
        working_set_mb: round_metric(processes.iter().map(|process| process.working_set_mb).sum()),
        private_mb: round_metric(processes.iter().map(|process| process.private_mb).sum()),
        process_count: processes.len(),
    };
    let scan = performance_scan_snapshot();

    PerformanceSnapshot {
        captured_ms,
        include_agent_clients,
        process,
        processes,
        scan,
        gpu: None,
        gpu_note: if include_agent_clients {
            "Advanced mode samples compatible agent client process trees; GPU remains disabled to keep diagnostics lightweight."
        } else {
            "Per-process GPU sampling is disabled in lightweight diagnostics mode"
        }
        .to_string(),
    }
}

fn performance_scan_snapshot() -> PerformanceScanSnapshot {
    PERFORMANCE_LAST_SCAN
        .get_or_init(|| Mutex::new(PerformanceScanSnapshot::default()))
        .lock()
        .map(|snapshot| snapshot.clone())
        .unwrap_or_default()
}

fn update_performance_scan_snapshot(sessions: &[AgentSession], duration: Duration) {
    let mut provider_counts = PerformanceProviderCounts::default();
    for session in sessions {
        match session.provider.as_str() {
            "copilot" => provider_counts.copilot += 1,
            "copilot-cli" => provider_counts.copilot_cli += 1,
            "claude" => provider_counts.claude += 1,
            "codex" => provider_counts.codex += 1,
            "opencode" => provider_counts.opencode += 1,
            _ => {}
        }
    }

    let snapshot = PerformanceScanSnapshot {
        updated_ms: current_time_ms(),
        duration_ms: duration.as_millis().min(u128::from(u64::MAX)) as u64,
        total_sessions: sessions.len(),
        provider_counts,
        codex_turn_requests: PERFORMANCE_CODEX_TURN_REQUESTS.load(Ordering::Relaxed),
        codex_turns_returned: PERFORMANCE_CODEX_TURNS_RETURNED.load(Ordering::Relaxed),
    };

    if let Ok(mut last_scan) = PERFORMANCE_LAST_SCAN
        .get_or_init(|| Mutex::new(PerformanceScanSnapshot::default()))
        .lock()
    {
        *last_scan = snapshot;
    }
}

#[cfg(target_os = "windows")]
fn collect_performance_processes(
    captured_ms: u64,
    include_agent_clients: bool,
) -> Vec<PerformanceProcessSnapshot> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcessId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
    };

    #[derive(Debug, Clone)]
    struct ProcessEntry {
        pid: u32,
        parent_pid: u32,
        name: String,
        image_path: String,
    }

    #[derive(Debug, Clone)]
    struct ProcessRole {
        role: String,
        root_pid: u32,
        root_name: String,
    }

    fn display_process_name(entry: &ProcessEntry) -> String {
        if entry.name.is_empty() {
            format!("pid-{}", entry.pid)
        } else {
            entry.name.clone()
        }
    }

    let snapshot_handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot_handle == INVALID_HANDLE_VALUE {
        return Vec::new();
    }

    let mut entries = Vec::new();
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    if unsafe { Process32FirstW(snapshot_handle, &mut entry) } != 0 {
        loop {
            entries.push(ProcessEntry {
                pid: entry.th32ProcessID,
                parent_pid: entry.th32ParentProcessID,
                name: wide_null_string(&entry.szExeFile),
                image_path: String::new(),
            });

            entry = unsafe { std::mem::zeroed() };
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if unsafe { Process32NextW(snapshot_handle, &mut entry) } == 0 {
                break;
            }
        }
    }
    unsafe {
        CloseHandle(snapshot_handle);
    }

    if include_agent_clients {
        for entry in &mut entries {
            entry.image_path = performance_process_image_path(entry.pid).unwrap_or_default();
        }
    }

    let current_pid = unsafe { GetCurrentProcessId() };
    let mut included_processes: HashMap<u32, ProcessRole> = HashMap::new();
    let current_entry = entries
        .iter()
        .find(|entry| entry.pid == current_pid)
        .cloned()
        .unwrap_or(ProcessEntry {
            pid: current_pid,
            parent_pid: 0,
            name: "agentwatcher.exe".to_string(),
            image_path: String::new(),
        });

    let seed_process_tree =
        |root: &ProcessEntry, role: &str, included: &mut HashMap<u32, ProcessRole>| {
            let root_name = display_process_name(root);
            let root_pid = root.pid;
            included.entry(root_pid).or_insert_with(|| ProcessRole {
                role: role.to_string(),
                root_pid,
                root_name: root_name.clone(),
            });

            let mut changed = true;
            while changed {
                changed = false;
                for entry in &entries {
                    let Some(parent_role) = included.get(&entry.parent_pid) else {
                        continue;
                    };
                    if parent_role.root_pid != root_pid || included.contains_key(&entry.pid) {
                        continue;
                    }
                    included.insert(
                        entry.pid,
                        ProcessRole {
                            role: role.to_string(),
                            root_pid,
                            root_name: root_name.clone(),
                        },
                    );
                    changed = true;
                }
            }
        };

    seed_process_tree(&current_entry, "AgentWatcher", &mut included_processes);

    if include_agent_clients {
        for entry in &entries {
            if included_processes.contains_key(&entry.pid) {
                continue;
            }
            if let Some(role) = performance_agent_process_role(&entry.name, &entry.image_path) {
                seed_process_tree(entry, role, &mut included_processes);
            }
        }
    }

    let cpu_count = std::thread::available_parallelism()
        .map(|count| count.get() as f64)
        .unwrap_or(1.0)
        .max(1.0);
    let mut snapshots = Vec::new();
    let mut seen_pids = HashSet::new();
    let mut cpu_samples = match PERFORMANCE_PROCESS_SAMPLES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        Ok(samples) => samples,
        Err(_) => return Vec::new(),
    };

    for entry in entries
        .into_iter()
        .filter(|entry| included_processes.contains_key(&entry.pid))
    {
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ,
                0,
                entry.pid,
            )
        };
        if handle.is_null() {
            continue;
        }
        let process_role = included_processes
            .get(&entry.pid)
            .cloned()
            .unwrap_or(ProcessRole {
                role: "Agent".to_string(),
                root_pid: entry.pid,
                root_name: display_process_name(&entry),
            });

        let mut memory_counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
        memory_counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        let memory_ok = unsafe {
            GetProcessMemoryInfo(
                handle,
                &mut memory_counters,
                std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            )
        } != 0;

        let cpu_time_100ns = process_cpu_time_100ns(handle);
        let cpu_percent = cpu_time_100ns
            .and_then(|cpu_time| {
                let previous = cpu_samples.get(&entry.pid)?;
                if captured_ms <= previous.sampled_ms || cpu_time < previous.cpu_time_100ns {
                    return None;
                }
                let wall_delta_100ns =
                    captured_ms.saturating_sub(previous.sampled_ms) as f64 * 10_000.0;
                if wall_delta_100ns <= 0.0 {
                    return None;
                }
                let cpu_delta_100ns = cpu_time.saturating_sub(previous.cpu_time_100ns) as f64;
                Some((cpu_delta_100ns / (wall_delta_100ns * cpu_count)) * 100.0)
            })
            .unwrap_or(0.0);

        if let Some(cpu_time) = cpu_time_100ns {
            cpu_samples.insert(
                entry.pid,
                ProcessCpuSample {
                    sampled_ms: captured_ms,
                    cpu_time_100ns: cpu_time,
                },
            );
        }

        let working_set_mb = if memory_ok {
            bytes_to_mb(memory_counters.WorkingSetSize as u64)
        } else {
            0.0
        };
        let private_mb = if memory_ok {
            bytes_to_mb(memory_counters.PagefileUsage as u64)
        } else {
            0.0
        };

        snapshots.push(PerformanceProcessSnapshot {
            pid: entry.pid,
            parent_pid: entry.parent_pid,
            name: display_process_name(&entry),
            role: process_role.role,
            root_pid: process_role.root_pid,
            root_name: process_role.root_name,
            cpu_percent: round_metric(cpu_percent.max(0.0)),
            working_set_mb: round_metric(working_set_mb),
            private_mb: round_metric(private_mb),
        });
        seen_pids.insert(entry.pid);

        unsafe {
            CloseHandle(handle);
        }
    }

    cpu_samples.retain(|pid, _| seen_pids.contains(pid));
    snapshots
}

#[cfg(target_os = "windows")]
fn performance_process_image_path(pid: u32) -> Option<String> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if handle.is_null() {
        return None;
    }

    let mut buffer = vec![0u16; 1024];
    let mut size = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size) } != 0;
    unsafe {
        CloseHandle(handle);
    }

    if !ok || size == 0 {
        return None;
    }
    Some(String::from_utf16_lossy(&buffer[..size as usize]))
}

fn performance_agent_process_role(process_name: &str, image_path: &str) -> Option<&'static str> {
    let normalized = process_name.to_ascii_lowercase();
    let normalized_path = image_path.replace('/', "\\").to_ascii_lowercase();
    if normalized_path.contains("\\openai\\codex\\") {
        return Some("Codex Desktop");
    }
    if normalized_path.contains("\\opencode\\")
        || normalized_path.contains("\\opencode-ai\\")
        || normalized_path.contains("\\opencode-windows-")
    {
        return Some("OpenCode");
    }
    if normalized_path.contains("\\anthropicclaude\\") || normalized_path.contains("\\claude\\") {
        return Some("Claude");
    }
    if normalized_path.contains("\\github copilot\\") {
        return Some("Copilot");
    }
    if normalized_path.contains("\\microsoft vs code\\")
        || normalized_path.contains("\\code - insiders\\")
        || normalized_path.contains("\\vscodium\\")
    {
        return Some("VS Code");
    }

    match normalized.as_str() {
        "code.exe" => Some("VS Code"),
        "code - insiders.exe" => Some("VS Code Insiders"),
        "code - exploration.exe" => Some("VS Code Exploration"),
        "vscodium.exe" | "codium.exe" => Some("VS Code Compatible"),
        "codex.exe" | "codex-cli.exe" | "openai-codex.exe" => Some("Codex"),
        "opencode.exe" | "opencode.cmd" => Some("OpenCode"),
        "claude.exe" | "claude-code.exe" | "claude_desktop.exe" | "claude desktop.exe" => {
            Some("Claude")
        }
        "copilot.exe" | "copilot-cli.exe" | "github.copilot.exe" => Some("Copilot"),
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
fn collect_performance_processes(
    _captured_ms: u64,
    _include_agent_clients: bool,
) -> Vec<PerformanceProcessSnapshot> {
    Vec::new()
}

#[cfg(target_os = "windows")]
fn process_cpu_time_100ns(handle: windows_sys::Win32::Foundation::HANDLE) -> Option<u64> {
    use windows_sys::Win32::Foundation::FILETIME;
    use windows_sys::Win32::System::Threading::GetProcessTimes;

    let mut creation_time: FILETIME = unsafe { std::mem::zeroed() };
    let mut exit_time: FILETIME = unsafe { std::mem::zeroed() };
    let mut kernel_time: FILETIME = unsafe { std::mem::zeroed() };
    let mut user_time: FILETIME = unsafe { std::mem::zeroed() };

    if unsafe {
        GetProcessTimes(
            handle,
            &mut creation_time,
            &mut exit_time,
            &mut kernel_time,
            &mut user_time,
        )
    } == 0
    {
        return None;
    }

    Some(filetime_to_u64(kernel_time).saturating_add(filetime_to_u64(user_time)))
}

#[cfg(target_os = "windows")]
fn filetime_to_u64(file_time: windows_sys::Win32::Foundation::FILETIME) -> u64 {
    ((file_time.dwHighDateTime as u64) << 32) | file_time.dwLowDateTime as u64
}

#[cfg(target_os = "windows")]
fn wide_null_string(buffer: &[u16]) -> String {
    let len = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / 1_048_576.0
}

fn round_metric(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

#[tauri::command]
fn open_session(session: OpenSessionRequest) -> Result<(), String> {
    if clean_option(session.provider.as_deref()) == Some("codex") {
        return open_codex_session(&session);
    }
    if clean_option(session.provider.as_deref()) == Some("opencode") {
        return open_opencode_session(&session);
    }

    if bridge_extension_installed() && request_session_resource(&session).is_some() {
        let session_deep_link = session_deep_link(&session).map(|text| text.to_string());
        match open_session_with_bridge(&session) {
            Ok(()) => return Ok(()),
            Err(bridge_error) => {
                if session_deep_link.is_none() {
                    return open_agents_page(session.workspace_path.as_deref()).map_err(
                        |fallback_error| {
                            format!(
                                "Bridge session launch failed ({}); fallback failed ({})",
                                bridge_error, fallback_error
                            )
                        },
                    );
                }
            }
        }
    }

    if let Some(deep_link) = session_deep_link(&session) {
        match open_vscode_deep_link(&deep_link) {
            Ok(()) => return Ok(()),
            Err(exact_error) => {
                return open_agents_page(session.workspace_path.as_deref()).map_err(
                    |fallback_error| {
                        format!(
                            "Exact session launch failed ({}); fallback failed ({})",
                            exact_error, fallback_error
                        )
                    },
                );
            }
        }
    }

    open_agents_page(session.workspace_path.as_deref())
}

#[tauri::command]
async fn launch_handoff(request: HandoffLaunchRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || launch_handoff_blocking(request))
        .await
        .map_err(|error| format!("Failed to wait for handoff launch task: {}", error))?
}

#[tauri::command]
async fn prepare_handoff_source_context(
    request: HandoffSourceContextRequest,
) -> Result<HandoffSourceContext, String> {
    tauri::async_runtime::spawn_blocking(move || prepare_handoff_source_context_blocking(request))
        .await
        .map_err(|error| format!("Failed to wait for handoff source context task: {}", error))?
}

fn prepare_handoff_source_context_blocking(
    request: HandoffSourceContextRequest,
) -> Result<HandoffSourceContext, String> {
    let provider = clean_option(request.provider.as_deref()).unwrap_or("unknown");
    if provider != "opencode" {
        return Ok(HandoffSourceContext {
            provider: provider.to_string(),
            session_id: raw_request_session_id(request.id.as_deref()).map(str::to_string),
            primary_source_file: request
                .session_path
                .as_deref()
                .and_then(|path| clean_option(Some(path)))
                .map(str::to_string),
            inline_summary: None,
            message_count: 0,
            warning: None,
        });
    }

    prepare_opencode_handoff_source_context(&request)
}

fn launch_handoff_blocking(request: HandoffLaunchRequest) -> Result<(), String> {
    let mode = clean_option(Some(request.mode.as_str()))
        .ok_or_else(|| "Missing handoff launch mode".to_string())?;
    let workspace_path = clean_option(request.workspace_path.as_deref())
        .ok_or_else(|| "Missing target workspace path".to_string())?;
    let target = resolve_workspace_launch_target(workspace_path)?;
    let wait_for_ack = request.wait_for_ack.unwrap_or(true);

    match mode {
        "agents" => open_vscode_command_in_workspace(
            &target.argument_path,
            "workbench.action.chat.openNewSessionEditor.local",
            true,
            wait_for_ack,
        ),
        "code-chat" => open_vscode_command_in_workspace(
            &target.argument_path,
            "workbench.action.chat.openNewSessionEditor.copilotcli",
            true,
            wait_for_ack,
        ),
        "claude-panel" => open_vscode_command_in_workspace(
            &target.argument_path,
            "claude-vscode.editor.open",
            true,
            wait_for_ack,
        ),
        "codex-app" => {
            let prompt = clean_option(request.prompt.as_deref())
                .ok_or_else(|| "Missing handoff prompt".to_string())?;
            launch_codex_handoff(&target.argument_path, prompt).map(|_| ())
        }
        "opencode-cli" => {
            let prompt = clean_option(request.prompt.as_deref())
                .ok_or_else(|| "Missing handoff prompt".to_string())?;
            launch_opencode_handoff(&target.argument_path, prompt).map(|_| ())
        }
        other => Err(format!("Unsupported handoff launch mode: {}", other)),
    }
}

fn resolve_workspace_launch_target(workspace_path: &str) -> Result<WorkspaceLaunchTarget, String> {
    let raw_path = PathBuf::from(workspace_path);
    if !raw_path.is_absolute() {
        return Err("Target workspace path must be absolute".to_string());
    }

    let canonical_path = fs::canonicalize(&raw_path)
        .map_err(|error| format!("Target workspace path is not accessible: {}", error))?;

    if canonical_path.is_dir() {
        return Ok(WorkspaceLaunchTarget {
            argument_path: path_to_cli_string(&canonical_path),
        });
    }

    let is_code_workspace = canonical_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.eq_ignore_ascii_case("code-workspace"))
        .unwrap_or(false);

    if canonical_path.is_file() && is_code_workspace {
        return Ok(WorkspaceLaunchTarget {
            argument_path: path_to_cli_string(&canonical_path),
        });
    }

    Err("Target workspace path must be a folder or .code-workspace file".to_string())
}

fn path_to_cli_string(path: &Path) -> String {
    let path_text = path.to_string_lossy().into_owned();
    #[cfg(target_os = "windows")]
    {
        if let Some(stripped) = path_text.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{}", stripped);
        }
        if let Some(stripped) = path_text.strip_prefix(r"\\?\") {
            return stripped.to_string();
        }
    }
    path_text
}

fn open_vscode_command_in_workspace(
    workspace_path: &str,
    command: &str,
    insert_prompt: bool,
    wait_for_ack: bool,
) -> Result<(), String> {
    ensure_bridge_command_route_available()?;
    let ack_target = if insert_prompt && wait_for_ack {
        Some(create_handoff_ack_target()?)
    } else {
        None
    };
    open_workspace_in_new_window(workspace_path)?;

    let command_link = if insert_prompt {
        if let Some((ack_path, ack_token)) = ack_target.as_ref() {
            bridge_handoff_link(command, Some(ack_path.as_path()), Some(ack_token.as_str()))
        } else {
            bridge_handoff_link(command, None, None)
        }
    } else {
        bridge_command_link(command)
    };

    std::thread::sleep(Duration::from_millis(450));
    open_vscode_deep_link(&command_link)?;

    if let Some((ack_path, ack_token)) = ack_target {
        wait_for_handoff_ack(&ack_path, &ack_token)?;
    }

    Ok(())
}

fn create_handoff_ack_target() -> Result<(PathBuf, String), String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("System clock error: {}", error))?
        .as_millis();
    let token = format!("{}-{}", std::process::id(), now);
    let dir = env::temp_dir().join("AgentWatcher").join("handoff-acks");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Failed to prepare handoff ack directory: {}", error))?;
    Ok((dir.join(format!("{}.json", token)), token))
}

fn wait_for_handoff_ack(ack_path: &Path, token: &str) -> Result<(), String> {
    wait_for_bridge_ack(
        ack_path,
        token,
        Duration::from_secs(8),
        "handoff prompt insertion",
    )
}

fn wait_for_session_open_ack(ack_path: &Path, token: &str) -> Result<(), String> {
    wait_for_bridge_ack(ack_path, token, Duration::from_secs(5), "session open")
}

fn wait_for_bridge_ack(
    ack_path: &Path,
    token: &str,
    timeout: Duration,
    action_label: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        if ack_path.exists() {
            let text = fs::read_to_string(ack_path)
                .map_err(|error| format!("Failed to read bridge acknowledgement: {}", error))?;
            let ack: HandoffAck = serde_json::from_str(&text)
                .map_err(|error| format!("Invalid bridge acknowledgement: {}", error))?;

            if ack.token.as_deref() != Some(token) {
                return Err("Bridge acknowledgement token mismatch".to_string());
            }

            let _ = fs::remove_file(ack_path);
            if ack.ok {
                return Ok(());
            }

            return Err(ack
                .message
                .filter(|message| !message.trim().is_empty())
                .unwrap_or_else(|| format!("VS Code Bridge reported {} failure", action_label)));
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    Err(format!(
        "VS Code Bridge did not confirm {} before timeout",
        action_label
    ))
}

fn bridge_session_open_error_should_retry(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("workspace mismatch")
        || lower.contains("target workspace")
        || lower.contains("session open before timeout")
}

#[tauri::command]
fn get_todo_state() -> Result<Value, String> {
    let path = todo_state_path()?;
    read_todo_state_from_path(&path)
}

#[tauri::command]
fn save_todo_state(state: Value) -> Result<(), String> {
    let path = todo_state_path()?;
    write_todo_state_to_path(&path, &state)
}

#[tauri::command]
fn scan_plugins() -> Result<plugin_catalog::AwPluginCatalogSnapshot, String> {
    let (mount_store, installed_root) = plugin_catalog_paths()?;
    let mut snapshot = plugin_catalog::scan_catalog(&mount_store, &installed_root);
    let state_path = plugin_state_path()?;
    for plugin in &mut snapshot.plugins {
        plugin.enabled =
            plugin_state::get_plugin_runtime_state(&state_path, &plugin.descriptor.name)?.enabled;
    }
    Ok(snapshot)
}

#[tauri::command]
fn mount_plugin(plugin_path: String) -> Result<plugin_catalog::AwDiscoveredPlugin, String> {
    let path = PathBuf::from(plugin_path.trim());
    let (mount_store, _) = plugin_catalog_paths()?;
    let plugin = plugin_catalog::mount_plugin(&mount_store, &path)?;
    plugin_state::record_plugin_audit(
        &plugin_state_path()?,
        &plugin.descriptor.name,
        "mount",
        Some(format!("mounted external plugin from {}", plugin.root_path)),
    )?;
    Ok(plugin)
}

#[tauri::command]
fn unmount_plugin(plugin_name: String) -> Result<bool, String> {
    ensure_plugin_exists(&plugin_name)?;
    let (mount_store, _) = plugin_catalog_paths()?;
    let changed = plugin_catalog::unmount_plugin(&mount_store, &plugin_name)?;
    if changed {
        plugin_state::record_plugin_audit(
            &plugin_state_path()?,
            &plugin_name,
            "unmount",
            Some("unmounted external plugin".to_string()),
        )?;
    }
    Ok(changed)
}

#[tauri::command]
async fn invoke_plugin_command(
    request: plugin_catalog::AwPluginInvokeRequest,
) -> Result<plugin_catalog::AwPluginInvokeResponse, String> {
    ensure_plugin_enabled(&request.plugin_name)?;
    tauri::async_runtime::spawn_blocking(move || {
        let snapshot = scan_plugin_catalog()?;
        let plugin = plugin_catalog::find_plugin(&snapshot, &request.plugin_name)?;
        plugin_catalog::invoke_plugin(plugin, &request)
    })
    .await
    .map_err(|error| format!("failed to wait for plugin command: {error}"))?
}

#[tauri::command]
fn get_plugin_runtime_state(
    plugin_name: String,
) -> Result<plugin_state::AwPluginRuntimeState, String> {
    ensure_plugin_exists(&plugin_name)?;
    let path = plugin_state_path()?;
    plugin_state::get_plugin_runtime_state(&path, &plugin_name)
}

#[tauri::command]
fn set_plugin_enabled(
    plugin_name: String,
    enabled: bool,
) -> Result<plugin_state::AwPluginRuntimeState, String> {
    ensure_plugin_exists(&plugin_name)?;
    let path = plugin_state_path()?;
    plugin_state::set_plugin_enabled(
        &path,
        &plugin_name,
        enabled,
        Some("设置面板插件开关".to_string()),
    )
}

#[tauri::command]
fn get_orchestration_status() -> Result<orchestration::AwOrchestrationStatus, String> {
    let current_dir = env::current_dir()
        .map_err(|error| format!("Failed to determine AgentWatcher runtime directory: {error}"))?;
    orchestration::get_orchestration_status(&current_dir)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DebugRuntimeState {
    silent_debug: bool,
}

fn agentwatcher_silent_debug_enabled() -> bool {
    env::var("AGENTWATCHER_DEBUG_SILENT")
        .ok()
        .map(|value| {
            let normalized = value.trim().to_ascii_lowercase();
            matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

#[tauri::command]
fn get_debug_runtime_state() -> DebugRuntimeState {
    DebugRuntimeState {
        silent_debug: agentwatcher_silent_debug_enabled(),
    }
}

fn ensure_plugin_exists(plugin_name: &str) -> Result<(), String> {
    let snapshot = scan_plugin_catalog()?;
    plugin_catalog::find_plugin(&snapshot, plugin_name).map(|_| ())
}

fn ensure_plugin_enabled(plugin_name: &str) -> Result<(), String> {
    let path = plugin_state_path()?;
    let state = plugin_state::get_plugin_runtime_state(&path, plugin_name)?;
    if state.enabled {
        Ok(())
    } else {
        Err(format!("Plugin is disabled: {plugin_name}"))
    }
}

fn plugin_state_path() -> Result<PathBuf, String> {
    let appdata = env::var_os("APPDATA").map(PathBuf::from).ok_or_else(|| {
        "APPDATA is not available; cannot locate AgentWatcher plugin state".to_string()
    })?;
    Ok(plugin_state::plugin_state_path_from_appdata(&appdata))
}

fn plugin_catalog_paths() -> Result<(PathBuf, PathBuf), String> {
    let appdata = env::var_os("APPDATA").map(PathBuf::from).ok_or_else(|| {
        "APPDATA is not available; cannot locate AgentWatcher plugin mounts".to_string()
    })?;
    let localappdata = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| appdata.clone());
    Ok((
        plugin_catalog::mount_store_path_from_appdata(&appdata),
        plugin_catalog::installed_plugin_root_from_localappdata(&localappdata),
    ))
}

fn scan_plugin_catalog() -> Result<plugin_catalog::AwPluginCatalogSnapshot, String> {
    let (mount_store, installed_root) = plugin_catalog_paths()?;
    Ok(plugin_catalog::scan_catalog(&mount_store, &installed_root))
}

fn todo_state_path() -> Result<PathBuf, String> {
    let appdata = env::var_os("APPDATA").map(PathBuf::from).ok_or_else(|| {
        "APPDATA is not available; cannot locate AgentWatcher todo storage".to_string()
    })?;
    Ok(todo_state_path_from_appdata(&appdata))
}

fn todo_state_path_from_appdata(appdata: &Path) -> PathBuf {
    appdata.join("AgentWatcher").join("todos.v1.json")
}

fn default_todo_state() -> Value {
    serde_json::json!({
        "version": 1,
        "workspaces": {}
    })
}

fn read_todo_state_from_path(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(default_todo_state());
    }

    let text = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read todo state: {}", error))?;
    if text.trim().is_empty() {
        return Ok(default_todo_state());
    }

    serde_json::from_str(&text).map_err(|error| format!("Invalid todo state JSON: {}", error))
}

fn truncate_value_string(value: &mut Value, limit: usize) {
    if let Some(text) = value.as_str() {
        if text.len() > limit {
            *value = Value::String(text.chars().take(limit).collect());
        }
    }
}

fn trim_array_to_last(value: &mut Value, limit: usize) {
    if let Some(items) = value.as_array_mut() {
        if items.len() > limit {
            let remove_count = items.len() - limit;
            items.drain(0..remove_count);
        }
    }
}

fn sanitize_todo_dispatch(dispatch: &mut Value) {
    let Some(object) = dispatch.as_object_mut() else {
        return;
    };
    for key in ["lastPrompt", "launchError"] {
        if let Some(value) = object.get_mut(key) {
            truncate_value_string(value, TODO_LONG_TEXT_LIMIT);
        }
    }
    for key in ["sessionPath", "transcriptPath", "sessionResource"] {
        if let Some(value) = object.get_mut(key) {
            truncate_value_string(value, TODO_LONG_TEXT_LIMIT);
        }
    }
}

fn sanitize_todo_task(task: &mut Value) {
    let Some(object) = task.as_object_mut() else {
        return;
    };
    if let Some(dispatches) = object.get_mut("dispatches") {
        trim_array_to_last(dispatches, TODO_DISPATCH_HISTORY_LIMIT);
        if let Some(items) = dispatches.as_array_mut() {
            for dispatch in items {
                sanitize_todo_dispatch(dispatch);
            }
        }
    }
    if let Some(events) = object
        .get_mut("pluginWorkflowLifecycle")
        .and_then(|value| value.get_mut("events"))
    {
        trim_array_to_last(events, TODO_PLUGIN_WORKFLOW_EVENT_LIMIT);
    }
    if let Some(stdout) = object
        .get_mut("execution")
        .and_then(|value| value.get_mut("result"))
        .and_then(|value| value.get_mut("stdoutExcerpt"))
    {
        truncate_value_string(stdout, TODO_SHORT_TEXT_LIMIT);
    }
    if let Some(stderr) = object
        .get_mut("execution")
        .and_then(|value| value.get_mut("result"))
        .and_then(|value| value.get_mut("stderrExcerpt"))
    {
        truncate_value_string(stderr, TODO_SHORT_TEXT_LIMIT);
    }
}

fn sanitize_todo_state_for_write(state: &Value) -> Value {
    let mut sanitized = state.clone();
    if let Some(workspaces) = sanitized
        .get_mut("workspaces")
        .and_then(Value::as_object_mut)
    {
        for workspace in workspaces.values_mut() {
            if let Some(tasks) = workspace.get_mut("tasks").and_then(Value::as_array_mut) {
                for task in tasks {
                    sanitize_todo_task(task);
                }
            }
        }
    }
    sanitized
}

fn write_todo_state_to_path(path: &Path, state: &Value) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Todo state path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to prepare todo storage directory: {}", error))?;

    let sanitized = sanitize_todo_state_for_write(state);
    let text = serde_json::to_string_pretty(&sanitized)
        .map_err(|error| format!("Failed to encode todo state: {}", error))?;
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, text)
        .map_err(|error| format!("Failed to write todo state: {}", error))?;
    if path.exists() {
        fs::remove_file(path)
            .map_err(|error| format!("Failed to replace previous todo state: {}", error))?;
    }
    fs::rename(&temp_path, path).map_err(|error| format!("Failed to replace todo state: {}", error))
}

fn ensure_bridge_command_route_available() -> Result<(), String> {
    if bridge_command_route_available() {
        return Ok(());
    }

    install_bridge()?;

    if bridge_command_route_available() {
        Ok(())
    } else {
        Err("VS Code Bridge is not available after installation".to_string())
    }
}

fn bridge_command_route_available() -> bool {
    let code_path = find_code_cli_path();
    let inventory = get_bridge_extension_inventory(code_path.as_deref());

    if !inventory.legacy_ids.is_empty() {
        return false;
    }

    let Some(installed_version) = inventory.stable_version.as_deref() else {
        return false;
    };

    !bridge_version_needs_update(&get_local_bridge_version(), installed_version)
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

fn session_bridge_link(
    session: &OpenSessionRequest,
    ack_path: Option<&Path>,
    ack_token: Option<&str>,
) -> Option<String> {
    let session_resource = request_session_resource(session)?;
    let mut link = format!(
        "vscode://{}/open?target=editor&scopedResource={}",
        bridge_extension_id(),
        percent_encode_query_component(&session_resource)
    );

    if let Some(workspace_path) = clean_option(session.workspace_path.as_deref()) {
        link.push_str("&workspacePath=");
        link.push_str(&percent_encode_query_component(workspace_path));
    }

    append_bridge_ack_query(&mut link, ack_path, ack_token);
    Some(link)
}

fn bridge_command_link(command: &str) -> String {
    format!(
        "vscode://{}/command?command={}",
        bridge_extension_id(),
        percent_encode_query_component(command)
    )
}

fn bridge_handoff_link(command: &str, ack_path: Option<&Path>, ack_token: Option<&str>) -> String {
    let mut link = format!(
        "vscode://{}/handoff?command={}&insertPrompt=1",
        bridge_extension_id(),
        percent_encode_query_component(command)
    );

    append_bridge_ack_query(&mut link, ack_path, ack_token);
    link
}

fn append_bridge_ack_query(link: &mut String, ack_path: Option<&Path>, ack_token: Option<&str>) {
    if let Some(ack_path) = ack_path {
        link.push_str("&ackPath=");
        link.push_str(&percent_encode_query_component(&path_to_cli_string(
            ack_path,
        )));
    }

    if let Some(ack_token) = ack_token {
        link.push_str("&ackToken=");
        link.push_str(&percent_encode_query_component(ack_token));
    }
}

fn request_session_resource(session: &OpenSessionRequest) -> Option<String> {
    clean_option(session.session_resource.as_deref())
        .map(str::to_string)
        .or_else(|| session_resource_from_request(session))
}

fn open_session_with_bridge(session: &OpenSessionRequest) -> Result<(), String> {
    if let Some(workspace_path) = clean_option(session.workspace_path.as_deref()) {
        open_workspace(workspace_path)?;
    }

    const BRIDGE_OPEN_ATTEMPTS: usize = 4;
    const BRIDGE_OPEN_INITIAL_DELAY_MS: u64 = 1200;
    const BRIDGE_OPEN_RETRY_DELAY_MS: u64 = 900;

    let mut last_error = None;
    for attempt in 0..BRIDGE_OPEN_ATTEMPTS {
        let (ack_path, ack_token) = create_handoff_ack_target()?;
        let bridge_link = session_bridge_link(session, Some(ack_path.as_path()), Some(&ack_token))
            .ok_or_else(|| "Missing AgentWatcher session resource.".to_string())?;

        let delay_ms = if attempt == 0 {
            BRIDGE_OPEN_INITIAL_DELAY_MS
        } else {
            BRIDGE_OPEN_RETRY_DELAY_MS
        };
        std::thread::sleep(Duration::from_millis(delay_ms));
        open_vscode_deep_link(&bridge_link)?;

        match wait_for_session_open_ack(&ack_path, &ack_token) {
            Ok(()) => return Ok(()),
            Err(error) => {
                let should_retry = bridge_session_open_error_should_retry(&error)
                    && attempt + 1 < BRIDGE_OPEN_ATTEMPTS;
                last_error = Some(error);
                if !should_retry {
                    break;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "VS Code Bridge did not confirm session open.".to_string()))
}

fn open_workspace(workspace_path: &str) -> Result<(), String> {
    let arguments = vec![workspace_path.to_string()];
    spawn_code_cli(&arguments)
}

fn open_workspace_in_new_window(workspace_path: &str) -> Result<(), String> {
    let arguments = vec!["--new-window".to_string(), workspace_path.to_string()];
    spawn_code_cli(&arguments)
}

fn bridge_extension_installed() -> bool {
    let code_path = find_code_cli_path();
    get_bridge_extension_inventory(code_path.as_deref())
        .stable_version
        .is_some()
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
        "opencode" => Some(opencode_session_resource(session_id)),
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

    if is_windows_drive_path(&path_text) || !path_text.starts_with('/') {
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
    text.map(str::trim)
        .filter(|text_value| !text_value.is_empty())
}

fn scan_codex_sessions(scan_time_ms: u64, options: &ResolvedScanOptions) -> Vec<AgentSession> {
    let limit = codex_thread_list_limit(options);
    let mut params = json!({
        "limit": limit,
        "sortKey": "updated_at",
        "sortDirection": "desc"
    });

    if options.hide_archived {
        params["archived"] = Value::Bool(false);
    }

    let threads = with_codex_app_server_client(|client| {
        let response = client.request("thread/list", params)?;
        let Some(thread_items) = response.get("data").and_then(Value::as_array) else {
            return Ok(Vec::new());
        };

        let mut scanned_threads = Vec::with_capacity(thread_items.len());
        let mut turn_fetch_budget = codex_scan_turn_fetch_budget(options);
        for thread_json in thread_items {
            let Some(thread_id) = clean_option(string_at(thread_json, &["/id", "/sessionId"]))
            else {
                continue;
            };
            let updated_ms = codex_thread_updated_ms(thread_json, scan_time_ms);
            if !is_active_session(updated_ms, scan_time_ms, options) {
                continue;
            }

            let mut turn_summary = cached_codex_turn_summary(thread_id, updated_ms);
            if turn_summary.is_none() && turn_fetch_budget > 0 {
                turn_fetch_budget -= 1;
                if let Some(fetched_summary) =
                    fetch_codex_turn_summary(client, thread_id, updated_ms, scan_time_ms)
                {
                    cache_codex_turn_summary(thread_id, fetched_summary.clone());
                    turn_summary = Some(fetched_summary);
                }
            }

            scanned_threads.push((thread_json.clone(), turn_summary));
        }
        Ok(scanned_threads)
    })
    .unwrap_or_default();

    let mut sessions: Vec<AgentSession> = threads
        .iter()
        .filter_map(|(thread_json, turn_summary)| {
            read_codex_thread_with_summary(
                thread_json,
                scan_time_ms,
                options,
                turn_summary.as_ref(),
            )
        })
        .collect();

    let mut seen_thread_ids: HashSet<String> = sessions
        .iter()
        .filter_map(|session| session.id.strip_prefix("codex:").map(str::to_string))
        .collect();

    for session in scan_codex_file_sessions(scan_time_ms, options) {
        let Some(thread_id) = session.id.strip_prefix("codex:") else {
            sessions.push(session);
            continue;
        };
        if seen_thread_ids.insert(thread_id.to_string()) {
            sessions.push(session);
        }
    }

    sessions
}

fn scan_codex_file_sessions(scan_time_ms: u64, options: &ResolvedScanOptions) -> Vec<AgentSession> {
    let Some(user_profile_path) = env::var_os("USERPROFILE") else {
        return Vec::new();
    };

    let codex_home = PathBuf::from(user_profile_path).join(".codex");
    let mut roots = vec![codex_home.join("sessions")];
    if !options.hide_archived {
        roots.push(codex_home.join("archived_sessions"));
    }

    let mut candidates = Vec::new();
    for root in roots {
        collect_codex_jsonl_candidates(&root, scan_time_ms, options, &mut candidates);
    }

    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.1));
    candidates
        .into_iter()
        .take(candidate_scan_limit(options))
        .filter_map(|(session_path, _)| {
            read_codex_jsonl_session(&session_path, scan_time_ms, options)
        })
        .collect()
}

fn collect_codex_jsonl_candidates(
    root_path: &Path,
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
    candidates: &mut Vec<(PathBuf, u64)>,
) {
    let mut pending_dirs = vec![root_path.to_path_buf()];
    while let Some(directory_path) = pending_dirs.pop() {
        for entry in read_directory(&directory_path) {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                pending_dirs.push(entry_path);
                continue;
            }
            if !has_extension(&entry_path, "jsonl") {
                continue;
            }
            let modified_ms = dir_entry_modified_ms(&entry);
            if !is_active_session(
                fallback_updated_ms_from_modified(modified_ms, scan_time_ms),
                scan_time_ms,
                options,
            ) {
                continue;
            }
            candidates.push((entry_path, modified_ms));
        }
    }
}

fn codex_thread_list_limit(options: &ResolvedScanOptions) -> usize {
    options
        .max_sessions
        .saturating_mul(2)
        .max(options.max_sessions + 20)
        .clamp(40, 120)
}

fn codex_scan_turn_fetch_budget(options: &ResolvedScanOptions) -> usize {
    CODEX_SCAN_TURN_FETCH_BUDGET.min(options.max_sessions)
}

fn codex_thread_updated_ms(thread_json: &Value, scan_time_ms: u64) -> u64 {
    timestamp_ms_from_json(thread_json)
        .or_else(|| {
            thread_json
                .pointer("/createdAt")
                .and_then(timestamp_ms_from_value)
        })
        .unwrap_or(scan_time_ms)
}

fn is_codex_subagent_thread_json(thread_json: &Value) -> bool {
    is_codex_subagent_metadata(
        clean_option(string_at(
            thread_json,
            &["/thread_source", "/threadSource", "/payload/thread_source"],
        )),
        clean_option(string_at(
            thread_json,
            &[
                "/parent_thread_id",
                "/parentThreadId",
                "/payload/parent_thread_id",
            ],
        )),
        thread_json
            .pointer("/source")
            .or_else(|| thread_json.pointer("/payload/source")),
    )
}

fn is_codex_subagent_metadata(
    thread_source: Option<&str>,
    parent_thread_id: Option<&str>,
    source_json: Option<&Value>,
) -> bool {
    if thread_source
        .map(|source| source.eq_ignore_ascii_case("subagent"))
        .unwrap_or(false)
    {
        return true;
    }

    let has_subagent_source = source_json
        .and_then(Value::as_object)
        .map(|source| source.contains_key("subagent"))
        .unwrap_or(false);
    has_subagent_source && parent_thread_id.is_some()
}

fn cached_codex_turn_summary(thread_id: &str, updated_ms: u64) -> Option<CodexTurnSummary> {
    CODEX_TURN_SUMMARY_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|cache| cache.get(thread_id).cloned())
        .filter(|summary| summary.thread_updated_ms == updated_ms)
}

fn cache_codex_turn_summary(thread_id: &str, summary: CodexTurnSummary) {
    if let Ok(mut cache) = CODEX_TURN_SUMMARY_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        cache.insert(thread_id.to_string(), summary);
        if cache.len() > 512 {
            let mut entries: Vec<(String, u64)> = cache
                .iter()
                .map(|(cached_thread_id, summary)| (cached_thread_id.clone(), summary.fetched_ms))
                .collect();
            entries.sort_by_key(|(_, fetched_ms)| *fetched_ms);
            let prune_count = cache.len().saturating_sub(512);
            for (cached_thread_id, _) in entries.into_iter().take(prune_count) {
                cache.remove(&cached_thread_id);
            }
        }
    }
}

fn fetch_codex_turn_summary(
    client: &mut CodexWsClient,
    thread_id: &str,
    updated_ms: u64,
    fetched_ms: u64,
) -> Option<CodexTurnSummary> {
    PERFORMANCE_CODEX_TURN_REQUESTS.fetch_add(1, Ordering::Relaxed);
    let turns_response = client
        .request(
            "thread/turns/list",
            json!({
                "threadId": thread_id,
                "limit": CODEX_TURN_FETCH_LIMIT
            }),
        )
        .ok()?;
    let turns = turns_response.get("data").and_then(Value::as_array)?;
    PERFORMANCE_CODEX_TURNS_RETURNED.fetch_add(turns.len() as u64, Ordering::Relaxed);

    let mut summary = codex_turn_summary_from_turns(turns);
    summary.thread_updated_ms = updated_ms;
    summary.fetched_ms = fetched_ms;
    Some(summary)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            scan_sessions,
            get_performance_snapshot,
            open_session,
            launch_handoff,
            prepare_handoff_source_context,
            get_todo_state,
            save_todo_state,
            scan_plugins,
            mount_plugin,
            unmount_plugin,
            invoke_plugin_command,
            get_plugin_runtime_state,
            set_plugin_enabled,
            get_orchestration_status,
            get_bridge_status,
            install_bridge,
            get_debug_runtime_state,
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
                let _ = window.set_always_on_top(!agentwatcher_silent_debug_enabled());
                let _ = window.set_decorations(false);
                let _ = window.set_resizable(true);
            }

            auto_install_bridge_on_startup();
            start_session_file_watcher(app.handle().clone());

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building AgentWatcher");

    app.run(|_app_handle, event| {
        if matches!(
            event,
            tauri::RunEvent::Exit | tauri::RunEvent::ExitRequested { .. }
        ) {
            shutdown_codex_app_server();
        }
    });
}

#[cfg(test)]
fn codex_thread_with_turns(client: &mut CodexWsClient, thread_json: &Value) -> Value {
    let mut enriched_thread = thread_json.clone();
    let Some(thread_id) = clean_option(string_at(thread_json, &["/id", "/sessionId"])) else {
        return enriched_thread;
    };

    PERFORMANCE_CODEX_TURN_REQUESTS.fetch_add(1, Ordering::Relaxed);
    let Ok(turns_response) = client.request(
        "thread/turns/list",
        json!({
            "threadId": thread_id,
            "limit": CODEX_TURN_FETCH_LIMIT
        }),
    ) else {
        return enriched_thread;
    };

    if let Some(turns) = turns_response.get("data").and_then(Value::as_array) {
        PERFORMANCE_CODEX_TURNS_RETURNED.fetch_add(turns.len() as u64, Ordering::Relaxed);
        if let Some(object) = enriched_thread.as_object_mut() {
            object.insert("turns".to_string(), Value::Array(turns.clone()));
        }
    }

    enriched_thread
}

#[cfg(test)]
fn read_codex_thread(
    thread_json: &Value,
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
) -> Option<AgentSession> {
    let turn_summary = codex_turn_summary_from_thread(thread_json);
    read_codex_thread_with_summary(thread_json, scan_time_ms, options, Some(&turn_summary))
}

fn read_codex_thread_with_summary(
    thread_json: &Value,
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
    turn_summary: Option<&CodexTurnSummary>,
) -> Option<AgentSession> {
    let thread_id = clean_option(
        string_at(thread_json, &["/id"]).or_else(|| string_at(thread_json, &["/sessionId"])),
    )?;
    let updated_ms = codex_thread_updated_ms(thread_json, scan_time_ms);
    if !is_active_session(updated_ms, scan_time_ms, options) {
        return None;
    }
    if is_codex_subagent_thread_json(thread_json) {
        return None;
    }

    let workspace_path = clean_option(string_at(thread_json, &["/cwd"])).map(str::to_string);
    let thread_path = clean_option(string_at(thread_json, &["/path"])).map(str::to_string);
    let preview = clean_option(string_at(thread_json, &["/preview"])).and_then(clean_preview_text);
    let turn_summary = turn_summary.cloned().unwrap_or_default();
    let last_user_raw = turn_summary.last_user_message.clone();
    let last_ai_raw = turn_summary.last_ai_message.clone();
    let message_count = turn_summary.message_count;
    let (last_user_message, last_user_message_truncated) = apply_user_preview_budget(last_user_raw);
    let (last_ai_message, last_ai_message_truncated, last_ai_message_excerpt_kind) =
        apply_ai_preview_budget(last_ai_raw);
    let title = clean_option(string_at(thread_json, &["/name"]))
        .and_then(title_candidate_from_text)
        .or_else(|| preview.as_deref().and_then(title_candidate_from_text))
        .unwrap_or_else(|| {
            fallback_session_title(
                "Codex",
                workspace_path.as_deref().unwrap_or("Codex"),
                thread_id,
            )
        });
    let status = codex_status_from_thread_with_turn_status(
        thread_json,
        updated_ms,
        scan_time_ms,
        turn_summary.latest_turn_status.as_deref(),
    );
    let session_resource = codex_thread_resource(thread_id);
    let workspace_hint = workspace_path
        .as_deref()
        .and_then(|path_text| path_segments_basename(&workspace_path_segments(path_text)))
        .unwrap_or_else(|| "Codex".to_string());
    let workspace_identity = build_workspace_identity(
        "codex",
        thread_id,
        &workspace_hint,
        workspace_path.as_deref(),
        thread_path.as_deref(),
        Some(&session_resource),
    );
    let mut todo_ids = Vec::new();
    for text in [
        Some(title.as_str()),
        preview.as_deref(),
        last_user_message.as_deref(),
        last_ai_message.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        collect_todo_ids_from_text(text, &mut todo_ids);
    }

    let plugin_workflow = detect_plugin_workflow_session_marker(
        "codex",
        &[last_user_message.as_deref(), last_ai_message.as_deref()],
    );

    Some(AgentSession {
        id: format!("codex:{}", thread_id),
        provider: "codex".to_string(),
        provider_label: "CX".to_string(),
        title,
        workspace: workspace_identity.name.clone(),
        workspace_path,
        workspace_key: workspace_identity.key,
        workspace_name: workspace_identity.name,
        workspace_label: workspace_identity.label,
        workspace_group: workspace_identity.group,
        workspace_discriminator: workspace_identity.discriminator,
        session_path: thread_path,
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
        todo_ids,
        plugin_workflow,
    })
}

fn read_codex_jsonl_session(
    session_path: &Path,
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
) -> Option<AgentSession> {
    let fingerprint = jsonl_fingerprint(session_path)?;
    let fallback_updated_ms = fallback_updated_ms(&fingerprint, scan_time_ms);
    if !is_active_session(fallback_updated_ms, scan_time_ms, options) {
        return None;
    }

    let summary = cached_or_read_codex_file_session(session_path, &fingerprint)?;
    if options.hide_archived && summary.archived {
        return None;
    }

    let CodexFileSessionSummary {
        session_id,
        is_subagent,
        workspace_path,
        title,
        latest_timestamp_ms,
        message_count,
        last_user_message,
        last_user_message_truncated,
        last_ai_message,
        last_ai_message_truncated,
        last_ai_message_excerpt_kind,
        todo_ids,
        ..
    } = summary;
    if is_subagent {
        return None;
    }

    let (updated_ms, activity_source) =
        latest_activity_ms(latest_timestamp_ms, fallback_updated_ms);
    if !is_active_session(updated_ms, scan_time_ms, options) {
        return None;
    }

    let status = status_from_activity(updated_ms, scan_time_ms, activity_source, None);
    let workspace_path = workspace_path.filter(|path_text| !path_text.trim().is_empty());
    let workspace = workspace_path
        .as_deref()
        .map(Path::new)
        .and_then(display_name_from_path)
        .or_else(|| session_path.parent().and_then(display_name_from_path))
        .unwrap_or_else(|| "Codex".to_string());
    let title = title.unwrap_or_else(|| fallback_session_title("Codex", &workspace, &session_id));
    let session_path_text = path_to_string(session_path);
    let session_resource = codex_thread_resource(&session_id);
    let workspace_identity = build_workspace_identity(
        "codex",
        &session_id,
        &workspace,
        workspace_path.as_deref(),
        Some(&session_path_text),
        Some(&session_resource),
    );
    let plugin_workflow = detect_plugin_workflow_session_marker(
        "codex",
        &[last_user_message.as_deref(), last_ai_message.as_deref()],
    );

    Some(AgentSession {
        id: format!("codex:{}", session_id),
        provider: "codex".to_string(),
        provider_label: "CX".to_string(),
        title,
        workspace: workspace_identity.name.clone(),
        workspace_path,
        workspace_key: workspace_identity.key,
        workspace_name: workspace_identity.name,
        workspace_label: workspace_identity.label,
        workspace_group: workspace_identity.group,
        workspace_discriminator: workspace_identity.discriminator,
        session_path: Some(session_path_text),
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
        todo_ids,
        plugin_workflow,
    })
}

fn cached_or_read_codex_file_session(
    session_path: &Path,
    fingerprint: &JsonlFingerprint,
) -> Option<CodexFileSessionSummary> {
    let path_key = path_to_string(session_path);
    if let Some(summary) = cached_jsonl_summary(&CODEX_FILE_SESSION_CACHE, &path_key, fingerprint) {
        return Some(summary);
    }

    let summary = read_codex_file_session_summary(session_path, fingerprint)?;
    store_jsonl_summary(
        &CODEX_FILE_SESSION_CACHE,
        path_key,
        fingerprint,
        summary.clone(),
    );
    Some(summary)
}

fn read_codex_file_session_summary(
    session_path: &Path,
    fingerprint: &JsonlFingerprint,
) -> Option<CodexFileSessionSummary> {
    let mut session_id =
        codex_session_id_from_path(session_path).unwrap_or_else(|| "unknown".to_string());
    let mut is_subagent = false;
    let mut workspace_path = None;
    let mut title = None;
    let mut latest_timestamp_ms = None;
    let mut first_user_message = None;
    let mut last_user_message = None;
    let mut last_ai_message = None;
    let mut todo_ids = Vec::new();
    let mut archived = false;
    let mut head_message_count = 0u32;
    let mut tail_message_count = 0u32;

    visit_jsonl_head_values(
        session_path,
        fingerprint.len,
        COPILOT_TITLE_SAMPLE_LINES,
        |_, json_value| {
            if is_archived_json(json_value) {
                archived = true;
            }
            latest_timestamp_ms =
                newest_timestamp_ms(latest_timestamp_ms, timestamp_ms_from_json(json_value));
            if string_at(json_value, &["/type"]) == Some("session_meta") {
                if let Some(next_session_id) =
                    string_at(json_value, &["/payload/id", "/payload/sessionId"])
                {
                    session_id = clean_label(next_session_id, 96);
                }
                if is_codex_subagent_metadata(
                    clean_option(string_at(
                        json_value,
                        &["/payload/thread_source", "/payload/threadSource"],
                    )),
                    clean_option(string_at(
                        json_value,
                        &["/payload/parent_thread_id", "/payload/parentThreadId"],
                    )),
                    json_value.pointer("/payload/source"),
                ) {
                    is_subagent = true;
                }
            }
            if workspace_path.is_none() {
                workspace_path = string_at(json_value, &["/payload/cwd", "/cwd"])
                    .filter(|path_text| !path_text.trim().is_empty())
                    .map(str::to_string);
            }
            if let Some(user_text) = codex_jsonl_user_text(json_value) {
                head_message_count = head_message_count.saturating_add(1);
                collect_todo_ids_from_text(&user_text, &mut todo_ids);
                if !is_codex_context_user_text(&user_text) {
                    if title.is_none() {
                        title = title_candidate_from_text(&user_text);
                    }
                    if first_user_message.is_none() {
                        first_user_message = Some(user_text);
                    }
                }
            } else if codex_jsonl_ai_text(json_value).is_some() {
                head_message_count = head_message_count.saturating_add(1);
            }
        },
    )?;

    let tail_covers_file = tail_sample_covers_file(fingerprint.len);
    let _ = visit_jsonl_tail_values(session_path, fingerprint.len, |json_value| {
        if is_archived_json(json_value) {
            archived = true;
        }
        latest_timestamp_ms =
            newest_timestamp_ms(latest_timestamp_ms, timestamp_ms_from_json(json_value));
        if let Some(user_text) = codex_jsonl_user_text(json_value) {
            tail_message_count = tail_message_count.saturating_add(1);
            collect_todo_ids_from_text(&user_text, &mut todo_ids);
            if !is_user_system_error_text(&user_text) && !is_codex_context_user_text(&user_text) {
                if title.is_none() {
                    title = title_candidate_from_text(&user_text);
                }
                last_user_message = Some(user_text);
            }
        } else if let Some(ai_text) = codex_jsonl_ai_text(json_value) {
            tail_message_count = tail_message_count.saturating_add(1);
            collect_todo_ids_from_text(&ai_text, &mut todo_ids);
            if !is_ai_model_noise_text(&ai_text) {
                last_ai_message = Some(ai_text);
            }
        }
    });

    let (last_user_message, last_user_message_truncated) =
        apply_user_preview_budget(last_user_message.or(first_user_message));
    let (last_ai_message, last_ai_message_truncated, last_ai_message_excerpt_kind) =
        apply_ai_preview_budget(last_ai_message);

    Some(CodexFileSessionSummary {
        session_id,
        is_subagent,
        workspace_path,
        title,
        latest_timestamp_ms,
        message_count: sampled_message_count(
            head_message_count,
            tail_message_count,
            tail_covers_file,
        ),
        last_user_message,
        last_user_message_truncated,
        last_ai_message,
        last_ai_message_truncated,
        last_ai_message_excerpt_kind,
        archived,
        todo_ids,
    })
}

fn codex_session_id_from_path(session_path: &Path) -> Option<String> {
    let file_name = file_stem(session_path)?;
    let uuid_tail = file_name
        .char_indices()
        .rev()
        .nth(35)
        .map(|(index, _)| &file_name[index..])?;
    if looks_like_uuid(uuid_tail) {
        Some(uuid_tail.to_string())
    } else {
        Some(clean_label(&file_name, 96))
    }
}

fn looks_like_uuid(text: &str) -> bool {
    if text.len() != 36 {
        return false;
    }
    text.chars().enumerate().all(|(index, ch)| {
        if matches!(index, 8 | 13 | 18 | 23) {
            ch == '-'
        } else {
            ch.is_ascii_hexdigit()
        }
    })
}

fn codex_jsonl_user_text(json_value: &Value) -> Option<String> {
    if string_at(json_value, &["/type"]) == Some("event_msg")
        && string_at(json_value, &["/payload/type"]) == Some("user_message")
    {
        return string_at(json_value, &["/payload/message"]).and_then(clean_preview_text);
    }

    let payload = json_value.pointer("/payload")?;
    if string_at(json_value, &["/type"]) == Some("response_item")
        && string_at(payload, &["/type"]) == Some("message")
        && string_at(payload, &["/role"]) == Some("user")
    {
        return codex_response_message_text(payload);
    }

    None
}

fn codex_jsonl_ai_text(json_value: &Value) -> Option<String> {
    if string_at(json_value, &["/type"]) == Some("event_msg") {
        match string_at(json_value, &["/payload/type"]) {
            Some("agent_message") => {
                return string_at(json_value, &["/payload/message"]).and_then(clean_preview_text);
            }
            Some("task_complete") => {
                return string_at(json_value, &["/payload/last_agent_message"])
                    .and_then(clean_preview_text);
            }
            _ => {}
        }
    }

    let payload = json_value.pointer("/payload")?;
    if string_at(json_value, &["/type"]) == Some("response_item")
        && string_at(payload, &["/type"]) == Some("message")
        && string_at(payload, &["/role"]) == Some("assistant")
    {
        return codex_response_message_text(payload);
    }

    None
}

fn codex_response_message_text(message_payload: &Value) -> Option<String> {
    if let Some(text) = string_at(message_payload, &["/content"]) {
        return clean_preview_text(text);
    }

    let content_items = json_array_at(message_payload, &["/content"])?;
    let mut parts = Vec::new();
    for content_item in content_items {
        match string_at(content_item, &["/type"]) {
            Some("input_text") | Some("output_text") | Some("text") => {
                if let Some(text) = string_at(content_item, &["/text"]) {
                    if let Some(clean) = clean_preview_text(text) {
                        parts.push(clean);
                    }
                }
            }
            _ => {}
        }
    }
    if parts.is_empty() {
        return None;
    }
    clean_preview_text(&parts.join("\n"))
}

fn is_codex_context_user_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("# AGENTS.md instructions")
        || trimmed.starts_with("<environment_context>")
        || trimmed.contains("\n<environment_context>")
        || trimmed.starts_with("Current task state:")
}

#[cfg(test)]
fn codex_turn_summary_from_thread(thread_json: &Value) -> CodexTurnSummary {
    let Some(turns) = thread_json.get("turns").and_then(Value::as_array) else {
        return CodexTurnSummary::default();
    };
    codex_turn_summary_from_turns(turns)
}

fn codex_turn_summary_from_turns(turns: &[Value]) -> CodexTurnSummary {
    let mut last_user_message = None;
    let mut last_ai_message = None;
    let mut message_count = 0u32;
    let mut latest_turn_status = None;

    let mut sorted_turns: Vec<&Value> = turns.iter().collect();
    sorted_turns.sort_by_key(|turn| codex_turn_timestamp_ms(turn).unwrap_or(0));
    for turn in sorted_turns {
        if let Some(status) = string_at(turn, &["/status"]) {
            latest_turn_status = Some(status.to_string());
        }
        if let Some(items) = turn.get("items").and_then(Value::as_array) {
            for item in items {
                match string_at(item, &["/type"]) {
                    Some("userMessage") => {
                        if let Some(text) = codex_user_message_text(item) {
                            last_user_message = Some(text);
                            message_count = message_count.saturating_add(1);
                        }
                    }
                    Some("agentMessage") => {
                        if let Some(text) = string_at(item, &["/text"]).and_then(clean_preview_text)
                        {
                            last_ai_message = Some(text);
                            message_count = message_count.saturating_add(1);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    CodexTurnSummary {
        last_user_message,
        last_ai_message,
        message_count,
        latest_turn_status,
        ..CodexTurnSummary::default()
    }
}

fn codex_turn_timestamp_ms(turn_json: &Value) -> Option<u64> {
    timestamp_ms_from_json(turn_json)
        .or_else(|| {
            turn_json
                .pointer("/startedAt")
                .and_then(timestamp_ms_from_value)
        })
        .or_else(|| {
            turn_json
                .pointer("/completedAt")
                .and_then(timestamp_ms_from_value)
        })
}

fn codex_user_message_text(item: &Value) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(content_items) = item.get("content").and_then(Value::as_array) {
        for content_item in content_items {
            if let Some(text) = string_at(content_item, &["/text"]) {
                if let Some(clean) = clean_preview_text(text) {
                    parts.push(clean);
                }
            }
        }
    }

    if parts.is_empty() {
        return string_at(item, &["/text"]).and_then(clean_preview_text);
    }

    clean_preview_text(&parts.join("\n"))
}

#[cfg(test)]
fn codex_status_from_thread(thread_json: &Value, updated_ms: u64, scan_time_ms: u64) -> String {
    let latest_turn_status = codex_latest_turn_status(thread_json).map(str::to_string);
    codex_status_from_thread_with_turn_status(
        thread_json,
        updated_ms,
        scan_time_ms,
        latest_turn_status.as_deref(),
    )
}

fn codex_status_from_thread_with_turn_status(
    thread_json: &Value,
    updated_ms: u64,
    scan_time_ms: u64,
    latest_turn_status: Option<&str>,
) -> String {
    let status_value = thread_json.get("status").unwrap_or(&Value::Null);
    let status_type = status_value
        .get("type")
        .and_then(Value::as_str)
        .or_else(|| status_value.as_str())
        .unwrap_or("idle");

    if status_type.eq_ignore_ascii_case("active") {
        let active_flags = status_value
            .get("activeFlags")
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if active_flags.iter().any(|flag| {
            flag.as_str()
                .map(|flag_text| {
                    flag_text.eq_ignore_ascii_case("waitingOnApproval")
                        || flag_text.eq_ignore_ascii_case("waitingOnUserInput")
                })
                .unwrap_or(false)
        }) {
            return "waiting".to_string();
        }
        return "running".to_string();
    }

    if let Some(latest_turn_status) = latest_turn_status {
        if codex_turn_status_is_waiting(latest_turn_status) {
            return "waiting".to_string();
        }
        if codex_turn_status_is_running(latest_turn_status) {
            return "running".to_string();
        }
        if codex_turn_status_is_finished(latest_turn_status) {
            return "idle".to_string();
        }
    }

    if status_type.eq_ignore_ascii_case("running") {
        "running".to_string()
    } else if status_type.eq_ignore_ascii_case("waiting") {
        "waiting".to_string()
    } else if scan_time_ms.saturating_sub(updated_ms) <= STATUS_RECENT_CONTENT_WINDOW_MS {
        "running".to_string()
    } else {
        "idle".to_string()
    }
}

#[cfg(test)]
fn codex_latest_turn_status(thread_json: &Value) -> Option<&str> {
    let turns = thread_json.get("turns").and_then(Value::as_array)?;
    turns
        .iter()
        .max_by_key(|turn| codex_turn_timestamp_ms(turn).unwrap_or(0))
        .and_then(|turn| string_at(turn, &["/status"]))
}

fn codex_turn_status_is_waiting(status_text: &str) -> bool {
    matches!(
        status_text.to_ascii_lowercase().as_str(),
        "waiting" | "waiting_on_user" | "waiting_on_user_input" | "waiting_on_approval"
    )
}

fn codex_turn_status_is_running(status_text: &str) -> bool {
    matches!(
        status_text.to_ascii_lowercase().as_str(),
        "active" | "running" | "in_progress" | "queued" | "pending"
    )
}

fn codex_turn_status_is_finished(status_text: &str) -> bool {
    matches!(
        status_text.to_ascii_lowercase().as_str(),
        "completed" | "failed" | "cancelled" | "canceled"
    )
}

fn codex_thread_resource(thread_id: &str) -> String {
    format!("codex://threads/{}", percent_encode_path_segment(thread_id))
}

fn scan_opencode_sessions(scan_time_ms: u64, options: &ResolvedScanOptions) -> Vec<AgentSession> {
    let Some(db_path) = opencode_db_path() else {
        return Vec::new();
    };
    if !db_path.exists() {
        return Vec::new();
    }
    if let Some(cached_sessions) = cached_opencode_scan(scan_time_ms, options) {
        return cached_sessions;
    }

    let Ok(connection) = Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return Vec::new();
    };
    let _ = connection.busy_timeout(Duration::from_millis(OPENCODE_SQLITE_BUSY_TIMEOUT_MS));
    let _ = connection.pragma_update(None, "query_only", "ON");
    let _ = connection.pragma_update(None, "temp_store", "MEMORY");

    let mut statement = match connection.prepare(
        "select id, title, directory, path, time_created, time_updated, time_archived \
         from session \
         where (?2 = 0 or time_updated >= ?2 or time_created >= ?2) \
           and (?3 = 0 or time_archived is null) \
         order by time_updated desc \
         limit ?1",
    ) {
        Ok(statement) => statement,
        Err(_) => return Vec::new(),
    };

    let active_cutoff_ms = scan_time_ms.saturating_sub(options.active_window_ms) as i64;
    let hide_archived = if options.hide_archived { 1_i64 } else { 0_i64 };
    let rows = match statement.query_map(
        rusqlite::params![
            opencode_session_scan_limit(options) as i64,
            active_cutoff_ms,
            hide_archived
        ],
        |row| {
            let time_archived: Option<i64> = row.get(6)?;
            Ok(OpenCodeSessionRow {
                id: row.get(0)?,
                title: row.get(1)?,
                directory: row.get(2)?,
                path: row.get(3)?,
                time_created: sqlite_millis_to_u64(row.get::<_, i64>(4)?),
                time_updated: sqlite_millis_to_u64(row.get::<_, i64>(5)?),
                time_archived: time_archived.map(sqlite_millis_to_u64),
            })
        },
    ) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();
    let mut summary_fetch_budget = OPENCODE_SUMMARY_FETCH_BUDGET.min(options.max_sessions);
    for row in rows.filter_map(Result::ok) {
        if options.hide_archived && row.time_archived.is_some() {
            continue;
        }
        let row_updated_ms = row.time_updated.max(row.time_created);
        if !is_active_session(row_updated_ms, scan_time_ms, options) {
            continue;
        }
        let summary =
            cached_or_read_opencode_session_summary(&connection, &row, &mut summary_fetch_budget);
        if let Some(session) = read_opencode_session_row(&row, summary, scan_time_ms, options) {
            sessions.push(session);
            if sessions.len() >= options.max_sessions {
                break;
            }
        }
    }

    cache_opencode_scan(scan_time_ms, options, &sessions);
    sessions
}

fn opencode_db_path() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|user_profile| {
            user_profile
                .join(".local")
                .join("share")
                .join("opencode")
                .join("opencode.db")
        })
}

fn opencode_session_scan_limit(options: &ResolvedScanOptions) -> usize {
    options
        .max_sessions
        .saturating_mul(3)
        .max(options.max_sessions + 20)
        .clamp(40, OPENCODE_SESSION_SCAN_LIMIT_MAX)
}

fn sqlite_millis_to_u64(value: i64) -> u64 {
    if value > 0 {
        value as u64
    } else {
        0
    }
}

fn cached_opencode_scan(
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
) -> Option<Vec<AgentSession>> {
    let key = opencode_scan_cache_key(options);
    OPENCODE_SCAN_CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|cache| cache.clone())
        .filter(|cached| {
            cached.key == key
                && scan_time_ms.saturating_sub(cached.captured_ms) <= OPENCODE_SCAN_CACHE_TTL_MS
        })
        .map(|cached| refresh_cached_session_time_labels(&cached.sessions, scan_time_ms))
}

fn cache_opencode_scan(
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
    sessions: &[AgentSession],
) {
    if let Ok(mut cache) = OPENCODE_SCAN_CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = Some(CachedOpenCodeScan {
            captured_ms: scan_time_ms,
            key: opencode_scan_cache_key(options),
            sessions: sessions.to_vec(),
        });
    }
}

fn opencode_scan_cache_key(options: &ResolvedScanOptions) -> OpenCodeScanCacheKey {
    OpenCodeScanCacheKey {
        max_sessions: options.max_sessions,
        active_window_ms: options.active_window_ms,
        hide_archived: options.hide_archived,
    }
}

fn clear_opencode_scan_cache() {
    if let Ok(mut cache) = OPENCODE_SCAN_CACHE.get_or_init(|| Mutex::new(None)).lock() {
        *cache = None;
    }
}

fn refresh_cached_session_time_labels(
    sessions: &[AgentSession],
    scan_time_ms: u64,
) -> Vec<AgentSession> {
    sessions
        .iter()
        .cloned()
        .map(|mut session| {
            session.time_label = time_label(session.updated_ms, scan_time_ms);
            session
        })
        .collect()
}

fn cached_or_read_opencode_session_summary(
    connection: &Connection,
    row: &OpenCodeSessionRow,
    summary_fetch_budget: &mut usize,
) -> OpenCodeSessionSummary {
    if let Some(summary) = cached_opencode_session_summary(&row.id, row.time_updated) {
        return summary;
    }

    if *summary_fetch_budget == 0 {
        return lightweight_opencode_session_summary(row);
    }
    *summary_fetch_budget = summary_fetch_budget.saturating_sub(1);

    let summary = opencode_session_summary(connection, &row.id);
    cache_opencode_session_summary(&row.id, row.time_updated, summary.clone());
    summary
}

fn lightweight_opencode_session_summary(row: &OpenCodeSessionRow) -> OpenCodeSessionSummary {
    OpenCodeSessionSummary {
        latest_part_ms: row.time_updated.max(row.time_created),
        ..OpenCodeSessionSummary::default()
    }
}

fn cached_opencode_session_summary(
    session_id: &str,
    session_updated_ms: u64,
) -> Option<OpenCodeSessionSummary> {
    OPENCODE_SESSION_SUMMARY_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .ok()
        .and_then(|cache| cache.get(session_id).cloned())
        .filter(|cached| cached.session_updated_ms == session_updated_ms)
        .map(|cached| cached.summary)
}

fn cache_opencode_session_summary(
    session_id: &str,
    session_updated_ms: u64,
    summary: OpenCodeSessionSummary,
) {
    if let Ok(mut cache) = OPENCODE_SESSION_SUMMARY_CACHE
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
    {
        cache.insert(
            session_id.to_string(),
            CachedOpenCodeSummary {
                session_updated_ms,
                summary,
            },
        );
        if cache.len() > 512 {
            let mut entries: Vec<(String, u64)> = cache
                .iter()
                .map(|(cached_session_id, cached)| {
                    (cached_session_id.clone(), cached.session_updated_ms)
                })
                .collect();
            entries.sort_by_key(|(_, updated_ms)| *updated_ms);
            let prune_count = cache.len().saturating_sub(512);
            for (cached_session_id, _) in entries.into_iter().take(prune_count) {
                cache.remove(&cached_session_id);
            }
        }
    }
}

fn read_opencode_session_row(
    row: &OpenCodeSessionRow,
    summary: OpenCodeSessionSummary,
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
) -> Option<AgentSession> {
    let updated_ms = row
        .time_updated
        .max(row.time_created)
        .max(summary.latest_part_ms);
    if !is_active_session(updated_ms, scan_time_ms, options) {
        return None;
    }

    let workspace_path = clean_option(Some(row.directory.as_str())).map(str::to_string);
    let session_path = row
        .path
        .as_deref()
        .and_then(|path_text| clean_option(Some(path_text)))
        .map(str::to_string);
    let session_resource = opencode_session_resource(&row.id);
    let workspace_hint = workspace_path
        .as_deref()
        .and_then(|path_text| path_segments_basename(&workspace_path_segments(path_text)))
        .or_else(|| {
            session_path
                .as_deref()
                .and_then(|path_text| path_segments_basename(&workspace_path_segments(path_text)))
        })
        .unwrap_or_else(|| "OpenCode".to_string());
    let title = clean_option(Some(row.title.as_str()))
        .and_then(title_candidate_from_text)
        .or_else(|| {
            summary
                .last_user_message
                .as_deref()
                .and_then(title_candidate_from_text)
        })
        .unwrap_or_else(|| fallback_session_title("OpenCode", &workspace_hint, &row.id));
    let status = status_from_activity(
        updated_ms,
        scan_time_ms,
        ActivitySource::ContentTimestamp,
        summary.status_hint,
    );
    let workspace_identity = build_workspace_identity(
        "opencode",
        &row.id,
        &workspace_hint,
        workspace_path.as_deref(),
        session_path.as_deref(),
        Some(&session_resource),
    );
    let mut todo_ids = summary.todo_ids;
    collect_todo_ids_from_text(&title, &mut todo_ids);
    let (last_user_message, last_user_message_truncated) =
        apply_user_preview_budget(summary.last_user_message);
    let (last_ai_message, last_ai_message_truncated, last_ai_message_excerpt_kind) =
        apply_ai_preview_budget(summary.last_ai_message);

    let plugin_workflow = detect_plugin_workflow_session_marker(
        "opencode",
        &[last_user_message.as_deref(), last_ai_message.as_deref()],
    );

    Some(AgentSession {
        id: format!("opencode:{}", row.id),
        provider: "opencode".to_string(),
        provider_label: "OC".to_string(),
        title,
        workspace: workspace_identity.name.clone(),
        workspace_path,
        workspace_key: workspace_identity.key,
        workspace_name: workspace_identity.name,
        workspace_label: workspace_identity.label,
        workspace_group: workspace_identity.group,
        workspace_discriminator: workspace_identity.discriminator,
        session_path,
        session_resource: Some(session_resource),
        status,
        time_label: time_label(updated_ms, scan_time_ms),
        updated_ms,
        message_count: summary.message_count,
        last_user_message,
        last_user_message_truncated,
        last_ai_message,
        last_ai_message_truncated,
        last_ai_message_excerpt_kind,
        branch: None,
        todo_ids,
        plugin_workflow,
    })
}

fn opencode_session_summary(connection: &Connection, session_id: &str) -> OpenCodeSessionSummary {
    let mut summary = OpenCodeSessionSummary::default();
    let mut message_json_by_id: HashMap<String, Value> = HashMap::new();

    if let Ok(mut statement) = connection.prepare(
        "select id, time_updated, data \
         from message \
         where session_id = ?1 \
         order by time_updated desc \
         limit ?2",
    ) {
        if let Ok(rows) = statement.query_map(
            rusqlite::params![session_id, OPENCODE_PART_SCAN_LIMIT as i64],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    sqlite_millis_to_u64(row.get::<_, i64>(1)?),
                    row.get::<_, String>(2)?,
                ))
            },
        ) {
            for (message_id, time_updated, message_data) in rows.filter_map(Result::ok) {
                summary.latest_part_ms = summary.latest_part_ms.max(time_updated);
                summary.message_count = summary.message_count.saturating_add(1);
                if let Ok(message_json) = serde_json::from_str::<Value>(&message_data) {
                    merge_session_status_hint(
                        &mut summary.status_hint,
                        opencode_message_status_hint(&message_json),
                    );
                    message_json_by_id.insert(message_id, message_json);
                }
            }
        }
    }

    if let Ok(mut statement) = connection.prepare(
        "select message_id, time_updated, data \
         from part \
         where session_id = ?1 \
         order by time_updated desc \
         limit ?2",
    ) {
        if let Ok(rows) = statement.query_map(
            rusqlite::params![session_id, OPENCODE_PART_SCAN_LIMIT as i64],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    sqlite_millis_to_u64(row.get::<_, i64>(1)?),
                    row.get::<_, String>(2)?,
                ))
            },
        ) {
            for (message_id, time_updated, part_data) in rows.filter_map(Result::ok) {
                summary.latest_part_ms = summary.latest_part_ms.max(time_updated);
                let Ok(part_json) = serde_json::from_str::<Value>(&part_data) else {
                    continue;
                };
                let message_json = message_json_by_id.get(&message_id);
                merge_session_status_hint(
                    &mut summary.status_hint,
                    opencode_part_status_hint(&part_json),
                );

                if let Some(text) = string_at(&part_json, &["/text"]) {
                    collect_todo_ids_from_text(text, &mut summary.todo_ids);
                }

                if string_at(&part_json, &["/type"]) != Some("text") {
                    continue;
                }
                let Some(text) = string_at(&part_json, &["/text"]).and_then(clean_preview_text)
                else {
                    continue;
                };
                match message_json.and_then(|message| string_at(message, &["/role"])) {
                    Some("user") if summary.last_user_message.is_none() => {
                        summary.last_user_message = Some(text);
                    }
                    Some("assistant")
                        if summary.last_ai_message.is_none() && !is_ai_model_noise_text(&text) =>
                    {
                        summary.last_ai_message = Some(text);
                    }
                    _ => {}
                }
            }
        }
    }

    summary
}

fn prepare_opencode_handoff_source_context(
    request: &HandoffSourceContextRequest,
) -> Result<HandoffSourceContext, String> {
    let session_id = raw_request_session_id(request.id.as_deref())
        .map(str::to_string)
        .or_else(|| {
            request
                .session_resource
                .as_deref()
                .and_then(opencode_session_id_from_resource)
        })
        .ok_or_else(|| "Missing OpenCode source session id".to_string())?;
    let db_path = opencode_db_path()
        .filter(|path| path.exists())
        .ok_or_else(|| "OpenCode database not found".to_string())?;
    let connection = Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| format!("Failed to open OpenCode database: {}", error))?;
    let _ = connection.busy_timeout(Duration::from_millis(OPENCODE_SQLITE_BUSY_TIMEOUT_MS));
    let _ = connection.pragma_update(None, "query_only", "ON");
    let row = read_opencode_session_metadata(&connection, &session_id, request)?;
    let messages = read_opencode_transcript_messages(&connection, &session_id)?;
    let markdown = render_opencode_handoff_source_markdown(&row, &messages);
    let path = write_opencode_handoff_source_markdown(&row, &markdown)?;
    let inline_summary = opencode_handoff_inline_summary(&row, &messages);

    Ok(HandoffSourceContext {
        provider: "opencode".to_string(),
        session_id: Some(session_id),
        primary_source_file: Some(path_to_cli_string(&path)),
        inline_summary,
        message_count: messages.len() as u32,
        warning: None,
    })
}

fn read_opencode_session_metadata(
    connection: &Connection,
    session_id: &str,
    request: &HandoffSourceContextRequest,
) -> Result<OpenCodeSessionRow, String> {
    let mut statement = connection
        .prepare(
            "select id, title, directory, path, time_created, time_updated, time_archived \
             from session \
             where id = ?1 \
             limit 1",
        )
        .map_err(|error| format!("Failed to prepare OpenCode session query: {}", error))?;

    let result = statement.query_row(rusqlite::params![session_id], |row| {
        let time_archived: Option<i64> = row.get(6)?;
        Ok(OpenCodeSessionRow {
            id: row.get(0)?,
            title: row.get(1)?,
            directory: row.get(2)?,
            path: row.get(3)?,
            time_created: sqlite_millis_to_u64(row.get::<_, i64>(4)?),
            time_updated: sqlite_millis_to_u64(row.get::<_, i64>(5)?),
            time_archived: time_archived.map(sqlite_millis_to_u64),
        })
    });

    match result {
        Ok(row) => Ok(row),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(OpenCodeSessionRow {
            id: session_id.to_string(),
            title: request
                .title
                .as_deref()
                .and_then(|title| clean_option(Some(title)))
                .unwrap_or("OpenCode source session")
                .to_string(),
            directory: request
                .workspace_path
                .as_deref()
                .and_then(|path| clean_option(Some(path)))
                .unwrap_or("")
                .to_string(),
            path: request
                .session_path
                .as_deref()
                .and_then(|path| clean_option(Some(path)))
                .map(str::to_string),
            time_created: current_time_ms(),
            time_updated: current_time_ms(),
            time_archived: None,
        }),
        Err(error) => Err(format!(
            "Failed to read OpenCode session metadata: {}",
            error
        )),
    }
}

fn read_opencode_transcript_messages(
    connection: &Connection,
    session_id: &str,
) -> Result<Vec<OpenCodeTranscriptMessage>, String> {
    let mut messages = Vec::new();
    let mut message_index_by_id = HashMap::new();
    let mut statement = connection
        .prepare(
            "select id, time_created, time_updated, data \
             from message \
             where session_id = ?1 \
             order by time_created asc, time_updated asc, id asc",
        )
        .map_err(|error| format!("Failed to prepare OpenCode message query: {}", error))?;
    let rows = statement
        .query_map(rusqlite::params![session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                sqlite_millis_to_u64(row.get::<_, i64>(1)?),
                sqlite_millis_to_u64(row.get::<_, i64>(2)?),
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| format!("Failed to read OpenCode messages: {}", error))?;

    for (message_id, time_created, time_updated, message_data) in rows.filter_map(Result::ok) {
        let role = serde_json::from_str::<Value>(&message_data)
            .ok()
            .and_then(|json| string_at(&json, &["/role"]).map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string());
        message_index_by_id.insert(message_id.clone(), messages.len());
        messages.push(OpenCodeTranscriptMessage {
            id: message_id,
            role,
            time_ms: time_updated.max(time_created),
            parts: Vec::new(),
        });
    }

    let mut part_statement = connection
        .prepare(
            "select message_id, time_created, time_updated, data \
             from part \
             where session_id = ?1 \
             order by time_created asc, time_updated asc, id asc",
        )
        .map_err(|error| format!("Failed to prepare OpenCode part query: {}", error))?;
    let part_rows = part_statement
        .query_map(rusqlite::params![session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                sqlite_millis_to_u64(row.get::<_, i64>(1)?),
                sqlite_millis_to_u64(row.get::<_, i64>(2)?),
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|error| format!("Failed to read OpenCode parts: {}", error))?;

    for (message_id, time_created, time_updated, part_data) in part_rows.filter_map(Result::ok) {
        let Some(part) = opencode_transcript_part_from_data(&part_data) else {
            continue;
        };
        if let Some(index) = message_index_by_id.get(&message_id).copied() {
            messages[index].time_ms = messages[index].time_ms.max(time_updated.max(time_created));
            messages[index].parts.push(part);
        }
    }

    Ok(messages)
}

fn opencode_transcript_part_from_data(part_data: &str) -> Option<OpenCodeTranscriptPart> {
    let part_json = serde_json::from_str::<Value>(part_data).ok()?;
    let part_type = string_at(&part_json, &["/type"]).unwrap_or("unknown");
    match part_type {
        "text" => string_at(&part_json, &["/text"])
            .and_then(clean_handoff_part_text)
            .map(|text| OpenCodeTranscriptPart {
                label: "text".to_string(),
                text,
            }),
        "tool" => opencode_tool_part_handoff_text(&part_json).map(|text| OpenCodeTranscriptPart {
            label: "tool".to_string(),
            text,
        }),
        "reasoning" => Some(OpenCodeTranscriptPart {
            label: "reasoning".to_string(),
            text: "[reasoning omitted from portable handoff context]".to_string(),
        }),
        "step-start" | "step-finish" => Some(OpenCodeTranscriptPart {
            label: part_type.to_string(),
            text: opencode_compact_json_for_handoff(&part_json, 2_000),
        }),
        _ => Some(OpenCodeTranscriptPart {
            label: part_type.to_string(),
            text: opencode_compact_json_for_handoff(&part_json, 8_000),
        }),
    }
}

fn opencode_tool_part_handoff_text(part_json: &Value) -> Option<String> {
    let tool = string_at(part_json, &["/tool"]).unwrap_or("unknown");
    let status = string_at(part_json, &["/state/status"]).unwrap_or("unknown");
    let call_id = string_at(part_json, &["/callID"]).unwrap_or("");
    let mut lines = vec![format!(
        "[tool: {} status={} callID={}]",
        tool, status, call_id
    )];

    if let Some(input) = part_json.pointer("/state/input") {
        if !input.is_null() {
            lines.push("input:".to_string());
            lines.push(opencode_compact_json_for_handoff(input, 12_000));
        }
    }
    if let Some(error) = string_at(part_json, &["/state/error"]).and_then(clean_handoff_part_text) {
        lines.push("error:".to_string());
        lines.push(error);
    }
    if let Some(raw) = string_at(part_json, &["/state/raw"]).and_then(clean_handoff_part_text) {
        lines.push("raw output:".to_string());
        lines.push(raw);
    }

    clean_handoff_part_text(&lines.join("\n"))
}

fn clean_handoff_part_text(raw_text: &str) -> Option<String> {
    let cleaned = clean_preview_text(raw_text)?;
    Some(limit_chars(&cleaned, OPENCODE_HANDOFF_PART_MAX_CHARS))
}

fn opencode_compact_json_for_handoff(value: &Value, max_chars: usize) -> String {
    let text = serde_json::to_string(value).unwrap_or_else(|_| value.to_string());
    limit_chars(&text, max_chars)
}

fn limit_chars(text: &str, max_chars: usize) -> String {
    let mut output = String::new();
    let mut truncated = false;
    for (index, character) in text.chars().enumerate() {
        if index >= max_chars {
            truncated = true;
            break;
        }
        output.push(character);
    }
    if truncated {
        output.push_str("\n[truncated]");
    }
    output
}

fn render_opencode_handoff_source_markdown(
    row: &OpenCodeSessionRow,
    messages: &[OpenCodeTranscriptMessage],
) -> String {
    let mut output = String::new();
    output.push_str("# AgentWatcher OpenCode Source Session\n\n");
    output.push_str("This file is generated by AgentWatcher for cross-provider handoff. Source session A is reference context only; the new task in session B has priority.\n\n");
    output.push_str("## Metadata\n\n");
    output.push_str("- Provider: OpenCode\n");
    output.push_str(&format!("- Session ID: {}\n", row.id));
    output.push_str(&format!("- Title: {}\n", row.title));
    output.push_str(&format!("- Workspace: {}\n", row.directory));
    output.push_str(&format!(
        "- Session resource: {}\n",
        opencode_session_resource(&row.id)
    ));
    output.push_str(&format!("- Message count: {}\n", messages.len()));
    output.push_str("\n## Transcript\n\n");

    if messages.is_empty() {
        output.push_str(
            "_No OpenCode message/part rows were readable from the local SQLite database._\n",
        );
        return output;
    }

    for message in messages {
        output.push_str(&format!(
            "### {} · {} · {}\n\n",
            message.role, message.id, message.time_ms
        ));
        if message.parts.is_empty() {
            output.push_str("_No readable parts._\n\n");
            continue;
        }
        for part in &message.parts {
            output.push_str(&format!("#### {}\n\n", part.label));
            output.push_str(&part.text);
            output.push_str("\n\n");
        }
    }

    output
}

fn write_opencode_handoff_source_markdown(
    row: &OpenCodeSessionRow,
    markdown: &str,
) -> Result<PathBuf, String> {
    let dir = env::temp_dir()
        .join("AgentWatcher")
        .join("handoff-sources")
        .join("opencode");
    fs::create_dir_all(&dir)
        .map_err(|error| format!("Failed to prepare handoff source directory: {}", error))?;
    let file_name = format!(
        "{}-{}.md",
        safe_file_name_token(&row.id),
        row.time_updated.max(row.time_created)
    );
    let path = dir.join(file_name);
    let temp_path = path.with_extension("md.tmp");
    fs::write(&temp_path, markdown)
        .map_err(|error| format!("Failed to write OpenCode handoff source: {}", error))?;
    if path.exists() {
        fs::remove_file(&path)
            .map_err(|error| format!("Failed to replace OpenCode handoff source: {}", error))?;
    }
    fs::rename(&temp_path, &path)
        .map_err(|error| format!("Failed to finalize OpenCode handoff source: {}", error))?;
    Ok(path)
}

fn opencode_handoff_inline_summary(
    row: &OpenCodeSessionRow,
    messages: &[OpenCodeTranscriptMessage],
) -> Option<String> {
    let last_user = messages
        .iter()
        .rev()
        .find(|message| message.role == "user")
        .and_then(|message| message.parts.iter().find(|part| part.label == "text"))
        .map(|part| limit_chars(&part.text, 700));
    let last_assistant = messages
        .iter()
        .rev()
        .find(|message| message.role == "assistant")
        .and_then(|message| message.parts.iter().find(|part| part.label == "text"))
        .map(|part| limit_chars(&part.text, 1_200));
    let mut lines = vec![
        format!(
            "OpenCode source file was exported from SQLite for session {}.",
            row.id
        ),
        format!("Exported message count: {}.", messages.len()),
    ];
    if let Some(last_user) = last_user {
        lines.push(format!("Latest user text in export: {}", last_user));
    }
    if let Some(last_assistant) = last_assistant {
        lines.push(format!(
            "Latest assistant text in export: {}",
            last_assistant
        ));
    }
    clean_preview_text(&lines.join("\n"))
}

fn merge_session_status_hint(
    current_hint: &mut Option<SessionStatusHint>,
    candidate_hint: Option<SessionStatusHint>,
) {
    match candidate_hint {
        Some(SessionStatusHint::Waiting) => *current_hint = Some(SessionStatusHint::Waiting),
        Some(SessionStatusHint::Running) if current_hint.is_none() => {
            *current_hint = Some(SessionStatusHint::Running)
        }
        _ => {}
    }
}

fn opencode_message_status_hint(message_json: &Value) -> Option<SessionStatusHint> {
    if string_at(message_json, &["/role"]) == Some("assistant")
        && !opencode_assistant_message_finished(message_json)
    {
        Some(SessionStatusHint::Running)
    } else {
        None
    }
}

fn opencode_assistant_message_finished(message_json: &Value) -> bool {
    message_json.pointer("/time/completed").is_some()
        || clean_option(string_at(message_json, &["/finish"])).is_some()
        || message_json.pointer("/error").is_some()
}

fn opencode_part_status_hint(part_json: &Value) -> Option<SessionStatusHint> {
    let part_type = string_at(part_json, &["/type"])
        .unwrap_or_default()
        .to_ascii_lowercase();
    let state_status = string_at(part_json, &["/state/status", "/status"])
        .unwrap_or_default()
        .to_ascii_lowercase();

    if opencode_is_waiting_status(&state_status) {
        return Some(SessionStatusHint::Waiting);
    }
    if opencode_is_running_status(&state_status) {
        return if opencode_part_mentions_user_control(part_json) {
            Some(SessionStatusHint::Waiting)
        } else {
            Some(SessionStatusHint::Running)
        };
    }

    if part_type == "tool" && !opencode_tool_part_finished(part_json) {
        if opencode_part_mentions_user_control(part_json) {
            Some(SessionStatusHint::Waiting)
        } else {
            Some(SessionStatusHint::Running)
        }
    } else {
        None
    }
}

fn opencode_is_waiting_status(status_text: &str) -> bool {
    matches!(
        status_text,
        "waiting" | "blocked" | "needs_user_input" | "requires_action" | "requires_approval"
    )
}

fn opencode_is_running_status(status_text: &str) -> bool {
    matches!(
        status_text,
        "pending" | "queued" | "running" | "started" | "in_progress" | "submitted"
    )
}

fn opencode_tool_part_finished(part_json: &Value) -> bool {
    let status = string_at(part_json, &["/state/status", "/status"])
        .unwrap_or_default()
        .to_ascii_lowercase();
    matches!(
        status.as_str(),
        "completed" | "success" | "failed" | "error" | "cancelled" | "canceled" | "skipped"
    ) || part_json.pointer("/state/time/end").is_some()
        || part_json.pointer("/time/end").is_some()
}

fn opencode_part_mentions_user_control(part_json: &Value) -> bool {
    for text in [
        string_at(part_json, &["/type"]),
        string_at(part_json, &["/tool"]),
        string_at(part_json, &["/state/title", "/title"]),
        string_at(part_json, &["/permission"]),
        string_at(part_json, &["/state/permission"]),
    ]
    .into_iter()
    .flatten()
    {
        let lower_text = text.to_ascii_lowercase();
        if lower_text.contains("permission")
            || lower_text.contains("approval")
            || lower_text.contains("question")
            || lower_text.contains("ask")
            || lower_text.contains("control")
        {
            return true;
        }
    }
    false
}

fn opencode_session_resource(session_id: &str) -> String {
    format!(
        "opencode://sessions/{}",
        percent_encode_path_segment(session_id)
    )
}

fn open_codex_session(session: &OpenSessionRequest) -> Result<(), String> {
    if let Some(resource) = clean_option(session.session_resource.as_deref()) {
        if resource.starts_with("codex://") {
            match open_windows_url(resource) {
                Ok(()) => return Ok(()),
                Err(open_error) => {
                    if let Some(workspace_path) = clean_option(session.workspace_path.as_deref()) {
                        return open_codex_workspace(workspace_path).map_err(|fallback_error| {
                            format!(
                                "Codex thread launch failed ({}); workspace fallback failed ({})",
                                open_error, fallback_error
                            )
                        });
                    }
                    return Err(open_error);
                }
            }
        }
    }

    if let Some(raw_id) = raw_request_session_id(session.id.as_deref()) {
        let link = codex_thread_resource(raw_id);
        match open_windows_url(&link) {
            Ok(()) => return Ok(()),
            Err(open_error) => {
                if let Some(workspace_path) = clean_option(session.workspace_path.as_deref()) {
                    return open_codex_workspace(workspace_path).map_err(|fallback_error| {
                        format!(
                            "Codex thread launch failed ({}); workspace fallback failed ({})",
                            open_error, fallback_error
                        )
                    });
                }
                return Err(open_error);
            }
        }
    }

    open_codex_workspace(
        clean_option(session.workspace_path.as_deref())
            .ok_or_else(|| "Missing Codex workspace path".to_string())?,
    )
}

fn launch_codex_handoff(workspace_path: &str, prompt: &str) -> Result<String, String> {
    let thread_id = with_codex_app_server_client(|client| {
        let start_response = client.request(
            "thread/start",
            json!({
                "cwd": workspace_path,
                "threadSource": "user"
            }),
        )?;
        let thread_id = string_at(&start_response, &["/thread/id"])
            .ok_or_else(|| "Codex app-server did not return a thread id".to_string())?
            .to_string();
        client.request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [
                    {
                        "type": "text",
                        "text": prompt
                    }
                ]
            }),
        )?;
        Ok(thread_id)
    })?;

    let _ = open_windows_url(&codex_thread_resource(&thread_id));
    Ok(thread_id)
}

fn open_opencode_session(session: &OpenSessionRequest) -> Result<(), String> {
    let workspace_path = clean_option(session.workspace_path.as_deref())
        .ok_or_else(|| "Missing OpenCode workspace path".to_string())?;
    let session_id = raw_request_session_id(session.id.as_deref())
        .map(str::to_string)
        .or_else(|| {
            clean_option(session.session_resource.as_deref())
                .and_then(opencode_session_id_from_resource)
        })
        .ok_or_else(|| "Missing OpenCode session id".to_string())?;

    let port = ensure_opencode_web_server()?;
    open_windows_url(&opencode_web_session_url(port, workspace_path, &session_id))
        .map_err(|error| format!("Failed to open OpenCode Web session: {}", error))
}

fn opencode_session_id_from_resource(resource: &str) -> Option<String> {
    let encoded_id = resource.strip_prefix("opencode://sessions/")?;
    clean_option(Some(&percent_decode(encoded_id))).map(str::to_string)
}

fn opencode_web_session_url(port: u16, workspace_path: &str, session_id: &str) -> String {
    format!(
        "http://127.0.0.1:{}/{}/session/{}",
        port,
        opencode_web_directory_slug(workspace_path),
        percent_encode_path_segment(session_id)
    )
}

fn opencode_web_directory_slug(workspace_path: &str) -> String {
    base64_url_no_padding(workspace_path.as_bytes())
}

fn ensure_opencode_web_server() -> Result<u16, String> {
    if let Some(port) = current_owned_opencode_web_server_port() {
        return Ok(port);
    }

    for port in discover_opencode_web_server_ports() {
        if probe_opencode_web_server(port) {
            return Ok(port);
        }
    }

    start_owned_opencode_web_server()
}

fn current_owned_opencode_web_server_port() -> Option<u16> {
    let mut server_guard = OPENCODE_WEB_SERVER
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()?;
    let server = server_guard.as_mut()?;
    if server.child.try_wait().ok().flatten().is_some() {
        *server_guard = None;
        return None;
    }
    if probe_opencode_web_server(server.port) {
        Some(server.port)
    } else {
        None
    }
}

fn start_owned_opencode_web_server() -> Result<u16, String> {
    let opencode_cli =
        find_opencode_cli_path().ok_or_else(|| "OpenCode CLI not found".to_string())?;
    let port = reserve_localhost_port()?;
    let mut command = Command::new(opencode_cli);
    command.args([
        "serve",
        "--hostname",
        "127.0.0.1",
        "--port",
        &port.to_string(),
    ]);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    let mut child = spawn_hidden(command)
        .map_err(|error| format!("Failed to start OpenCode Web server: {}", error))?;

    let start = Instant::now();
    while start.elapsed() <= Duration::from_millis(6_000) {
        if probe_opencode_web_server(port) {
            if let Ok(mut server_guard) =
                OPENCODE_WEB_SERVER.get_or_init(|| Mutex::new(None)).lock()
            {
                *server_guard = Some(OpenCodeWebServerProcess { port, child });
            }
            return Ok(port);
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    let _ = child.kill();
    let _ = child.wait();
    Err(format!(
        "OpenCode Web server did not become ready on port {}",
        port
    ))
}

fn reserve_localhost_port() -> Result<u16, String> {
    TcpListener::bind(("127.0.0.1", 0))
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .map_err(|error| format!("Failed to reserve local port: {}", error))
}

fn probe_opencode_web_server(port: u16) -> bool {
    let Ok(addr) = format!("127.0.0.1:{}", port).parse() else {
        return false;
    };
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(180)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(220)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(120)));
    if stream
        .write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }

    let mut buffer = [0u8; 4096];
    let Ok(read_len) = stream.read(&mut buffer) else {
        return false;
    };
    if read_len == 0 {
        return false;
    }
    let response = String::from_utf8_lossy(&buffer[..read_len]);
    response.contains(" 200 ") && response.contains("OpenCode")
}

fn discover_opencode_web_server_ports() -> Vec<u16> {
    let opencode_process_ids = opencode_process_ids();
    if opencode_process_ids.is_empty() {
        return Vec::new();
    }
    let mut command = Command::new("netstat.exe");
    command.args(["-ano", "-p", "tcp"]);
    let Ok(output) = spawn_hidden_with_output(command) else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    opencode_web_server_ports_from_netstat_output(
        &String::from_utf8_lossy(&output.stdout),
        &opencode_process_ids,
    )
}

fn opencode_web_server_ports_from_netstat_output(
    output: &str,
    opencode_process_ids: &HashSet<u32>,
) -> Vec<u16> {
    let mut ports = Vec::new();
    for line in output.lines() {
        let columns: Vec<&str> = line.split_whitespace().collect();
        if columns.len() < 5
            || !columns[0].eq_ignore_ascii_case("TCP")
            || !columns[3].eq_ignore_ascii_case("LISTENING")
        {
            continue;
        }
        let Some(pid) = columns[4].parse::<u32>().ok() else {
            continue;
        };
        if !opencode_process_ids.contains(&pid) {
            continue;
        }
        let Some(port) = port_from_netstat_endpoint(columns[1]) else {
            continue;
        };
        if !ports.contains(&port) {
            ports.push(port);
        }
    }
    ports.sort_unstable();
    ports
}

fn port_from_netstat_endpoint(endpoint: &str) -> Option<u16> {
    endpoint
        .rsplit_once(':')
        .and_then(|(_, port)| port.trim_matches(']').parse::<u16>().ok())
}

#[cfg(target_os = "windows")]
fn opencode_process_ids() -> HashSet<u32> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };

    let snapshot_handle = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snapshot_handle == INVALID_HANDLE_VALUE {
        return HashSet::new();
    }

    let mut ids = HashSet::new();
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    if unsafe { Process32FirstW(snapshot_handle, &mut entry) } != 0 {
        loop {
            if wide_null_string(&entry.szExeFile).eq_ignore_ascii_case("opencode.exe") {
                ids.insert(entry.th32ProcessID);
            }

            entry = unsafe { std::mem::zeroed() };
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if unsafe { Process32NextW(snapshot_handle, &mut entry) } == 0 {
                break;
            }
        }
    }

    unsafe {
        CloseHandle(snapshot_handle);
    }
    ids
}

#[cfg(not(target_os = "windows"))]
fn opencode_process_ids() -> HashSet<u32> {
    HashSet::new()
}

fn opencode_desktop_is_running() -> bool {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("tasklist.exe");
        command.args(["/FI", "IMAGENAME eq OpenCode.exe", "/NH"]);
        spawn_hidden_with_output(command)
            .ok()
            .filter(|output| output.status.success())
            .map(|output| {
                String::from_utf8_lossy(&output.stdout)
                    .to_ascii_lowercase()
                    .contains("opencode.exe")
            })
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

#[cfg(test)]
fn opencode_open_project_deep_link(workspace_path: &str) -> String {
    format!(
        "opencode://open-project?directory={}",
        percent_encode_query_component(workspace_path)
    )
}

fn opencode_new_session_deep_link(workspace_path: &str, prompt: &str) -> String {
    let mut link = format!(
        "opencode://new-session?directory={}",
        percent_encode_query_component(workspace_path)
    );
    if let Some(prompt_text) = clean_option(Some(prompt)) {
        link.push_str("&prompt=");
        link.push_str(&percent_encode_query_component(prompt_text));
    }
    link
}

fn spawn_visible_terminal(
    title: &str,
    executable: &str,
    arguments: &[&str],
) -> std::io::Result<Child> {
    let mut command = Command::new("cmd.exe");
    command.args(["/C", "start", title, executable]);
    command.args(arguments);
    command.spawn()
}

fn opencode_cli_run_supports_interactive(opencode_cli: &str) -> bool {
    let mut command = Command::new(opencode_cli);
    command.args(["run", "--help"]);
    spawn_hidden_with_output(command)
        .ok()
        .map(|output| {
            let mut help_text = String::from_utf8_lossy(&output.stdout).into_owned();
            help_text.push_str(&String::from_utf8_lossy(&output.stderr));
            help_text.contains("--interactive")
        })
        .unwrap_or(false)
}

fn launch_opencode_cli_handoff(workspace_path: &str, prompt: &str) -> Result<String, String> {
    let opencode_cli =
        find_opencode_cli_path().ok_or_else(|| "OpenCode CLI not found".to_string())?;
    if opencode_cli_run_supports_interactive(&opencode_cli) {
        spawn_visible_terminal(
            "AgentWatcher OpenCode",
            &opencode_cli,
            &[
                "run",
                "--dir",
                workspace_path,
                "--title",
                "AgentWatcher Handoff",
                "--interactive",
                prompt,
            ],
        )
        .map(|_| "opencode-cli".to_string())
        .map_err(|error| format!("Failed to launch OpenCode run: {}", error))
    } else {
        spawn_visible_terminal(
            "AgentWatcher OpenCode",
            &opencode_cli,
            &[
                "run",
                "--dir",
                workspace_path,
                "--title",
                "AgentWatcher Handoff",
                prompt,
            ],
        )
        .map(|_| "opencode-cli".to_string())
        .map_err(|error| format!("Failed to launch OpenCode run: {}", error))
    }
}

fn launch_opencode_handoff(workspace_path: &str, prompt: &str) -> Result<String, String> {
    if opencode_desktop_is_running() {
        let link = opencode_new_session_deep_link(workspace_path, prompt);
        return open_windows_url(&link)
            .map(|_| "opencode-desktop".to_string())
            .or_else(|desktop_error| {
                launch_opencode_cli_handoff(workspace_path, prompt).map_err(|cli_error| {
                    format!(
                        "OpenCode Desktop handoff failed ({}); CLI fallback failed ({})",
                        desktop_error, cli_error
                    )
                })
            });
    }

    launch_opencode_cli_handoff(workspace_path, prompt)
}

fn find_opencode_cli_path() -> Option<String> {
    let mut candidates = Vec::new();
    if let Some(appdata_path) = env::var_os("APPDATA") {
        let npm_root = PathBuf::from(appdata_path).join("npm");
        candidates.push(path_to_string(
            &npm_root
                .join("node_modules")
                .join("opencode-ai")
                .join("bin")
                .join("opencode.exe"),
        ));
        candidates.push(path_to_string(&npm_root.join("opencode.cmd")));
        candidates.push(path_to_string(&npm_root.join("opencode.ps1")));
        candidates.push(path_to_string(
            &npm_root
                .join("node_modules")
                .join("opencode-ai")
                .join("node_modules")
                .join("opencode-windows-x64")
                .join("bin")
                .join("opencode.exe"),
        ));
        candidates.push(path_to_string(
            &npm_root
                .join("node_modules")
                .join("opencode-ai")
                .join("node_modules")
                .join("opencode-windows-x64-baseline")
                .join("bin")
                .join("opencode.exe"),
        ));
    }
    candidates.extend(
        ["opencode.exe", "opencode.cmd", "opencode.ps1", "opencode"]
            .iter()
            .map(|candidate| candidate.to_string()),
    );

    for candidate in candidates {
        let mut command = Command::new(&candidate);
        command.arg("--version");
        if spawn_hidden_with_output(command)
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            return Some(candidate);
        }
    }
    None
}

fn with_codex_app_server_client<F, T>(operation: F) -> Result<T, String>
where
    F: FnOnce(&mut CodexWsClient) -> Result<T, String>,
{
    let port = ensure_codex_app_server()?;
    let mut client = CodexWsClient::connect(port)?;
    client.initialize()?;
    operation(&mut client)
}

fn ensure_codex_app_server() -> Result<u16, String> {
    let server_mutex = CODEX_APP_SERVER.get_or_init(|| Mutex::new(None));
    let mut server_guard = server_mutex
        .lock()
        .map_err(|_| "Codex app-server lock poisoned".to_string())?;

    if let Some(server) = server_guard.as_mut() {
        if server.child.try_wait().ok().flatten().is_none() && codex_app_server_ready(server.port) {
            return Ok(server.port);
        }
        stop_codex_app_server(server);
        *server_guard = None;
    }

    let codex_cli = find_codex_cli_path().ok_or_else(|| "Codex CLI not found".to_string())?;
    let mut last_error = None;
    for _ in 0..CODEX_APP_SERVER_START_ATTEMPTS {
        let port = reserve_loopback_port()?;
        let listen_url = format!("ws://127.0.0.1:{}", port);
        let mut command = Command::new(&codex_cli);
        command
            .args(["app-server", "--listen", &listen_url])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match spawn_hidden(command) {
            Ok(mut child) => {
                if wait_for_codex_app_server_ready(port, CODEX_APP_SERVER_READY_TIMEOUT_MS) {
                    *server_guard = Some(CodexAppServerProcess { port, child });
                    return Ok(port);
                }
                let _ = child.kill();
                let _ = child.wait();
                last_error = Some(format!(
                    "Codex app-server did not become ready on {}",
                    listen_url
                ));
            }
            Err(error) => {
                last_error = Some(format!("Failed to start Codex app-server: {}", error));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| "Unable to start Codex app-server".to_string()))
}

fn stop_codex_app_server(server: &mut CodexAppServerProcess) {
    let _ = server.child.kill();
    let _ = server.child.wait();
}

fn shutdown_codex_app_server() {
    let Some(server_mutex) = CODEX_APP_SERVER.get() else {
        return;
    };
    let Ok(mut server_guard) = server_mutex.lock() else {
        return;
    };
    if let Some(mut server) = server_guard.take() {
        stop_codex_app_server(&mut server);
    }
}

fn reserve_loopback_port() -> Result<u16, String> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("Failed to reserve loopback port: {}", error))?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|error| format!("Failed to read reserved loopback port: {}", error))
}

fn wait_for_codex_app_server_ready(port: u16, timeout_ms: u64) -> bool {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        if codex_app_server_ready(port) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(150));
    }
    false
}

fn codex_app_server_ready(port: u16) -> bool {
    let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(750)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(750)));
    let request = format!(
        "GET /readyz HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
        port
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut response = [0u8; 96];
    stream
        .read(&mut response)
        .map(|count| String::from_utf8_lossy(&response[..count]).contains("200 OK"))
        .unwrap_or(false)
}

struct CodexWsClient {
    stream: TcpStream,
    next_id: u64,
}

impl CodexWsClient {
    fn connect(port: u16) -> Result<Self, String> {
        let mut stream = TcpStream::connect(("127.0.0.1", port))
            .map_err(|error| format!("Failed to connect to Codex app-server: {}", error))?;
        let timeout = Duration::from_millis(CODEX_APP_SERVER_RPC_TIMEOUT_MS);
        let _ = stream.set_read_timeout(Some(timeout));
        let _ = stream.set_write_timeout(Some(timeout));
        websocket_handshake(&mut stream, port)?;
        Ok(Self { stream, next_id: 1 })
    }

    fn initialize(&mut self) -> Result<(), String> {
        self.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "agentwatcher",
                    "title": "AgentWatcher",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }),
        )?;
        self.notify("initialized", json!({}))
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let request_id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        websocket_send_text(
            &mut self.stream,
            &json!({
                "method": method,
                "id": request_id,
                "params": params
            })
            .to_string(),
        )?;

        let deadline = Instant::now() + Duration::from_millis(CODEX_APP_SERVER_RPC_TIMEOUT_MS);
        while Instant::now() < deadline {
            let message = websocket_read_text(&mut self.stream)?;
            let Some(response_json) = serde_json::from_str::<Value>(&message).ok() else {
                continue;
            };
            if response_json.get("id").and_then(Value::as_u64) != Some(request_id) {
                continue;
            }
            if let Some(error_value) = response_json.get("error") {
                let message = string_at(error_value, &["/message"])
                    .unwrap_or("Codex app-server request failed");
                return Err(message.to_string());
            }
            return Ok(response_json.get("result").cloned().unwrap_or(Value::Null));
        }

        Err(format!("Codex app-server request timed out: {}", method))
    }

    fn notify(&mut self, method: &str, params: Value) -> Result<(), String> {
        websocket_send_text(
            &mut self.stream,
            &json!({
                "method": method,
                "params": params
            })
            .to_string(),
        )
    }
}

impl Drop for CodexWsClient {
    fn drop(&mut self) {
        let _ = websocket_send_close(&mut self.stream);
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

fn websocket_handshake(stream: &mut TcpStream, port: u16) -> Result<(), String> {
    let seed = current_time_ms() ^ (u64::from(std::process::id()) << 16) ^ u64::from(port);
    let mut key_bytes = [0u8; 16];
    key_bytes[..8].copy_from_slice(&seed.to_be_bytes());
    key_bytes[8..12].copy_from_slice(&std::process::id().to_be_bytes());
    key_bytes[12..14].copy_from_slice(&port.to_be_bytes());
    key_bytes[14..].copy_from_slice(&(seed as u16).rotate_left(5).to_be_bytes());
    let websocket_key = base64_standard(&key_bytes);
    let request = format!(
        "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {websocket_key}\r\nSec-WebSocket-Version: 13\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("Failed to write Codex WebSocket handshake: {}", error))?;

    let mut response = Vec::new();
    let mut buffer = [0u8; 1];
    while response.len() < 8192 {
        stream
            .read_exact(&mut buffer)
            .map_err(|error| format!("Failed to read Codex WebSocket handshake: {}", error))?;
        response.push(buffer[0]);
        if response.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let response_text = String::from_utf8_lossy(&response);
    if response_text.starts_with("HTTP/1.1 101") || response_text.starts_with("HTTP/1.0 101") {
        Ok(())
    } else {
        Err(format!(
            "Codex WebSocket handshake failed: {}",
            response_text.lines().next().unwrap_or("empty response")
        ))
    }
}

fn websocket_send_text(stream: &mut TcpStream, text: &str) -> Result<(), String> {
    websocket_send_frame(stream, 0x1, text.as_bytes())
}

fn websocket_send_close(stream: &mut TcpStream) -> Result<(), String> {
    websocket_send_frame(stream, 0x8, &[])
}

fn websocket_send_pong(stream: &mut TcpStream, payload: &[u8]) -> Result<(), String> {
    websocket_send_frame(stream, 0xA, payload)
}

fn websocket_send_frame(stream: &mut TcpStream, opcode: u8, payload: &[u8]) -> Result<(), String> {
    let mut frame = Vec::with_capacity(payload.len() + 14);
    frame.push(0x80 | (opcode & 0x0F));
    let mask_bit = 0x80;
    if payload.len() <= 125 {
        frame.push(mask_bit | payload.len() as u8);
    } else if payload.len() <= u16::MAX as usize {
        frame.push(mask_bit | 126);
        frame.extend_from_slice(&(payload.len() as u16).to_be_bytes());
    } else {
        frame.push(mask_bit | 127);
        frame.extend_from_slice(&(payload.len() as u64).to_be_bytes());
    }
    let mask = codex_ws_mask();
    frame.extend_from_slice(&mask);
    for (index, byte_value) in payload.iter().enumerate() {
        frame.push(byte_value ^ mask[index % 4]);
    }
    stream
        .write_all(&frame)
        .map_err(|error| format!("Failed to write Codex WebSocket frame: {}", error))
}

fn websocket_read_text(stream: &mut TcpStream) -> Result<String, String> {
    loop {
        let mut header = [0u8; 2];
        stream
            .read_exact(&mut header)
            .map_err(|error| format!("Failed to read Codex WebSocket frame: {}", error))?;
        let opcode = header[0] & 0x0F;
        let masked = (header[1] & 0x80) != 0;
        let mut payload_len = u64::from(header[1] & 0x7F);
        if payload_len == 126 {
            let mut extended = [0u8; 2];
            stream.read_exact(&mut extended).map_err(|error| {
                format!("Failed to read Codex WebSocket frame length: {}", error)
            })?;
            payload_len = u64::from(u16::from_be_bytes(extended));
        } else if payload_len == 127 {
            let mut extended = [0u8; 8];
            stream.read_exact(&mut extended).map_err(|error| {
                format!("Failed to read Codex WebSocket frame length: {}", error)
            })?;
            payload_len = u64::from_be_bytes(extended);
        }
        if payload_len > 16 * 1024 * 1024 {
            return Err("Codex WebSocket frame is too large".to_string());
        }
        let mut mask = [0u8; 4];
        if masked {
            stream
                .read_exact(&mut mask)
                .map_err(|error| format!("Failed to read Codex WebSocket mask: {}", error))?;
        }
        let mut payload = vec![0u8; payload_len as usize];
        stream
            .read_exact(&mut payload)
            .map_err(|error| format!("Failed to read Codex WebSocket payload: {}", error))?;
        if masked {
            for (index, byte_value) in payload.iter_mut().enumerate() {
                *byte_value ^= mask[index % 4];
            }
        }
        match opcode {
            0x1 => {
                return String::from_utf8(payload)
                    .map_err(|error| format!("Codex WebSocket payload was not UTF-8: {}", error));
            }
            0x8 => return Err("Codex app-server closed the WebSocket".to_string()),
            0x9 => {
                websocket_send_pong(stream, &payload)?;
            }
            0xA => {}
            _ => {}
        }
    }
}

fn codex_ws_mask() -> [u8; 4] {
    let seed = current_time_ms() ^ u64::from(std::process::id());
    [
        (seed & 0xFF) as u8,
        ((seed >> 8) & 0xFF) as u8,
        ((seed >> 16) & 0xFF) as u8,
        ((seed >> 24) & 0xFF) as u8,
    ]
}

fn base64_standard(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
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
            output.push('=');
            output.push('=');
        }
        2 => {
            let first = bytes[index];
            let second = bytes[index + 1];
            output.push(ALPHABET[(first >> 2) as usize] as char);
            output.push(ALPHABET[(((first << 4) | (second >> 4)) & 63) as usize] as char);
            output.push(ALPHABET[((second << 2) & 63) as usize] as char);
            output.push('=');
        }
        _ => {}
    }
    output
}

fn find_codex_cli_path() -> Option<String> {
    let mut candidates = Vec::new();
    if let Some(appdata_path) = env::var_os("APPDATA") {
        candidates.push(path_to_string(
            &PathBuf::from(appdata_path).join("npm").join("codex.cmd"),
        ));
    }
    candidates.extend(
        ["codex.cmd", "codex.exe", "codex"]
            .iter()
            .map(|candidate| candidate.to_string()),
    );

    for candidate in candidates {
        let mut command = Command::new(&candidate);
        command.arg("--version");
        if spawn_hidden_with_output(command)
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            return Some(candidate);
        }
    }
    None
}

fn open_codex_workspace(workspace_path: &str) -> Result<(), String> {
    let codex_cli = find_codex_cli_path().ok_or_else(|| "Codex CLI not found".to_string())?;
    let mut command = Command::new(codex_cli);
    command.args(["app", workspace_path]);
    spawn_hidden(command)
        .map(|_| ())
        .map_err(|error| format!("Failed to open Codex workspace: {}", error))
}

fn open_windows_url(url: &str) -> Result<(), String> {
    let mut protocol_command = Command::new("rundll32.exe");
    protocol_command.args(["url.dll,FileProtocolHandler", url]);
    spawn_hidden(protocol_command)
        .map(|_| ())
        .map_err(|error| format!("Windows protocol launch failed: {}", error))
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
            if !is_active_session(
                fallback_updated_ms_from_modified(modified_ms, scan_time_ms),
                scan_time_ms,
                options,
            ) {
                continue;
            }
            candidates.push((session_path, workspace_info.clone(), modified_ms));
        }
    }

    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.2));
    candidates
        .into_iter()
        .take(candidate_scan_limit(options))
        .filter_map(|(session_path, workspace_info, _)| {
            read_copilot_session(&session_path, &workspace_info, scan_time_ms, options)
        })
        .collect()
}

fn copilot_cli_session_store_paths() -> Vec<PathBuf> {
    let Some(appdata_path) = env::var_os("APPDATA") else {
        return Vec::new();
    };
    let appdata_path = PathBuf::from(appdata_path);
    ["Code", "Code - Insiders"]
        .iter()
        .map(|product_folder| {
            appdata_path
                .join(product_folder)
                .join("User")
                .join("globalStorage")
                .join("github.copilot-chat")
                .join("session-store.db")
        })
        .filter(|path| path.exists())
        .collect()
}

fn scan_copilot_cli_sessions(
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
) -> Vec<AgentSession> {
    let mut sessions = Vec::new();
    for db_path in copilot_cli_session_store_paths() {
        let Ok(connection) = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ) else {
            continue;
        };
        let _ = connection.busy_timeout(Duration::from_millis(OPENCODE_SQLITE_BUSY_TIMEOUT_MS));
        let _ = connection.pragma_update(None, "query_only", "ON");
        sessions.extend(scan_copilot_cli_sessions_from_connection(
            &connection,
            &db_path,
            scan_time_ms,
            options,
        ));
    }
    sessions
}

fn scan_copilot_cli_sessions_from_connection(
    connection: &Connection,
    db_path: &Path,
    scan_time_ms: u64,
    options: &ResolvedScanOptions,
) -> Vec<AgentSession> {
    let mut statement = match connection.prepare(
        "select s.id, s.cwd, s.branch, s.summary, s.created_at, s.updated_at, \
                count(t.id), max(t.timestamp), \
                (select user_message from turns where session_id = s.id and trim(coalesce(user_message, '')) <> '' order by turn_index desc limit 1), \
                (select assistant_response from turns where session_id = s.id and trim(coalesce(assistant_response, '')) <> '' order by turn_index desc limit 1), \
                (select user_message from turns where session_id = s.id and trim(coalesce(user_message, '')) <> '' order by turn_index asc limit 1) \
         from sessions s \
         left join turns t on t.session_id = s.id \
         where lower(trim(coalesce(s.agent_name, ''))) = 'copilotcli' \
         group by s.id \
         order by s.updated_at desc \
         limit ?1",
    ) {
        Ok(statement) => statement,
        Err(_) => return Vec::new(),
    };
    let rows = match statement.query_map([candidate_scan_limit(options) as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, Option<String>>(7)?,
            row.get::<_, Option<String>>(8)?,
            row.get::<_, Option<String>>(9)?,
            row.get::<_, Option<String>>(10)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    rows.filter_map(Result::ok)
        .filter_map(
            |(
                session_id,
                workspace_path,
                branch,
                summary,
                created_at,
                updated_at,
                turn_count,
                last_turn_at,
                last_user_message,
                last_ai_message,
                first_user_message,
            )| {
                let updated_ms = [
                    updated_at.as_deref(),
                    last_turn_at.as_deref(),
                    created_at.as_deref(),
                ]
                .into_iter()
                .flatten()
                .filter_map(iso_timestamp_ms)
                .max()?;
                if !is_active_session(updated_ms, scan_time_ms, options) {
                    return None;
                }
                let workspace_path = workspace_path.filter(|path| !path.trim().is_empty());
                let workspace = workspace_path
                    .as_deref()
                    .map(Path::new)
                    .and_then(display_name_from_path)
                    .unwrap_or_else(|| "Copilot CLI".to_string());
                let title = summary
                    .as_deref()
                    .and_then(title_candidate_from_text)
                    .or_else(|| {
                        first_user_message
                            .as_deref()
                            .and_then(title_candidate_from_text)
                    })
                    .unwrap_or_else(|| {
                        fallback_session_title("Copilot CLI", &workspace, &session_id)
                    });
                let (last_user_message, last_user_message_truncated) =
                    apply_user_preview_budget(last_user_message.or(summary));
                let (last_ai_message, last_ai_message_truncated, last_ai_message_excerpt_kind) =
                    apply_ai_preview_budget(last_ai_message);
                let session_path = path_to_string(db_path);
                let session_resource = copilot_session_resource(&session_id);
                let workspace_identity = build_workspace_identity(
                    "copilot",
                    &session_id,
                    &workspace,
                    workspace_path.as_deref(),
                    Some(&session_path),
                    Some(&session_resource),
                );
                let plugin_workflow = detect_plugin_workflow_session_marker(
                    "copilot",
                    &[last_user_message.as_deref(), last_ai_message.as_deref()],
                );

                Some(AgentSession {
                    id: format!("copilot-cli:{}", session_id),
                    provider: "copilot-cli".to_string(),
                    provider_label: "GH›".to_string(),
                    title,
                    workspace,
                    workspace_path,
                    workspace_key: workspace_identity.key,
                    workspace_name: workspace_identity.name,
                    workspace_label: workspace_identity.label,
                    workspace_group: workspace_identity.group,
                    workspace_discriminator: workspace_identity.discriminator,
                    session_path: Some(session_path),
                    session_resource: Some(session_resource),
                    status: status_from_activity(
                        updated_ms,
                        scan_time_ms,
                        ActivitySource::ContentTimestamp,
                        None,
                    ),
                    time_label: time_label(updated_ms, scan_time_ms),
                    updated_ms,
                    message_count: u32::try_from(turn_count.max(0)).unwrap_or(u32::MAX),
                    last_user_message,
                    last_user_message_truncated,
                    last_ai_message,
                    last_ai_message_truncated,
                    last_ai_message_excerpt_kind,
                    branch,
                    todo_ids: Vec::new(),
                    plugin_workflow,
                })
            },
        )
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
        mut todo_ids,
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
        for todo_id in overlay.todo_ids {
            push_unique_todo_id(&mut todo_ids, todo_id);
        }
        if overlay.last_ai_message.is_some() {
            let (message, truncated, excerpt_kind) =
                apply_ai_preview_budget(overlay.last_ai_message);
            last_ai_message = message;
            last_ai_message_truncated = truncated;
            last_ai_message_excerpt_kind = excerpt_kind;
        }
    }

    let (updated_ms, activity_source) =
        latest_activity_ms(latest_timestamp_ms, fallback_updated_ms);
    if !is_active_session(updated_ms, scan_time_ms, options) {
        return None;
    }

    let status = status_from_activity(updated_ms, scan_time_ms, activity_source, status_hint);
    let title = title.unwrap_or_else(|| {
        fallback_session_title("Copilot", &workspace_info.display_name, &session_id)
    });
    let session_resource = copilot_session_resource(&session_id);
    let session_path_text = path_to_string(session_path);
    let workspace_identity = build_workspace_identity(
        "copilot",
        &session_id,
        &workspace_info.display_name,
        workspace_info.path_text.as_deref(),
        Some(&session_path_text),
        Some(&session_resource),
    );

    let plugin_workflow = detect_plugin_workflow_session_marker(
        "copilot",
        &[last_user_message.as_deref(), last_ai_message.as_deref()],
    );

    Some(AgentSession {
        id: format!("copilot:{}", session_id),
        provider: "copilot".to_string(),
        provider_label: "GH".to_string(),
        title,
        workspace: workspace_info.display_name.clone(),
        workspace_path: workspace_info.path_text.clone(),
        workspace_key: workspace_identity.key,
        workspace_name: workspace_identity.name,
        workspace_label: workspace_identity.label,
        workspace_group: workspace_identity.group,
        workspace_discriminator: workspace_identity.discriminator,
        session_path: Some(session_path_text),
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
        todo_ids,
        plugin_workflow,
    })
}

fn scan_claude_sessions(scan_time_ms: u64, options: &ResolvedScanOptions) -> Vec<AgentSession> {
    let Some(user_profile_path) = env::var_os("USERPROFILE") else {
        return Vec::new();
    };

    let projects_root = PathBuf::from(user_profile_path)
        .join(".claude")
        .join("projects");

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
            if !is_active_session(
                fallback_updated_ms_from_modified(modified_ms, scan_time_ms),
                scan_time_ms,
                options,
            ) {
                continue;
            }
            candidates.push((session_path, modified_ms));
        }
    }

    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.1));
    let mut sessions: Vec<AgentSession> = candidates
        .into_iter()
        .take(candidate_scan_limit(options))
        .filter_map(|(session_path, _)| {
            read_claude_jsonl_session(&session_path, scan_time_ms, options)
        })
        .collect();

    let mut known_session_paths: HashSet<String> = sessions
        .iter()
        .filter_map(|session| session.session_path.clone())
        .collect();
    for project_path in project_paths {
        for indexed_session in read_claude_index(
            &project_path.join("sessions-index.json"),
            scan_time_ms,
            options,
        ) {
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
        todo_ids,
    } = summary;
    if options.hide_archived && archived {
        return None;
    }

    let (updated_ms, activity_source) =
        latest_activity_ms(latest_timestamp_ms, fallback_updated_ms);
    if !is_active_session(updated_ms, scan_time_ms, options) {
        return None;
    }

    let status = status_from_activity(updated_ms, scan_time_ms, activity_source, status_hint);
    let workspace_path = workspace_path.filter(|path_text| !path_text.trim().is_empty());
    let workspace = workspace_path
        .as_deref()
        .map(Path::new)
        .and_then(display_name_from_path)
        .or_else(|| session_path.parent().and_then(display_name_from_path))
        .unwrap_or_else(|| "Claude".to_string());
    let title = title.unwrap_or_else(|| fallback_session_title("Claude", &workspace, &session_id));
    let session_path_text = path_to_string(session_path);
    let session_resource = provider_session_resource("claude-code", &session_id);
    let workspace_identity = build_workspace_identity(
        "claude",
        &session_id,
        &workspace,
        workspace_path.as_deref(),
        Some(&session_path_text),
        Some(&session_resource),
    );

    let plugin_workflow = detect_plugin_workflow_session_marker(
        "claude",
        &[last_user_message.as_deref(), last_ai_message.as_deref()],
    );

    Some(AgentSession {
        id: format!("claude:{}", session_id),
        provider: "claude".to_string(),
        provider_label: "C".to_string(),
        title,
        workspace,
        workspace_path,
        workspace_key: workspace_identity.key,
        workspace_name: workspace_identity.name,
        workspace_label: workspace_identity.label,
        workspace_group: workspace_identity.group,
        workspace_discriminator: workspace_identity.discriminator,
        session_path: Some(session_path_text),
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
        branch,
        todo_ids,
        plugin_workflow,
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
    store_jsonl_summary(
        &COPILOT_SESSION_CACHE,
        path_key,
        fingerprint,
        summary.clone(),
    );
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
    let mut todo_ids = Vec::new();
    let mut head_message_count = 0u32;
    let mut tail_message_count = 0u32;

    visit_jsonl_head_values(
        session_path,
        fingerprint.len,
        COPILOT_TITLE_SAMPLE_LINES,
        |_, json_value| {
            collect_todo_ids_from_copilot_requests(json_value, &mut todo_ids);
            if is_archived_json(json_value) {
                archived = true;
            }
            if let Some(next_session_id) = string_at(json_value, &["/v/sessionId", "/sessionId"]) {
                session_id = clean_label(next_session_id, 96);
            }
            if is_copilot_message_event(json_value) {
                head_message_count = head_message_count.saturating_add(1);
            }
            if let Some(next_title) =
                copilot_alias_title_text(json_value).and_then(title_candidate_from_text)
            {
                alias_title = Some(next_title);
            }
            if prompt_title.is_none() {
                prompt_title = copilot_title_text(json_value).and_then(title_candidate_from_text);
            }
        },
    )?;

    let tail_covers_file = tail_sample_covers_file(fingerprint.len);
    let _ = visit_jsonl_tail_values(session_path, fingerprint.len, |json_value| {
        collect_todo_ids_from_copilot_requests(json_value, &mut todo_ids);
        if is_archived_json(json_value) {
            archived = true;
        }
        latest_timestamp_ms =
            newest_timestamp_ms(latest_timestamp_ms, timestamp_ms_from_json(json_value));
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
        if let Some(next_title) =
            copilot_alias_title_text(json_value).and_then(title_candidate_from_text)
        {
            alias_title = Some(next_title);
        }
    });

    if last_user_message.is_none() {
        last_user_message = read_latest_copilot_user_message(session_path, fingerprint.len);
    }
    if last_ai_message.is_none() {
        last_ai_message = read_latest_copilot_ai_message(session_path, fingerprint.len);
    }

    let (last_user_message, last_user_message_truncated) =
        apply_user_preview_budget(last_user_message);
    let (last_ai_message, last_ai_message_truncated, last_ai_message_excerpt_kind) =
        apply_ai_preview_budget(last_ai_message);
    Some(CopilotSessionSummary {
        session_id,
        title: alias_title.or(prompt_title),
        status_hint,
        latest_timestamp_ms,
        message_count: sampled_message_count(
            head_message_count,
            tail_message_count,
            tail_covers_file,
        ),
        last_user_message,
        last_user_message_truncated,
        last_ai_message,
        last_ai_message_truncated,
        last_ai_message_excerpt_kind,
        archived,
        todo_ids,
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
    store_jsonl_summary(
        &CLAUDE_SESSION_CACHE,
        path_key,
        fingerprint,
        summary.clone(),
    );
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
    let mut todo_ids = Vec::new();
    let mut pending_ask_user_question_ids = HashSet::new();
    let mut archived = false;
    let mut latest_timestamp_ms = None;
    let mut head_message_count = 0u32;
    let mut tail_message_count = 0u32;

    visit_jsonl_head_values(
        session_path,
        fingerprint.len,
        CLAUDE_TITLE_SAMPLE_LINES,
        |_, json_value| {
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
            if let Some(next_title) =
                claude_alias_title_text(json_value).and_then(title_candidate_from_text)
            {
                alias_title = Some(next_title);
            } else if slug_title.is_none() {
                slug_title = string_at(json_value, &["/slug"]).and_then(title_candidate_from_text);
            }
            if event_type == Some("user") {
                if let Some(clean_text) = claude_clean_user_text(json_value) {
                    collect_todo_ids_from_text(&clean_text, &mut todo_ids);
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
        },
    )?;

    let tail_covers_file = tail_sample_covers_file(fingerprint.len);
    let _ = visit_jsonl_tail_values(session_path, fingerprint.len, |json_value| {
        if is_archived_json(json_value) {
            archived = true;
        }
        latest_timestamp_ms =
            newest_timestamp_ms(latest_timestamp_ms, timestamp_ms_from_json(json_value));
        if is_claude_message_event(json_value) {
            tail_message_count = tail_message_count.saturating_add(1);
        }
        match string_at(json_value, &["/type"]) {
            Some("user") => {
                let next = claude_interactive_user_text(json_value)
                    .or_else(|| claude_clean_user_text(json_value));
                if let Some(next_user_message) = next {
                    collect_todo_ids_from_text(&next_user_message, &mut todo_ids);
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
        update_claude_status_hint(
            &mut status_hint,
            &mut pending_ask_user_question_ids,
            json_value,
        );
        if let Some(next_title) =
            claude_alias_title_text(json_value).and_then(title_candidate_from_text)
        {
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

    let (last_user_message, last_user_message_truncated) =
        apply_user_preview_budget(last_user_message.or(first_user_message));
    let (last_ai_message, last_ai_message_truncated, last_ai_message_excerpt_kind) =
        apply_ai_preview_budget(last_ai_message);
    Some(ClaudeSessionSummary {
        title: alias_title.or(slug_title).or(prompt_title),
        workspace_path,
        branch,
        status_hint,
        latest_timestamp_ms,
        message_count: sampled_message_count(
            head_message_count,
            tail_message_count,
            tail_covers_file,
        ),
        last_user_message,
        last_user_message_truncated,
        last_ai_message,
        last_ai_message_truncated,
        last_ai_message_excerpt_kind,
        archived,
        todo_ids,
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

fn push_unique_todo_id(todo_ids: &mut Vec<String>, todo_id: String) {
    if !todo_ids.iter().any(|existing| existing == &todo_id) {
        todo_ids.push(todo_id);
    }
}

fn collect_todo_ids_from_text(text: &str, todo_ids: &mut Vec<String>) {
    let lower_text = text.to_ascii_lowercase();
    let bytes = lower_text.as_bytes();
    let mut index = 0usize;

    while index + 5 <= bytes.len() {
        let Some(offset) = lower_text[index..].find("todo-") else {
            break;
        };
        let start = index + offset;
        if start > 0 && bytes[start - 1].is_ascii_alphanumeric() {
            index = start + 5;
            continue;
        }

        let mut cursor = start + 5;
        let first_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_alphanumeric() {
            cursor += 1;
        }
        if cursor == first_start || cursor >= bytes.len() || bytes[cursor] != b'-' {
            index = start + 5;
            continue;
        }

        cursor += 1;
        let second_start = cursor;
        while cursor < bytes.len() && bytes[cursor].is_ascii_alphanumeric() {
            cursor += 1;
        }
        if cursor == second_start {
            index = start + 5;
            continue;
        }
        if cursor < bytes.len() && (bytes[cursor].is_ascii_alphanumeric() || bytes[cursor] == b'-')
        {
            index = cursor;
            continue;
        }

        push_unique_todo_id(todo_ids, lower_text[start..cursor].to_string());
        index = cursor;
    }
}

fn collect_todo_ids_from_copilot_requests(json_value: &Value, todo_ids: &mut Vec<String>) {
    if let Some(input_text) = input_text_from_copilot_event(json_value) {
        collect_todo_ids_from_text(input_text, todo_ids);
    }

    if let Some(requests) = copilot_event_requests(json_value) {
        for request in requests {
            if is_copilot_system_request(request) {
                continue;
            }
            if let Some(text) = copilot_request_message_text(request) {
                collect_todo_ids_from_text(text, todo_ids);
            }
        }
    }
}

fn read_copilot_transcript_overlay(session_path: &Path) -> Option<CopilotTranscriptOverlay> {
    let transcript_path = copilot_transcript_path(session_path)?;
    let fingerprint = jsonl_fingerprint(&transcript_path)?;
    let path_key = path_to_string(&transcript_path);
    if let Some(overlay) = cached_jsonl_summary(&COPILOT_TRANSCRIPT_CACHE, &path_key, &fingerprint)
    {
        return Some(overlay);
    }
    let text = read_file_suffix_text(
        &transcript_path,
        fingerprint.len,
        COPILOT_TRANSCRIPT_SCAN_BYTES,
    )?;
    let overlay = copilot_transcript_overlay_from_text(&text)?;
    store_jsonl_summary(
        &COPILOT_TRANSCRIPT_CACHE,
        path_key,
        &fingerprint,
        overlay.clone(),
    );
    Some(overlay)
}

fn copilot_unstarted_ask_is_recent(overlay: &CopilotTranscriptOverlay, scan_time_ms: u64) -> bool {
    overlay
        .unstarted_ask_timestamp_ms
        .map(|timestamp_ms| {
            scan_time_ms.saturating_sub(timestamp_ms) <= COPILOT_UNSTARTED_ASK_WAITING_WINDOW_MS
        })
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
        let Some(json_value) = json_value_from_jsonl_line(line_text) else {
            continue;
        };
        overlay.latest_timestamp_ms = newest_timestamp_ms(
            overlay.latest_timestamp_ms,
            timestamp_ms_from_json(&json_value),
        );

        match string_at(&json_value, &["/type"]) {
            Some("user.message") => {
                started_ask_tool_ids.clear();
                unstarted_ask_tool_ids.clear();
                unstarted_ask_timestamp_ms = None;
                if let Some(raw_content) = string_at(&json_value, &["/data/content"]) {
                    collect_todo_ids_from_text(raw_content, &mut overlay.todo_ids);
                }
                if let Some(content) =
                    string_at(&json_value, &["/data/content"]).and_then(clean_preview_text)
                {
                    overlay.last_user_message = Some(content);
                    saw_activity = true;
                }
            }
            Some("assistant.message") => {
                started_ask_tool_ids.clear();
                unstarted_ask_tool_ids.clear();
                unstarted_ask_timestamp_ms = None;
                let mut parts = Vec::new();
                if let Some(content) =
                    string_at(&json_value, &["/data/content"]).and_then(clean_preview_text)
                {
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
                    if let Some(message) = question_tool_preview_text(
                        json_value.pointer("/data/arguments").unwrap_or(&json_value),
                    ) {
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
            Some("assistant.message_delta")
            | Some("assistant.turn_end")
            | Some("assistant.turn_start") => {
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
        || !overlay.todo_ids.is_empty()
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
    let Some(tool_requests) = tool_requests.and_then(Value::as_array) else {
        return false;
    };
    let mut found_ask_request = false;
    for request in tool_requests {
        if string_at(request, &["/name"]) != Some("vscode_askQuestions") {
            continue;
        }
        found_ask_request = true;
        if let Some(tool_call_id) = string_at(request, &["/toolCallId"]) {
            unstarted_ask_tool_ids.insert(tool_call_id.to_string());
        }
        if let Some(message) =
            question_tool_preview_text(request.pointer("/arguments").unwrap_or(request))
        {
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
                Some("last-prompt") if last_prompt_fallback.is_none() => {
                    if let Some(lp_text) = string_at(&json_value, &["/lastPrompt"]) {
                        last_prompt_fallback = clean_preview_text(lp_text);
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
            cached_summary.len == fingerprint.len
                && cached_summary.modified_ms == fingerprint.modified_ms
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
    let modified_ms = metadata
        .modified()
        .ok()
        .and_then(system_time_ms)
        .unwrap_or(0);

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
    options
        .max_sessions
        .saturating_mul(4)
        .max(options.max_sessions + 20)
        .max(120)
}

fn latest_activity_ms(
    latest_timestamp_ms: Option<u64>,
    fallback_updated_ms: u64,
) -> (u64, ActivitySource) {
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
    Read::by_ref(&mut file)
        .take(read_len)
        .read_to_end(&mut bytes)
        .ok()?;

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
    Read::by_ref(&mut file)
        .take(read_len)
        .read_to_end(&mut bytes)
        .ok()?;

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

fn sampled_message_count(
    head_message_count: u32,
    tail_message_count: u32,
    tail_covers_file: bool,
) -> u32 {
    if tail_covers_file {
        tail_message_count
    } else {
        head_message_count.saturating_add(tail_message_count)
    }
}

fn update_copilot_status_hint(status_hint: &mut Option<SessionStatusHint>, json_value: &Value) {
    if let Some(response_hint) = copilot_response_status_hint(json_value) {
        *status_hint = Some(response_hint);
    } else if copilot_request_finished_status_hint(json_value)
        || input_text_from_copilot_event(json_value).is_some()
    {
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

fn copilot_visible_response_text<'a>(
    response_items: impl Iterator<Item = &'a Value>,
) -> Option<String> {
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
    matches!(
        string_at(response_item, &["/toolId"]),
        Some("vscode_askQuestions")
    )
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
            append_question_options(
                object.get("options").or_else(|| object.get("choices")),
                parts,
            );

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

fn append_question_field(
    object: &serde_json::Map<String, Value>,
    key: &str,
    parts: &mut Vec<String>,
) {
    if let Some(text) = object.get(key).and_then(Value::as_str) {
        let trimmed = text.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }
}

fn append_question_options(options: Option<&Value>, parts: &mut Vec<String>) {
    let Some(options) = options else {
        return;
    };
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
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
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
                Some("tool_use")
                    if string_at(content_block, &["/name"]) == Some("AskUserQuestion") =>
                {
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
    matches!(
        string_at(json_value, &["/type"]),
        Some("user") | Some("assistant")
    )
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
    CLAUDE_CONTEXT_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
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
        if bool_at(
            json_value,
            &["/toolUseResult/skipped", "/toolUseResult/isSkipped"],
        ) == Some(true)
        {
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
    let Some(tool_name) = string_at(content_block, &["/name"]) else {
        return false;
    };
    let normalized: String = tool_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        normalized.as_str(),
        "askuserquestion" | "askquestion" | "question"
    )
}

fn text_from_content_blocks(content_value: Option<&Value>) -> Option<&str> {
    content_value
        .and_then(Value::as_array)
        .and_then(|content_blocks| {
            content_blocks.iter().find_map(|content_block| {
                string_at(content_block, &["/text"])
                    .filter(|text_value| !text_value.trim().is_empty())
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

    let raw_session_id =
        string_at(entry_json, &["/sessionId", "/id"]).map(|session_id| clean_label(session_id, 96));
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
        .or_else(|| {
            session_path
                .as_deref()
                .and_then(|path_text| modified_time_ms(Path::new(path_text)))
        })
        .unwrap_or_else(|| modified_time_ms(index_path).unwrap_or(scan_time_ms));
    if !is_active_session(updated_ms, scan_time_ms, options) {
        return None;
    }

    let status = status_from_activity(updated_ms, scan_time_ms, ActivitySource::FileModified, None);
    let workspace_path = workspace_path.filter(|path_text| !path_text.trim().is_empty());
    let session_path = session_path.filter(|path_text| !path_text.trim().is_empty());
    let workspace = workspace_path
        .as_deref()
        .map(Path::new)
        .and_then(display_name_from_path)
        .or_else(|| index_path.parent().and_then(display_name_from_path))
        .unwrap_or_else(|| "Claude".to_string());
    let first_prompt = string_at(entry_json, &["/firstPrompt"]).and_then(clean_preview_text);
    let title = string_at(
        entry_json,
        &[
            "/aiTitle",
            "/customTitle",
            "/sessionTitle",
            "/title",
            "/slug",
        ],
    )
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
    let mut todo_ids = Vec::new();
    if let Some(first_prompt_text) = first_prompt.as_deref() {
        collect_todo_ids_from_text(first_prompt_text, &mut todo_ids);
    }
    let first_prompt_preview = apply_user_preview_budget(first_prompt);
    let session_resource = provider_session_resource("claude-code", &session_id);
    let workspace_identity = build_workspace_identity(
        "claude",
        &session_id,
        &workspace,
        workspace_path.as_deref(),
        session_path.as_deref(),
        Some(&session_resource),
    );
    let plugin_workflow =
        detect_plugin_workflow_session_marker("claude", &[first_prompt_preview.0.as_deref()]);

    Some(AgentSession {
        id: format!("claude:{}", session_id),
        provider: "claude".to_string(),
        provider_label: "C".to_string(),
        title,
        workspace,
        workspace_path,
        workspace_key: workspace_identity.key,
        workspace_name: workspace_identity.name,
        workspace_label: workspace_identity.label,
        workspace_group: workspace_identity.group,
        workspace_discriminator: workspace_identity.discriminator,
        session_path,
        session_resource: Some(session_resource),
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
        todo_ids,
        plugin_workflow,
    })
}
#[derive(Debug, Clone)]
struct WorkspaceIdentity {
    key: String,
    name: String,
    label: String,
    group: String,
    discriminator: String,
}

#[derive(Debug, Clone)]
struct WorkspaceInfo {
    display_name: String,
    path_text: Option<String>,
}

fn build_workspace_identity(
    provider: &str,
    session_id: &str,
    workspace_name_hint: &str,
    workspace_path: Option<&str>,
    session_path: Option<&str>,
    session_resource: Option<&str>,
) -> WorkspaceIdentity {
    let segments = workspace_path
        .map(workspace_path_segments)
        .unwrap_or_default();
    let name = workspace_path
        .and_then(|path_text| path_segments_basename(&workspace_path_segments(path_text)))
        .filter(|path_name| !path_name.trim().is_empty())
        .unwrap_or_else(|| clean_workspace_name(workspace_name_hint));
    let (label, group, discriminator) = workspace_label_parts(&name, &segments);
    let key = workspace_path
        .and_then(normalized_workspace_path_key)
        .unwrap_or_else(|| {
            fallback_workspace_key(provider, session_id, session_path, session_resource, &label)
        });

    WorkspaceIdentity {
        key,
        name,
        label,
        group,
        discriminator,
    }
}

fn workspace_label_parts(name: &str, segments: &[String]) -> (String, String, String) {
    if let Some((branch, plugin_name)) = uga_workspace_parts(segments) {
        let display_name = plugin_name.unwrap_or_else(|| name.to_string());
        return (
            label_with_discriminator(&display_name, &branch),
            "UGA".to_string(),
            branch,
        );
    }

    if let Some(plugin_name) = neon_plugin_name(segments) {
        let discriminator = "neon/Plugins".to_string();
        return (
            label_with_discriminator(&plugin_name, &discriminator),
            "neon".to_string(),
            discriminator,
        );
    }

    let parent = nearest_meaningful_parent(segments);
    let discriminator = parent.unwrap_or_else(|| "Unknown".to_string());
    let label = if discriminator == "Unknown" {
        name.to_string()
    } else {
        label_with_discriminator(name, &discriminator)
    };
    let group = semantic_workspace_group(segments).unwrap_or_else(|| discriminator.clone());

    (label, group, discriminator)
}

fn uga_workspace_parts(segments: &[String]) -> Option<(String, Option<String>)> {
    for (index, segment) in segments.iter().enumerate() {
        if !segment.eq_ignore_ascii_case("UGA") {
            continue;
        }

        let branch = segments.get(index + 1)?;
        let plugins_segment = segments.get(index + 2)?;
        if !plugins_segment.eq_ignore_ascii_case("Plugins") || !looks_like_uga_branch(branch) {
            continue;
        }

        let plugin_name = segments
            .get(index + 3)
            .filter(|plugin_segment| !plugin_segment.trim().is_empty())
            .cloned();
        return Some((branch.clone(), plugin_name));
    }

    None
}

fn neon_plugin_name(segments: &[String]) -> Option<String> {
    for (index, segment) in segments.iter().enumerate() {
        if !segment.eq_ignore_ascii_case("neon") {
            continue;
        }

        let plugins_segment = segments.get(index + 1)?;
        if !plugins_segment.eq_ignore_ascii_case("Plugins") {
            continue;
        }

        return segments
            .get(index + 2)
            .filter(|plugin_segment| !plugin_segment.trim().is_empty())
            .cloned();
    }

    None
}

fn looks_like_uga_branch(branch: &str) -> bool {
    let upper_branch = branch.to_ascii_uppercase();
    upper_branch == "DEV"
        || upper_branch
            .strip_prefix("DEV_")
            .map(|suffix| {
                suffix
                    .chars()
                    .all(|branch_char| branch_char.is_ascii_digit() || branch_char == '_')
            })
            .unwrap_or(false)
}

fn workspace_path_segments(path_text: &str) -> Vec<String> {
    path_text
        .trim()
        .replace('\\', "/")
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty() && !is_windows_drive_segment(segment))
        .map(str::to_string)
        .collect()
}

fn path_segments_basename(segments: &[String]) -> Option<String> {
    segments
        .last()
        .filter(|segment| !segment.trim().is_empty())
        .cloned()
}

fn nearest_meaningful_parent(segments: &[String]) -> Option<String> {
    if segments.len() < 2 {
        return None;
    }

    segments[..segments.len() - 1]
        .iter()
        .rev()
        .find(|segment| !is_generic_workspace_name(segment))
        .cloned()
}

fn semantic_workspace_group(segments: &[String]) -> Option<String> {
    for segment in segments.iter().rev() {
        if segment.eq_ignore_ascii_case("UGA") {
            return Some("UGA".to_string());
        }
        if segment.eq_ignore_ascii_case("neon") {
            return Some("neon".to_string());
        }
        if segment.eq_ignore_ascii_case("AiProject") {
            return Some("AiProject".to_string());
        }
        if segment.eq_ignore_ascii_case("Research") {
            return Some("Research".to_string());
        }
    }

    None
}

fn is_generic_workspace_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "plugins" | "source" | "private" | "public" | "content"
    )
}

fn is_windows_drive_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn clean_workspace_name(name: &str) -> String {
    let cleaned_name = clean_label(name, 80);
    if cleaned_name.is_empty() {
        "Unknown".to_string()
    } else {
        cleaned_name
    }
}

fn label_with_discriminator(name: &str, discriminator: &str) -> String {
    if discriminator.trim().is_empty() {
        name.to_string()
    } else {
        format!("{} \u{00b7} {}", name, discriminator)
    }
}

fn normalized_workspace_path_key(path_text: &str) -> Option<String> {
    let mut normalized_path = path_text.trim().replace('\\', "/");
    if normalized_path.is_empty() {
        return None;
    }

    if normalized_path.len() > 2
        && normalized_path.starts_with('/')
        && normalized_path.as_bytes().get(2).copied() == Some(b':')
    {
        normalized_path.remove(0);
    }

    let mut segments = Vec::new();
    for segment in normalized_path.split('/') {
        match segment.trim() {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            clean_segment => segments.push(clean_segment.to_string()),
        }
    }

    if segments.is_empty() {
        return None;
    }

    Some(format!("path:{}", segments.join("/").to_ascii_lowercase()))
}

fn fallback_workspace_key(
    provider: &str,
    session_id: &str,
    session_path: Option<&str>,
    session_resource: Option<&str>,
    workspace_label: &str,
) -> String {
    let source = session_path
        .or(session_resource)
        .unwrap_or(workspace_label)
        .trim()
        .replace('\\', "/")
        .to_ascii_lowercase();
    format!("fallback:{}:{}:{}", provider, session_id, source)
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
        include_copilot_cli: None,
        include_claude: None,
        include_codex: None,
        include_open_code: None,
        workspace_path_blacklist: None,
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
        include_copilot_cli: options.include_copilot_cli.unwrap_or(true),
        include_claude: options.include_claude.unwrap_or(true),
        include_codex: options.include_codex.unwrap_or(true),
        include_open_code: options.include_open_code.unwrap_or(true),
        workspace_path_blacklist: options
            .workspace_path_blacklist
            .unwrap_or_else(default_workspace_path_blacklist)
            .into_iter()
            .filter_map(|entry| resolve_workspace_blacklist_entry(&entry))
            .collect(),
    }
}

fn default_workspace_path_blacklist() -> Vec<String> {
    vec![path_to_string(&env::temp_dir())]
}

fn resolve_workspace_blacklist_entry(entry: &str) -> Option<String> {
    let trimmed = entry.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("%TEMP%") || trimmed.eq_ignore_ascii_case("%TMP%") {
        return Some(path_to_string(&env::temp_dir()));
    }
    Some(trimmed.to_string())
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

fn newest_timestamp_ms(
    current_timestamp_ms: Option<u64>,
    candidate_timestamp_ms: Option<u64>,
) -> Option<u64> {
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
    .find_map(|pointer| {
        json_value
            .pointer(pointer)
            .and_then(timestamp_ms_from_value)
    })
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

fn collect_copilot_carousel_answer(
    carousel_item: &Value,
    answer_value: &Value,
    answer_parts: &mut Vec<String>,
) {
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
    if !workspace_title.is_empty()
        && workspace_title != "Unknown"
        && workspace_title != provider_name
    {
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
    let mut decoded_path = percent_decode(encoded_path).replace('\\', "/");

    if let Some(rest) = decoded_path.strip_prefix("?/") {
        let lower_rest = rest.to_ascii_lowercase();
        if lower_rest.starts_with("unc/") {
            decoded_path = format!("//{}", &rest[4..]);
        } else {
            decoded_path = rest.to_string();
        }
    } else if let Some(rest) = decoded_path.strip_prefix("//?/") {
        let lower_rest = rest.to_ascii_lowercase();
        if lower_rest.starts_with("unc/") {
            decoded_path = format!("//{}", &rest[4..]);
        } else {
            decoded_path = rest.to_string();
        }
    }

    let is_drive_path = decoded_path.len() > 2
        && decoded_path.as_bytes().get(1).copied() == Some(b':')
        && decoded_path.as_bytes()[0].is_ascii_alphabetic();
    let mut path_text = decoded_path.replace('/', "\\");

    if path_text.starts_with('\\')
        && path_text.len() > 2
        && path_text.as_bytes().get(2).copied() == Some(b':')
    {
        path_text.remove(0);
    } else if !is_drive_path && !decoded_path.starts_with('/') && decoded_path.contains('/') {
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
    USER_SYSTEM_ERROR_SUBSTRINGS
        .iter()
        .any(|s| text.contains(s))
}

fn is_ai_model_noise_text(text: &str) -> bool {
    let lower_text = text.to_ascii_lowercase();
    AI_MODEL_NOISE_SUBSTRINGS
        .iter()
        .any(|s| lower_text.contains(s))
}

fn apply_tail_budget(cleaned: &str, max_chars: usize) -> (String, bool) {
    let text = cleaned.trim();
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return (text.to_string(), false);
    }
    let start_char = char_count - max_chars;
    let start_byte = text
        .char_indices()
        .nth(start_char)
        .map(|(i, _)| i)
        .unwrap_or(0);
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
    (
        format!("...{}{}", if tail.contains('\n') { "\n" } else { "" }, tail),
        true,
    )
}

fn apply_user_preview_budget(msg: Option<String>) -> (Option<String>, bool) {
    let Some(text) = msg else {
        return (None, false);
    };
    let (result, trunc) = apply_tail_budget(&text, USER_PREVIEW_MAX_CHARS);
    if result.is_empty() {
        (None, false)
    } else {
        (Some(result), trunc)
    }
}

fn apply_ai_preview_budget(msg: Option<String>) -> (Option<String>, bool, Option<String>) {
    let Some(text) = msg else {
        return (None, false, None);
    };
    let clean = text.trim();
    // Tail-first: show the most recent content of the AI response.
    // No error-keyword prioritization — model/system errors are filtered upstream.
    let (result, trunc) = apply_tail_budget(clean, AI_PREVIEW_MAX_CHARS);
    let excerpt_kind = if trunc {
        Some("tail".to_string())
    } else {
        None
    };
    if result.is_empty() {
        (None, false, None)
    } else {
        (Some(result), trunc, excerpt_kind)
    }
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
    let separator_index = trimmed_timestamp
        .find('T')
        .or_else(|| trimmed_timestamp.find(' '))?;
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

    Some(
        (utc_seconds as u64)
            .saturating_mul(1000)
            .saturating_add(fractional_ms),
    )
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
    spawn_code_cli_in(None, arguments)
}

fn spawn_code_cli_in(current_dir: Option<&str>, arguments: &[String]) -> Result<(), String> {
    let mut candidates = vec![
        "code".to_string(),
        "code.cmd".to_string(),
        "code.exe".to_string(),
    ];

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
        if let Some(current_dir) = clean_option(current_dir) {
            code_command.current_dir(current_dir);
        }

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

fn bridge_version_needs_update(local_version: &str, installed_version: &str) -> bool {
    match (
        parse_version(local_version),
        parse_version(installed_version),
    ) {
        (Some(local), Some(installed)) => local > installed,
        (Some(_), None) => true,
        _ => false,
    }
}

fn get_bridge_extension_inventory(code_path: Option<&str>) -> BridgeExtensionInventory {
    let mut inventory = bridge_extension_inventory_from_extension_folders();

    if let Some(code_path) = code_path {
        if let Ok(cli_inventory) = bridge_extension_inventory_from_cli(code_path) {
            inventory.merge(cli_inventory);
        }
    }

    inventory
}

fn bridge_extension_inventory_from_cli(
    code_path: &str,
) -> Result<BridgeExtensionInventory, String> {
    let mut list_cmd = Command::new(code_path);
    list_cmd.args(["--list-extensions", "--show-versions"]);
    let output = spawn_hidden_with_output(list_cmd)
        .map_err(|error| format!("Failed to list VS Code extensions: {}", error))?;

    if !output.status.success() {
        return Err(format!(
            "VS Code extension list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(bridge_extension_inventory_from_cli_output(
        &String::from_utf8_lossy(&output.stdout),
    ))
}

fn bridge_extension_inventory_from_cli_output(output: &str) -> BridgeExtensionInventory {
    let mut inventory = BridgeExtensionInventory::default();
    for raw_line in output.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        let (extension_id, version) = line
            .split_once('@')
            .map(|(id, version)| (id.trim(), version.trim()))
            .unwrap_or((line, "unknown"));

        if extension_id.eq_ignore_ascii_case(&bridge_extension_id()) {
            inventory.add_stable_version(version.to_string());
        } else if let Some(legacy_id) = legacy_bridge_extension_id(extension_id) {
            inventory.add_legacy_id(&legacy_id);
        }
    }

    inventory
}

fn bridge_extension_inventory_from_extension_folders() -> BridgeExtensionInventory {
    let Some(user_profile_path) = env::var_os("USERPROFILE") else {
        return BridgeExtensionInventory::default();
    };

    let user_profile_path = PathBuf::from(user_profile_path);
    let mut inventory = BridgeExtensionInventory::default();

    for extensions_dir in [
        user_profile_path.join(".vscode").join("extensions"),
        user_profile_path
            .join(".vscode-insiders")
            .join("extensions"),
    ] {
        for entry in read_directory(&extensions_dir) {
            let folder_name = entry.file_name().to_string_lossy().to_string();
            let package_json_path = entry.path().join("package.json");
            let package_json = read_json_file(&package_json_path);
            let package_extension_id = package_json.as_ref().and_then(package_extension_id);

            if package_extension_id
                .as_deref()
                .map(|extension_id| extension_id.eq_ignore_ascii_case(&bridge_extension_id()))
                .unwrap_or(false)
                || extension_folder_version_suffix(&folder_name, &bridge_extension_id()).is_some()
            {
                inventory.add_stable_version(extension_version_from_folder(
                    package_json.as_ref(),
                    &folder_name,
                    &bridge_extension_id(),
                ));
                continue;
            }

            if let Some(legacy_id) = package_extension_id
                .as_deref()
                .and_then(legacy_bridge_extension_id)
                .or_else(|| legacy_bridge_extension_id_from_folder(&folder_name))
            {
                inventory.add_legacy_id(&legacy_id);
            }
        }
    }

    inventory
}

fn package_extension_id(package_json: &Value) -> Option<String> {
    let publisher = string_at(package_json, &["/publisher"])?;
    let name = string_at(package_json, &["/name"])?;
    Some(format!("{}.{}", publisher, name))
}

fn extension_version_from_folder(
    package_json: Option<&Value>,
    folder_name: &str,
    extension_id: &str,
) -> String {
    package_json
        .and_then(|json_value| string_at(json_value, &["/version"]).map(str::to_string))
        .or_else(|| extension_folder_version_suffix(folder_name, extension_id).map(str::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn extension_folder_version_suffix<'a>(
    folder_name: &'a str,
    extension_id: &str,
) -> Option<&'a str> {
    let suffix = if extension_id.eq_ignore_ascii_case(&bridge_extension_id()) {
        let prefix = bridge_extension_folder_prefix();
        folder_name.strip_prefix(&prefix)?
    } else {
        folder_name.strip_prefix(extension_id)?.strip_prefix('-')?
    };
    suffix
        .chars()
        .next()
        .filter(|character| character.is_ascii_digit())?;
    Some(suffix)
}

fn legacy_bridge_extension_id(extension_id: &str) -> Option<String> {
    legacy_bridge_extension_ids()
        .into_iter()
        .find(|legacy_id| extension_id.eq_ignore_ascii_case(legacy_id))
}

fn legacy_bridge_extension_id_from_folder(folder_name: &str) -> Option<String> {
    legacy_bridge_extension_ids()
        .into_iter()
        .find(|legacy_id| extension_folder_version_suffix(folder_name, legacy_id).is_some())
}

fn uninstall_legacy_bridge_extensions(code_path: &str) -> Vec<String> {
    let mut warnings = Vec::new();

    for (index, extension_id) in legacy_bridge_extension_ids().into_iter().enumerate() {
        let legacy_label = format!("legacy extension {}", index + 1);
        let mut uninstall_cmd = Command::new(code_path);
        uninstall_cmd.args(["--uninstall-extension", extension_id.as_str()]);

        match spawn_hidden_with_output(uninstall_cmd) {
            Ok(output)
                if output.status.success() || bridge_uninstall_output_is_not_installed(&output) => {
            }
            Ok(output) => warnings.push(format!(
                "{}: {} {}",
                legacy_label,
                String::from_utf8_lossy(&output.stderr).trim(),
                String::from_utf8_lossy(&output.stdout).trim()
            )),
            Err(error) => warnings.push(format!("{}: {}", legacy_label, error)),
        }
    }

    warnings
}

fn bridge_uninstall_output_is_not_installed(output: &std::process::Output) -> bool {
    let combined_output = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .to_ascii_lowercase();

    combined_output.contains("not installed") || combined_output.contains("not found")
}

fn find_bridge_source_dir() -> Option<PathBuf> {
    bridge_base_dirs()
        .into_iter()
        .map(|base_dir| base_dir.join(bridge_source_dir_name()))
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
                if modified_time_ms(&existing_path).unwrap_or(0)
                    >= modified_time_ms(&path).unwrap_or(0) =>
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

    let vsix_path = package_dir.join(bridge_vsix_file_name(&safe_file_name_token(local_version)));

    let mut package_cmd = Command::new(npx_path);
    package_cmd
        .args([
            "--yes",
            "@vscode/vsce",
            "package",
            "--skip-license",
            "--out",
        ])
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
        Some(PathBuf::from(
            "C:\\Program Files\\Microsoft VS Code\\bin\\code.cmd",
        )),
        Some(PathBuf::from(
            "C:\\Program Files\\Microsoft VS Code\\Code.exe",
        )),
        Some(PathBuf::from(
            "C:\\Program Files (x86)\\Microsoft VS Code\\bin\\code.cmd",
        )),
        Some(PathBuf::from(
            "C:\\Program Files (x86)\\Microsoft VS Code\\Code.exe",
        )),
    ];

    for path in common_paths.into_iter().flatten() {
        if path.exists() {
            return Some(path_to_string(&path));
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
        EnumWindows, GetAncestor, GetWindowThreadProcessId, PrivateExtractIconsW, SendMessageW,
        SetClassLongPtrW, GA_ROOT, GCLP_HICON, GCLP_HICONSM, HICON, ICON_BIG, ICON_SMALL,
        ICON_SMALL2, WM_SETICON,
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
        if hwnd.is_null() || hwnds.contains(&hwnd) {
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
                let _ = SendMessageW(
                    target_hwnd,
                    WM_SETICON,
                    ICON_BIG as usize,
                    big_icon as isize,
                );
                let _ = SetClassLongPtrW(target_hwnd, GCLP_HICON, big_icon as isize);
            }

            if let Some(small_icon) = extract_icon(&exe_path_wide, 32) {
                let _ = SendMessageW(
                    target_hwnd,
                    WM_SETICON,
                    ICON_SMALL as usize,
                    small_icon as isize,
                );
                let _ = SendMessageW(
                    target_hwnd,
                    WM_SETICON,
                    ICON_SMALL2 as usize,
                    small_icon as isize,
                );
                let _ = SetClassLongPtrW(target_hwnd, GCLP_HICONSM, small_icon as isize);
            }
        };

        let hwnd = raw_hwnd.0 as HWND;
        let root_hwnd = GetAncestor(hwnd, GA_ROOT);
        let mut hwnds = Vec::new();
        add_hwnd(&mut hwnds, hwnd);
        add_hwnd(&mut hwnds, root_hwnd);

        let _ = EnumWindows(
            Some(enum_process_windows),
            &mut hwnds as *mut Vec<HWND> as LPARAM,
        );

        for target_hwnd in hwnds {
            apply_icons_to(target_hwnd);
        }
    }
}

#[cfg(target_os = "windows")]
fn apply_native_always_on_top<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
    always_on_top: bool,
) {
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
        if hwnd.is_null() || hwnds.contains(&hwnd) {
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

        let _ = EnumWindows(
            Some(enum_process_windows),
            &mut hwnds as *mut Vec<HWND> as LPARAM,
        );

        let insert_after = if always_on_top {
            HWND_TOPMOST
        } else {
            HWND_NOTOPMOST
        };
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
    fn plugin_workflow_session_marker_requires_explicit_marker() {
        assert!(detect_plugin_workflow_session_marker(
            "copilot",
            &[Some("please fix this Unreal plugin")]
        )
        .is_none());

        let marker = detect_plugin_workflow_session_marker(
            "copilot",
            &[Some(
                "[AgentWatcher Plugin Workflow]\nplugin=SamplePlugin workflow=SampleFlow provider=copilot taskId=aw-001 runId=run-abc contractVersion=plugin-workflow.v1",
            )],
        )
        .unwrap();

        assert_eq!(marker.plugin_name, "SamplePlugin");
        assert_eq!(marker.workflow, "SampleFlow");
        assert_eq!(marker.provider, "copilot");
        assert_eq!(marker.task_id.as_deref(), Some("aw-001"));
        assert_eq!(marker.run_id.as_deref(), Some("run-abc"));
        assert_eq!(
            marker.contract_version.as_deref(),
            Some("plugin-workflow.v1")
        );
    }

    #[test]
    fn bridge_cli_inventory_separates_stable_and_legacy_ids() {
        let stable_id = bridge_extension_id();
        let legacy_id = format!("{}-{}{}", stable_id, "safe", 4);
        let inventory = bridge_extension_inventory_from_cli_output(&format!(
            "{stable_id}@0.1.10\n{legacy_id}@0.1.9\n"
        ));

        assert_eq!(inventory.stable_version.as_deref(), Some("0.1.10"));
        assert_eq!(inventory.legacy_ids, vec![legacy_id]);
    }

    #[test]
    fn bridge_folder_suffix_does_not_treat_legacy_safe_as_stable() {
        let legacy_id = format!("{}-{}{}", bridge_extension_id(), "safe", 4);
        let legacy_folder = format!("{legacy_id}-0.1.9");

        assert_eq!(
            extension_folder_version_suffix(&legacy_folder, &bridge_extension_id()),
            None
        );
        assert_eq!(
            legacy_bridge_extension_id_from_folder(&legacy_folder),
            Some(legacy_id)
        );
    }

    #[test]
    fn todo_state_path_lives_under_agentwatcher_appdata() {
        let path = todo_state_path_from_appdata(Path::new(r"C:\Users\me\AppData\Roaming"));

        assert_eq!(
            path,
            PathBuf::from(r"C:\Users\me\AppData\Roaming\AgentWatcher\todos.v1.json")
        );
    }

    #[test]
    fn missing_todo_state_returns_empty_state() {
        let path = env::temp_dir().join(format!(
            "agentwatcher-missing-todo-state-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let state = read_todo_state_from_path(&path).unwrap();

        assert_eq!(state["version"], 1);
        assert!(state["workspaces"].as_object().unwrap().is_empty());
    }

    #[test]
    fn todo_state_round_trips_json() {
        let dir = env::temp_dir().join(format!(
            "agentwatcher-todo-state-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("todos.v1.json");
        let state = json!({
            "version": 1,
            "workspaces": {
                "path:f:/aiproject/agentwatcher": {
                    "label": "AgentWatcher",
                    "path": r"X:\Workspace\AgentWatcher",
                    "tasks": [
                        { "id": "todo-1", "title": "Build task input" }
                    ]
                }
            }
        });

        write_todo_state_to_path(&path, &state).unwrap();
        let saved = read_todo_state_from_path(&path).unwrap();

        assert_eq!(saved, state);
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn todo_state_write_bounds_plugin_workflow_task_payloads() {
        let dir = env::temp_dir().join(format!(
            "agentwatcher-todo-state-compact-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = dir.join("todos.v1.json");
        let dispatches: Vec<Value> = (0..9)
            .map(|index| {
                json!({
                    "id": format!("dispatch-{index}"),
                    "lastPrompt": "P".repeat(6_000),
                    "sessionPath": "S".repeat(6_000)
                })
            })
            .collect();
        let events: Vec<Value> = (0..20)
            .map(
                |index| json!({ "id": format!("event-{index}"), "type": "phase", "message": "ok" }),
            )
            .collect();
        let state = json!({
            "version": 1,
            "workspaces": {
                "path:f:/aiproject/agentwatcher": {
                    "label": "AgentWatcher",
                    "path": r"X:\Workspace\AgentWatcher",
                    "tasks": [{
                        "id": "todo-1",
                        "title": "Build task input",
                        "dispatches": dispatches,
                        "pluginWorkflowLifecycle": { "events": events }
                    }]
                }
            }
        });

        write_todo_state_to_path(&path, &state).unwrap();
        let saved = read_todo_state_from_path(&path).unwrap();
        let task = &saved["workspaces"]["path:f:/aiproject/agentwatcher"]["tasks"][0];

        assert_eq!(
            task["dispatches"].as_array().unwrap().len(),
            TODO_DISPATCH_HISTORY_LIMIT
        );
        assert_eq!(
            task["pluginWorkflowLifecycle"]["events"]
                .as_array()
                .unwrap()
                .len(),
            TODO_PLUGIN_WORKFLOW_EVENT_LIMIT
        );
        assert!(
            task["dispatches"][0]["lastPrompt"].as_str().unwrap().len() <= TODO_LONG_TEXT_LIMIT
        );

        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn todo_id_extraction_survives_long_prompt_preview_truncation() {
        let mut todo_ids = Vec::new();
        let long_prompt = format!(
            "AgentWatcher 待办 ID: todo-mpz70wv9-afo8hl\n{}",
            "补充说明。".repeat(200)
        );

        collect_todo_ids_from_text(&long_prompt, &mut todo_ids);

        assert_eq!(todo_ids, vec!["todo-mpz70wv9-afo8hl".to_string()]);
        assert!(!apply_user_preview_budget(Some(long_prompt))
            .0
            .unwrap()
            .contains("todo-mpz70wv9-afo8hl"));
    }

    #[test]
    fn copilot_transcript_overlay_collects_todo_ids_from_raw_user_messages() {
        let text = r#"{"type":"user.message","data":{"content":"AgentWatcher 待办 ID: todo-mpz70wv9-afo8hl\n任务正文很长也不影响匹配"}}"#;

        let overlay = copilot_transcript_overlay_from_text(text).unwrap();

        assert_eq!(overlay.todo_ids, vec!["todo-mpz70wv9-afo8hl".to_string()]);
    }

    #[test]
    fn copilot_unanswered_question_carousel_is_waiting() {
        let event = json!({
            "k": ["requests", 0, "response"],
            "v": [{ "kind": "questionCarousel" }]
        });

        assert_eq!(
            copilot_response_status_hint(&event),
            Some(SessionStatusHint::Waiting)
        );
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

        assert_eq!(
            copilot_response_status_hint(&event),
            Some(SessionStatusHint::Running)
        );
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
        assert!(overlay
            .last_user_message
            .unwrap()
            .contains("Please continue."));
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
        assert_eq!(
            overlay.last_user_message.as_deref(),
            Some("Continue after skipping.")
        );
    }

    #[test]
    fn copilot_transcript_later_assistant_message_clears_stale_question() {
        let transcript = r#"
{"type":"assistant.message","data":{"content":"Please choose.","toolRequests":[{"toolCallId":"ask-1","name":"vscode_askQuestions","arguments":{"questions":[{"header":"Old question","question":"This was skipped earlier."}]}}]},"timestamp":"2026-05-30T03:51:00.000Z"}
{"type":"assistant.message","data":{"content":"Continuing after that choice."},"timestamp":"2026-05-30T03:52:00.000Z"}
"#;

        let overlay = copilot_transcript_overlay_from_text(transcript).unwrap();

        assert_eq!(overlay.status_hint, Some(SessionStatusHint::Running));
        assert_eq!(
            overlay.last_ai_message.as_deref(),
            Some("Continuing after that choice.")
        );
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
        assert_eq!(
            latest_activity_ms(Some(100), 200),
            (200, ActivitySource::FileModified)
        );
        assert_eq!(
            latest_activity_ms(Some(300), 200),
            (300, ActivitySource::ContentTimestamp)
        );
        assert_eq!(
            latest_activity_ms(None, 200),
            (200, ActivitySource::FileModified)
        );
    }

    #[test]
    fn workspace_identity_disambiguates_uga_plugin_branch() {
        let identity = build_workspace_identity(
            "copilot",
            "session-1",
            "AesWorld",
            Some(r"F:\ShanghaiP4\neon\UGA\DEV_2\Plugins\AesWorld"),
            None,
            None,
        );

        assert_eq!(
            identity.key,
            "path:f:/shanghaip4/neon/uga/dev_2/plugins/aesworld"
        );
        assert_eq!(identity.name, "AesWorld");
        assert_eq!(identity.label, "AesWorld · DEV_2");
        assert_eq!(identity.group, "UGA");
        assert_eq!(identity.discriminator, "DEV_2");
    }

    #[test]
    fn workspace_identity_disambiguates_generic_plugin_root() {
        let identity = build_workspace_identity(
            "copilot",
            "session-2",
            "Plugins",
            Some(r"F:\ShanghaiP4\neon\UGA\DEV_2\Plugins"),
            None,
            None,
        );

        assert_eq!(identity.key, "path:f:/shanghaip4/neon/uga/dev_2/plugins");
        assert_eq!(identity.name, "Plugins");
        assert_eq!(identity.label, "Plugins · DEV_2");
        assert_eq!(identity.group, "UGA");
        assert_eq!(identity.discriminator, "DEV_2");
    }

    #[test]
    fn workspace_path_key_normalizes_trailing_and_duplicate_separators() {
        assert_eq!(
            normalized_workspace_path_key(r"F:\ShanghaiP4\neon\UGA\DEV_2\Plugins\AesWorld\\"),
            Some("path:f:/shanghaip4/neon/uga/dev_2/plugins/aesworld".to_string())
        );
        assert_eq!(
            normalized_workspace_path_key("/F:/ShanghaiP4//neon/UGA/./DEV_2/Plugins/AesWorld"),
            Some("path:f:/shanghaip4/neon/uga/dev_2/plugins/aesworld".to_string())
        );
    }

    #[test]
    fn decode_file_uri_normalizes_vscode_drive_paths() {
        assert_eq!(
            decode_file_uri("file:///f%3A/AiProject/AgentWatcher")
                .map(|path| path_to_string(&path)),
            Some(r"f:\AiProject\AgentWatcher".to_string())
        );
        assert_eq!(
            decode_file_uri("file://%3F/f%3A/AiProject/AgentWatcher")
                .map(|path| path_to_string(&path)),
            Some(r"f:\AiProject\AgentWatcher".to_string())
        );
    }

    #[test]
    fn decode_file_uri_normalizes_vscode_unc_paths() {
        assert_eq!(
            decode_file_uri("file://%3F/UNC/server/share/AgentWatcher")
                .map(|path| path_to_string(&path)),
            Some(r"\\server\share\AgentWatcher".to_string())
        );
    }

    #[test]
    fn scan_options_include_codex_by_default_and_can_disable_it() {
        assert!(resolve_scan_options(None).include_codex);

        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: None,
            active_window_days: None,
            hide_archived: None,
            include_copilot: None,
            include_copilot_cli: None,
            include_claude: None,
            include_codex: Some(false),
            include_open_code: None,
            workspace_path_blacklist: None,
        }));

        assert!(!options.include_codex);
    }

    #[test]
    fn scan_options_control_copilot_chat_and_cli_independently() {
        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: None,
            active_window_days: None,
            hide_archived: None,
            include_copilot: Some(false),
            include_copilot_cli: Some(true),
            include_claude: None,
            include_codex: None,
            include_open_code: None,
            workspace_path_blacklist: None,
        }));

        assert!(!options.include_copilot);
        assert!(options.include_copilot_cli);
    }

    #[test]
    fn session_limit_is_applied_per_provider_without_starving_others() {
        fn session(id: &str, provider: &str, updated_ms: u64) -> AgentSession {
            AgentSession {
                id: id.to_string(),
                provider: provider.to_string(),
                provider_label: provider.to_string(),
                title: id.to_string(),
                workspace: "workspace".to_string(),
                workspace_path: None,
                workspace_key: "workspace".to_string(),
                workspace_name: "workspace".to_string(),
                workspace_label: "workspace".to_string(),
                workspace_group: String::new(),
                workspace_discriminator: String::new(),
                session_path: None,
                session_resource: None,
                status: "idle".to_string(),
                time_label: String::new(),
                updated_ms,
                message_count: 0,
                last_user_message: None,
                last_user_message_truncated: false,
                last_ai_message: None,
                last_ai_message_truncated: false,
                last_ai_message_excerpt_kind: None,
                branch: None,
                todo_ids: Vec::new(),
                plugin_workflow: None,
            }
        }

        let mut sessions = vec![
            session("claude-3", "claude", 300),
            session("claude-2", "claude", 200),
            session("opencode-2", "opencode", 190),
            session("claude-1", "claude", 100),
            session("opencode-1", "opencode", 90),
            session("codex-1", "codex", 80),
        ];

        apply_session_limit_per_provider(&mut sessions, 2);

        assert_eq!(
            sessions
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "claude-3",
                "claude-2",
                "opencode-2",
                "opencode-1",
                "codex-1"
            ]
        );
    }

    #[test]
    fn workspace_path_blacklist_filters_directory_and_descendants_only() {
        fn session(id: &str, workspace_path: Option<&str>) -> AgentSession {
            AgentSession {
                id: id.to_string(),
                provider: "claude".to_string(),
                provider_label: "C".to_string(),
                title: id.to_string(),
                workspace: "workspace".to_string(),
                workspace_path: workspace_path.map(str::to_string),
                workspace_key: "workspace".to_string(),
                workspace_name: "workspace".to_string(),
                workspace_label: "workspace".to_string(),
                workspace_group: String::new(),
                workspace_discriminator: String::new(),
                session_path: None,
                session_resource: None,
                status: "idle".to_string(),
                time_label: String::new(),
                updated_ms: 0,
                message_count: 0,
                last_user_message: None,
                last_user_message_truncated: false,
                last_ai_message: None,
                last_ai_message_truncated: false,
                last_ai_message_excerpt_kind: None,
                branch: None,
                todo_ids: Vec::new(),
                plugin_workflow: None,
            }
        }

        let mut sessions = vec![
            session("temp-root", Some(r"C:\Users\YUMEI\AppData\Local\Temp")),
            session(
                "temp-child",
                Some(r"c:/users/yumei/appdata/local/temp/run/task"),
            ),
            session(
                "similar-prefix",
                Some(r"C:\Users\YUMEI\AppData\Local\TemporaryProject"),
            ),
            session("normal", Some(r"F:\AiProject\AgentWatcher")),
            session("unknown", None),
        ];

        apply_workspace_path_blacklist(
            &mut sessions,
            &[r"C:\Users\YUMEI\AppData\Local\Temp\.".to_string()],
        );

        assert_eq!(
            sessions
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            vec!["similar-prefix", "normal", "unknown"]
        );
    }

    #[test]
    fn workspace_path_blacklist_defaults_to_system_temp_and_respects_explicit_empty_list() {
        let defaults = resolve_scan_options(None);
        assert_eq!(
            defaults.workspace_path_blacklist,
            default_workspace_path_blacklist()
        );

        let explicit_empty = resolve_scan_options(Some(ScanOptions {
            max_sessions: None,
            active_window_days: None,
            hide_archived: None,
            include_copilot: None,
            include_copilot_cli: None,
            include_claude: None,
            include_codex: None,
            include_open_code: None,
            workspace_path_blacklist: Some(Vec::new()),
        }));
        assert!(explicit_empty.workspace_path_blacklist.is_empty());
    }

    #[test]
    fn scan_options_include_opencode_by_default_and_can_disable_it() {
        assert!(resolve_scan_options(None).include_open_code);

        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: None,
            active_window_days: None,
            hide_archived: None,
            include_copilot: None,
            include_copilot_cli: None,
            include_claude: None,
            include_codex: None,
            include_open_code: Some(false),
            workspace_path_blacklist: None,
        }));

        assert!(!options.include_open_code);
    }

    #[test]
    fn codex_status_maps_app_server_thread_states() {
        let scan_time_ms = 1_780_653_500_000;
        let recent_updated_ms = scan_time_ms - 60_000;
        let old_updated_ms = scan_time_ms - STATUS_RECENT_CONTENT_WINDOW_MS - 1;

        assert_eq!(
            codex_status_from_thread(
                &json!({
                    "status": { "type": "active", "activeFlags": ["waitingOnUserInput"] }
                }),
                old_updated_ms,
                scan_time_ms
            ),
            "waiting"
        );
        assert_eq!(
            codex_status_from_thread(
                &json!({
                    "status": { "type": "active", "activeFlags": [] }
                }),
                old_updated_ms,
                scan_time_ms
            ),
            "running"
        );
        assert_eq!(
            codex_status_from_thread(
                &json!({ "status": "notLoaded" }),
                recent_updated_ms,
                scan_time_ms
            ),
            "running"
        );
        assert_eq!(
            codex_status_from_thread(
                &json!({
                    "status": "notLoaded",
                    "turns": [
                        { "status": "completed", "startedAt": recent_updated_ms, "completedAt": recent_updated_ms + 1 }
                    ]
                }),
                recent_updated_ms,
                scan_time_ms
            ),
            "idle"
        );
        assert_eq!(
            codex_status_from_thread(
                &json!({ "status": "notLoaded" }),
                old_updated_ms,
                scan_time_ms
            ),
            "idle"
        );
    }

    #[test]
    fn codex_thread_resource_percent_encodes_thread_id() {
        assert_eq!(
            codex_thread_resource("thread/with space"),
            "codex://threads/thread%2Fwith%20space"
        );
    }

    #[test]
    fn codex_jsonl_session_id_reads_rollout_uuid_tail() {
        let path = Path::new(
            r"C:\Users\me\.codex\sessions\2026\06\23\rollout-2026-06-23T16-41-04-019ef3a3-cb3d-7632-beb5-f023ed79ab1e.jsonl",
        );

        assert_eq!(
            codex_session_id_from_path(path).as_deref(),
            Some("019ef3a3-cb3d-7632-beb5-f023ed79ab1e")
        );
    }

    #[test]
    fn codex_jsonl_session_fallback_reads_local_file_summary() {
        let session_id = "019ef3a3-cb3d-7632-beb5-f023ed79ab1e";
        let temp_dir =
            env::temp_dir().join(format!("agentwatcher-codex-jsonl-{}", current_time_ms()));
        fs::create_dir_all(&temp_dir).unwrap();
        let session_path =
            temp_dir.join(format!("rollout-2026-06-23T16-41-04-{}.jsonl", session_id));
        let jsonl = [
            json!({
                "timestamp": "2026-06-29T10:00:00Z",
                "type": "session_meta",
                "payload": {
                    "id": session_id,
                    "cwd": r"F:\ShanghaiP4\neon\Plugins\AesWorld"
                }
            })
            .to_string(),
            json!({
                "timestamp": "2026-06-29T10:01:00Z",
                "type": "response_item",
                "payload": {
                    "type": "message",
                    "id": "msg_should_not_be_session_id",
                    "role": "user",
                    "content": [
                        { "type": "input_text", "text": "# AGENTS.md instructions\nignore bootstrap context" }
                    ]
                }
            })
            .to_string(),
            json!({
                "timestamp": "2026-06-29T10:02:00Z",
                "type": "event_msg",
                "payload": {
                    "type": "user_message",
                    "message": "继续调查 AesWorld 渲染问题 todo-aw-codex"
                }
            })
            .to_string(),
            json!({
                "timestamp": "2026-06-29T10:03:00Z",
                "type": "event_msg",
                "payload": {
                    "type": "agent_message",
                    "message": "已经定位到本地 JSONL 会话。"
                }
            })
            .to_string(),
        ]
        .join("\n");
        fs::write(&session_path, jsonl).unwrap();

        let scan_time_ms = current_time_ms();
        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: Some(80),
            active_window_days: Some(7),
            hide_archived: Some(true),
            include_copilot: Some(false),
            include_copilot_cli: Some(false),
            include_claude: Some(false),
            include_codex: Some(true),
            include_open_code: Some(false),
            workspace_path_blacklist: None,
        }));
        let session = read_codex_jsonl_session(&session_path, scan_time_ms, &options).unwrap();
        let expected_session_path = path_to_string(&session_path);

        assert_eq!(session.id, format!("codex:{}", session_id));
        assert_eq!(session.provider, "codex");
        assert_eq!(
            session.workspace_path.as_deref(),
            Some(r"F:\ShanghaiP4\neon\Plugins\AesWorld")
        );
        assert_eq!(
            session.session_path.as_deref(),
            Some(expected_session_path.as_str())
        );
        assert_eq!(
            session.session_resource.as_deref(),
            Some("codex://threads/019ef3a3-cb3d-7632-beb5-f023ed79ab1e")
        );
        assert_eq!(
            session.last_user_message.as_deref(),
            Some("继续调查 AesWorld 渲染问题 todo-aw-codex")
        );
        assert_eq!(
            session.last_ai_message.as_deref(),
            Some("已经定位到本地 JSONL 会话。")
        );
        assert_eq!(session.todo_ids, vec!["todo-aw-codex".to_string()]);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn codex_jsonl_subagent_session_is_not_a_top_level_card() {
        let session_id = "019f1665-712a-7eb1-8c13-3d002c9b8bcc";
        let temp_dir =
            env::temp_dir().join(format!("agentwatcher-codex-subagent-{}", current_time_ms()));
        fs::create_dir_all(&temp_dir).unwrap();
        let session_path =
            temp_dir.join(format!("rollout-2026-06-30T10-39-39-{}.jsonl", session_id));
        let jsonl = [
            json!({
                "timestamp": "2026-06-30T02:39:55.532Z",
                "type": "session_meta",
                "payload": {
                    "id": session_id,
                    "parent_thread_id": "019ef3a3-cb3d-7632-beb5-f023ed79ab1e",
                    "cwd": r"F:\ShanghaiP4\neon\Plugins\AesWorld",
                    "source": {
                        "subagent": {
                            "thread_spawn": {
                                "parent_thread_id": "019ef3a3-cb3d-7632-beb5-f023ed79ab1e",
                                "depth": 1,
                                "agent_role": "ue-reviewer"
                            }
                        }
                    },
                    "thread_source": "subagent",
                    "agent_role": "ue-reviewer"
                }
            })
            .to_string(),
            json!({
                "timestamp": "2026-06-30T02:39:56.639Z",
                "type": "event_msg",
                "payload": {
                    "type": "user_message",
                    "message": "只读评审。工作区是 Host：F:\\ShanghaiP4\\neon\\Hosts\\W-neon-dev1\\T-hier-anchor-rebase_Host\\Plugins\\AesWorld。"
                }
            })
            .to_string(),
        ]
        .join("\n");
        fs::write(&session_path, jsonl).unwrap();

        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: Some(80),
            active_window_days: Some(7),
            hide_archived: Some(true),
            include_copilot: Some(false),
            include_copilot_cli: Some(false),
            include_claude: Some(false),
            include_codex: Some(true),
            include_open_code: Some(false),
            workspace_path_blacklist: None,
        }));

        assert!(read_codex_jsonl_session(&session_path, current_time_ms(), &options).is_none());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn opencode_session_resource_percent_encodes_session_id() {
        assert_eq!(
            opencode_session_resource("session/with space"),
            "opencode://sessions/session%2Fwith%20space"
        );
        assert_eq!(
            opencode_session_id_from_resource("opencode://sessions/session%2Fwith%20space")
                .as_deref(),
            Some("session/with space")
        );
    }

    #[test]
    fn session_bridge_link_scopes_resource_to_workspace() {
        let session = OpenSessionRequest {
            id: None,
            provider: None,
            workspace_path: Some(r"F:\Projects\AesWorld".to_string()),
            session_resource: Some("vscode-chat-session://local/session-abc".to_string()),
        };
        let ack_path =
            Path::new(r"C:\Users\pb763\AppData\Local\Temp\AgentWatcher\handoff-acks\abc.json");
        let link = session_bridge_link(&session, Some(ack_path), Some("token-abc")).unwrap();
        let bridge_id = format!(
            "{}.{}",
            "agentwatcher",
            ["agentwatcher-vscode", "-session", "-bridge"].concat()
        );
        let expected_prefix = format!("vscode://{bridge_id}/open?target=editor&scopedResource=");

        assert!(link.starts_with(&expected_prefix));
        assert!(link.contains("scopedResource=vscode-chat-session%3A%2F%2Flocal%2Fsession-abc"));
        assert!(link.contains("workspacePath=F%3A%5CProjects%5CAesWorld"));
        assert!(link.contains("ackToken=token-abc"));
        assert!(
            !link.contains("&resource="),
            "old resource= parameter lets stale bridge builds open sessions in the wrong workspace"
        );
    }

    #[test]
    fn opencode_desktop_deep_links_match_supported_routes() {
        assert_eq!(
            opencode_open_project_deep_link(r"X:\Workspace\AgentWatcher"),
            "opencode://open-project?directory=X%3A%5CWorkspace%5CAgentWatcher"
        );
        assert_eq!(
            opencode_new_session_deep_link(r"X:\Workspace\AgentWatcher", "hello todo-awoc-abc123"),
            "opencode://new-session?directory=X%3A%5CWorkspace%5CAgentWatcher&prompt=hello%20todo-awoc-abc123"
        );
    }

    #[test]
    fn opencode_web_session_url_matches_renderer_route_slug() {
        assert_eq!(
            opencode_web_directory_slug(r"X:\Workspace\DevFlow"),
            "WDpcV29ya3NwYWNlXERldkZsb3c"
        );
        assert_eq!(
            opencode_web_session_url(49348, r"X:\Workspace\DevFlow", "ses_test"),
            "http://127.0.0.1:49348/WDpcV29ya3NwYWNlXERldkZsb3c/session/ses_test"
        );
    }

    #[test]
    fn opencode_netstat_parser_keeps_only_listening_opencode_ports() {
        let mut opencode_process_ids = HashSet::new();
        opencode_process_ids.insert(2040);
        opencode_process_ids.insert(72592);
        let output = r#"
  Proto  Local Address          Foreign Address        State           PID
  TCP    127.0.0.1:49348        0.0.0.0:0              LISTENING       2040
  TCP    127.0.0.1:60670        0.0.0.0:0              LISTENING       28660
  TCP    [::1]:51511            [::]:0                 LISTENING       72592
  TCP    127.0.0.1:49348        127.0.0.1:50000        ESTABLISHED     2040
"#;

        assert_eq!(
            opencode_web_server_ports_from_netstat_output(output, &opencode_process_ids),
            vec![49348, 51511]
        );
    }

    #[test]
    fn opencode_handoff_source_markdown_contains_portable_transcript() {
        let row = OpenCodeSessionRow {
            id: "ses_test".to_string(),
            title: "Test handoff".to_string(),
            directory: r"X:\Workspace\AgentWatcher".to_string(),
            path: None,
            time_created: 1000,
            time_updated: 2000,
            time_archived: None,
        };
        let messages = vec![
            OpenCodeTranscriptMessage {
                id: "msg_user".to_string(),
                role: "user".to_string(),
                time_ms: 1100,
                parts: vec![OpenCodeTranscriptPart {
                    label: "text".to_string(),
                    text: "Please continue todo-awoc-123456".to_string(),
                }],
            },
            OpenCodeTranscriptMessage {
                id: "msg_assistant".to_string(),
                role: "assistant".to_string(),
                time_ms: 1200,
                parts: vec![
                    OpenCodeTranscriptPart {
                        label: "tool".to_string(),
                        text: "[tool: shell status=completed callID=call_1]\nraw output:\npassed"
                            .to_string(),
                    },
                    OpenCodeTranscriptPart {
                        label: "text".to_string(),
                        text: "Done and verified.".to_string(),
                    },
                ],
            },
        ];

        let markdown = render_opencode_handoff_source_markdown(&row, &messages);

        assert!(markdown.contains("Session ID: ses_test"));
        assert!(markdown.contains("Please continue todo-awoc-123456"));
        assert!(markdown.contains("[tool: shell status=completed callID=call_1]"));
        assert!(markdown.contains("Done and verified."));
        assert!(opencode_handoff_inline_summary(&row, &messages)
            .unwrap()
            .contains("Exported message count: 2."));
    }

    #[test]
    fn opencode_handoff_part_renderer_omits_reasoning_text() {
        let part = opencode_transcript_part_from_data(
            r#"{"type":"reasoning","text":"private chain of thought"}"#,
        )
        .unwrap();

        assert_eq!(part.label, "reasoning");
        assert!(!part.text.contains("private chain of thought"));
        assert!(part.text.contains("reasoning omitted"));
    }

    #[test]
    fn opencode_session_row_maps_to_agent_session() {
        let scan_time_ms = 1_780_651_255_000;
        let row = OpenCodeSessionRow {
            id: "ses_test".to_string(),
            title: "Implement OpenCode provider".to_string(),
            directory: r"X:\Workspace\AgentWatcher".to_string(),
            path: Some("AiProject/AgentWatcher".to_string()),
            time_created: scan_time_ms - 600_000,
            time_updated: scan_time_ms - 120_000,
            time_archived: None,
        };
        let summary = OpenCodeSessionSummary {
            last_user_message: Some("Please wire OpenCode todo-awoc-abc123".to_string()),
            last_ai_message: Some("OpenCode scanner is ready.".to_string()),
            message_count: 2,
            latest_part_ms: scan_time_ms - 60_000,
            status_hint: None,
            todo_ids: vec!["todo-awoc-abc123".to_string()],
        };
        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: Some(80),
            active_window_days: Some(7),
            hide_archived: Some(true),
            include_copilot: Some(false),
            include_copilot_cli: Some(false),
            include_claude: Some(false),
            include_codex: Some(false),
            include_open_code: Some(true),
            workspace_path_blacklist: None,
        }));

        let session = read_opencode_session_row(&row, summary, scan_time_ms, &options).unwrap();

        assert_eq!(session.provider, "opencode");
        assert_eq!(session.provider_label, "OC");
        assert_eq!(session.title, "Implement OpenCode provider");
        assert_eq!(session.workspace_name, "AgentWatcher");
        assert_eq!(session.status, "running");
        assert_eq!(
            session.session_resource.as_deref(),
            Some("opencode://sessions/ses_test")
        );
        assert_eq!(session.todo_ids, vec!["todo-awoc-abc123".to_string()]);
    }

    #[test]
    fn opencode_summary_reads_user_ai_text_and_todo_id() {
        let connection = opencode_memory_connection();
        connection
            .execute(
                "insert into message (id, session_id, time_created, time_updated, data) values (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    "msg_user",
                    "ses_1",
                    1_000_i64,
                    1_000_i64,
                    json!({ "role": "user", "time": { "created": 1_000 } }).to_string()
                ],
            )
            .unwrap();
        connection
            .execute(
                "insert into message (id, session_id, time_created, time_updated, data) values (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    "msg_assistant",
                    "ses_1",
                    2_000_i64,
                    2_000_i64,
                    json!({ "role": "assistant", "time": { "created": 2_000, "completed": 2_500 }, "finish": "stop" }).to_string()
                ],
            )
            .unwrap();
        connection
            .execute(
                "insert into part (id, message_id, session_id, time_created, time_updated, data) values (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    "part_user",
                    "msg_user",
                    "ses_1",
                    1_000_i64,
                    1_100_i64,
                    json!({ "type": "text", "text": "Start OpenCode task todo-awoc-abc123" }).to_string()
                ],
            )
            .unwrap();
        connection
            .execute(
                "insert into part (id, message_id, session_id, time_created, time_updated, data) values (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    "part_assistant",
                    "msg_assistant",
                    "ses_1",
                    2_000_i64,
                    2_100_i64,
                    json!({ "type": "text", "text": "OpenCode reply is visible." }).to_string()
                ],
            )
            .unwrap();

        let summary = opencode_session_summary(&connection, "ses_1");

        assert_eq!(summary.message_count, 2);
        assert_eq!(summary.latest_part_ms, 2_100);
        assert_eq!(
            summary.last_user_message.as_deref(),
            Some("Start OpenCode task todo-awoc-abc123")
        );
        assert_eq!(
            summary.last_ai_message.as_deref(),
            Some("OpenCode reply is visible.")
        );
        assert_eq!(summary.todo_ids, vec!["todo-awoc-abc123".to_string()]);
    }

    #[test]
    fn opencode_summary_budget_uses_lightweight_fallback_then_cache() {
        let connection = opencode_memory_connection();
        let session_id = "ses_budget_test";
        connection
            .execute(
                "insert into message (id, session_id, time_created, time_updated, data) values (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    "msg_budget_user",
                    session_id,
                    1_000_i64,
                    1_000_i64,
                    json!({ "role": "user", "time": { "created": 1_000 } }).to_string()
                ],
            )
            .unwrap();
        connection
            .execute(
                "insert into part (id, message_id, session_id, time_created, time_updated, data) values (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    "part_budget_user",
                    "msg_budget_user",
                    session_id,
                    1_000_i64,
                    1_100_i64,
                    json!({ "type": "text", "text": "Budgeted OpenCode detail" }).to_string()
                ],
            )
            .unwrap();
        let row = OpenCodeSessionRow {
            id: session_id.to_string(),
            title: "Budget session".to_string(),
            directory: r"X:\Workspace\AgentWatcher".to_string(),
            path: None,
            time_created: 900,
            time_updated: 1_000,
            time_archived: None,
        };

        let mut no_budget = 0usize;
        let lightweight =
            cached_or_read_opencode_session_summary(&connection, &row, &mut no_budget);
        assert_eq!(lightweight.message_count, 0);
        assert_eq!(lightweight.latest_part_ms, 1_000);

        let mut one_fetch = 1usize;
        let full = cached_or_read_opencode_session_summary(&connection, &row, &mut one_fetch);
        assert_eq!(one_fetch, 0);
        assert_eq!(full.message_count, 1);
        assert_eq!(
            full.last_user_message.as_deref(),
            Some("Budgeted OpenCode detail")
        );

        let mut no_budget_after_cache = 0usize;
        let cached =
            cached_or_read_opencode_session_summary(&connection, &row, &mut no_budget_after_cache);
        assert_eq!(cached.message_count, 1);
        assert_eq!(no_budget_after_cache, 0);
    }

    #[test]
    fn opencode_live_scan_perf_when_enabled() {
        if env::var("AGENTWATCHER_OPENCODE_LIVE_PERF").ok().as_deref() != Some("1") {
            return;
        }

        clear_opencode_scan_cache();
        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: Some(80),
            active_window_days: Some(7),
            hide_archived: Some(true),
            include_copilot: Some(false),
            include_copilot_cli: Some(false),
            include_claude: Some(false),
            include_codex: Some(false),
            include_open_code: Some(true),
            workspace_path_blacklist: None,
        }));
        let start = Instant::now();
        let first_scan = scan_opencode_sessions(current_time_ms(), &options);
        let first_ms = start.elapsed().as_millis();
        let start = Instant::now();
        let cached_scan = scan_opencode_sessions(current_time_ms(), &options);
        let cached_ms = start.elapsed().as_millis();

        println!(
            "OpenCode live scan: first={}ms cached={}ms sessions={} cached_sessions={}",
            first_ms,
            cached_ms,
            first_scan.len(),
            cached_scan.len()
        );
        assert_eq!(first_scan.len(), cached_scan.len());
    }

    #[test]
    fn opencode_status_maps_running_tool_and_permission_waiting() {
        assert_eq!(
            opencode_part_status_hint(&json!({
                "type": "tool",
                "tool": "bash",
                "state": { "status": "running" }
            })),
            Some(SessionStatusHint::Running)
        );
        assert_eq!(
            opencode_part_status_hint(&json!({
                "type": "tool",
                "tool": "permission",
                "state": { "status": "pending", "title": "Permission request" }
            })),
            Some(SessionStatusHint::Waiting)
        );
    }

    fn opencode_memory_connection() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute(
                "create table message (
                    id text primary key,
                    session_id text not null,
                    time_created integer not null,
                    time_updated integer not null,
                    data text not null
                )",
                [],
            )
            .unwrap();
        connection
            .execute(
                "create table part (
                    id text primary key,
                    message_id text not null,
                    session_id text not null,
                    time_created integer not null,
                    time_updated integer not null,
                    data text not null
                )",
                [],
            )
            .unwrap();
        connection
    }

    #[test]
    fn codex_thread_summary_reads_app_server_thread_json() {
        let scan_time_ms = 1_780_651_255_000;
        let updated_ms = scan_time_ms - STATUS_RECENT_CONTENT_WINDOW_MS - 1;
        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: Some(80),
            active_window_days: Some(7),
            hide_archived: Some(true),
            include_copilot: Some(false),
            include_copilot_cli: Some(false),
            include_claude: Some(false),
            include_codex: Some(true),
            include_open_code: Some(false),
            workspace_path_blacklist: None,
        }));
        let thread = json!({
            "id": "019e9715-ea58-74f2-af8f-feca74ba9d08",
            "preview": "Wire Codex desktop support",
            "status": "notLoaded",
            "updatedAt": updated_ms,
            "cwd": r"X:\Workspace\AgentWatcher",
            "path": r"C:\Users\me\.codex\sessions\2026\06\05\thread.jsonl",
            "turns": [
                {
                    "id": "turn-new",
                    "status": "completed",
                    "startedAt": updated_ms,
                    "completedAt": updated_ms + 1,
                    "items": [
                        {
                            "type": "userMessage",
                            "content": [
                                { "type": "text", "text": "Latest Codex Todo handoff todo-mpz70wv9-afo8hl" }
                            ]
                        },
                        {
                            "type": "agentMessage",
                            "text": "Latest AI reply."
                        }
                    ]
                },
                {
                    "id": "turn-old",
                    "status": "completed",
                    "startedAt": updated_ms - 60_000,
                    "completedAt": updated_ms - 59_000,
                    "items": [
                        {
                            "type": "userMessage",
                            "content": [
                                { "type": "text", "text": "First prompt should not win" }
                            ]
                        },
                        {
                            "type": "agentMessage",
                            "text": "Old AI reply."
                        }
                    ]
                }
            ]
        });

        let session = read_codex_thread(&thread, scan_time_ms, &options).unwrap();

        assert_eq!(session.id, "codex:019e9715-ea58-74f2-af8f-feca74ba9d08");
        assert_eq!(session.provider, "codex");
        assert_eq!(session.provider_label, "CX");
        assert_eq!(session.status, "idle");
        assert_eq!(
            session.workspace_path.as_deref(),
            Some(r"X:\Workspace\AgentWatcher")
        );
        assert_eq!(
            session.session_resource.as_deref(),
            Some("codex://threads/019e9715-ea58-74f2-af8f-feca74ba9d08")
        );
        assert_eq!(session.message_count, 4);
        assert_eq!(
            session.last_user_message.as_deref(),
            Some("Latest Codex Todo handoff todo-mpz70wv9-afo8hl")
        );
        assert_eq!(session.last_ai_message.as_deref(), Some("Latest AI reply."));
        assert_eq!(session.todo_ids, vec!["todo-mpz70wv9-afo8hl".to_string()]);
    }

    #[test]
    fn codex_app_server_subagent_thread_is_not_a_top_level_card() {
        let scan_time_ms = 1_780_651_255_000;
        let updated_ms = scan_time_ms - 60_000;
        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: Some(80),
            active_window_days: Some(7),
            hide_archived: Some(true),
            include_copilot: Some(false),
            include_copilot_cli: Some(false),
            include_claude: Some(false),
            include_codex: Some(true),
            include_open_code: Some(false),
            workspace_path_blacklist: None,
        }));
        let thread = json!({
            "id": "019f1665-d477-7443-9248-4068f48b2ffb",
            "preview": "只读评审。工作区是 Host：F:\\ShanghaiP4\\neon\\Hosts\\W-neon-dev1",
            "status": "notLoaded",
            "updatedAt": updated_ms,
            "cwd": r"F:\ShanghaiP4\neon\Plugins\AesWorld",
            "parent_thread_id": "019ef3a3-cb3d-7632-beb5-f023ed79ab1e",
            "thread_source": "subagent",
            "source": {
                "subagent": {
                    "thread_spawn": {
                        "parent_thread_id": "019ef3a3-cb3d-7632-beb5-f023ed79ab1e",
                        "depth": 1,
                        "agent_role": "critic"
                    }
                }
            },
            "turns": [
                {
                    "id": "turn-subagent",
                    "status": "completed",
                    "startedAt": updated_ms,
                    "completedAt": updated_ms + 1,
                    "items": [
                        {
                            "type": "userMessage",
                            "content": [
                                { "type": "text", "text": "只读评审。工作区是 Host。" }
                            ]
                        }
                    ]
                }
            ]
        });

        assert!(read_codex_thread(&thread, scan_time_ms, &options).is_none());
    }

    #[test]
    fn codex_thread_without_turn_summary_does_not_use_first_preview_as_last_user() {
        let scan_time_ms = 1_780_651_255_000;
        let updated_ms = scan_time_ms - STATUS_RECENT_CONTENT_WINDOW_MS - 1;
        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: Some(80),
            active_window_days: Some(7),
            hide_archived: Some(true),
            include_copilot: Some(false),
            include_copilot_cli: Some(false),
            include_claude: Some(false),
            include_codex: Some(true),
            include_open_code: Some(false),
            workspace_path_blacklist: None,
        }));
        let thread = json!({
            "id": "thread-preview-only",
            "preview": "This is the first prompt, not the latest user message",
            "status": "notLoaded",
            "updatedAt": updated_ms,
            "cwd": r"X:\Workspace\AgentWatcher"
        });

        let session =
            read_codex_thread_with_summary(&thread, scan_time_ms, &options, None).unwrap();

        assert_eq!(
            session.title,
            "This is the first prompt, not the latest user message"
        );
        assert_eq!(session.last_user_message, None);
        assert_eq!(session.last_ai_message, None);
        assert_eq!(session.message_count, 0);
    }

    #[test]
    fn codex_thread_uses_cached_turn_summary_for_preview_fields() {
        let scan_time_ms = 1_780_651_255_000;
        let updated_ms = scan_time_ms - STATUS_RECENT_CONTENT_WINDOW_MS - 1;
        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: Some(80),
            active_window_days: Some(7),
            hide_archived: Some(true),
            include_copilot: Some(false),
            include_copilot_cli: Some(false),
            include_claude: Some(false),
            include_codex: Some(true),
            include_open_code: Some(false),
            workspace_path_blacklist: None,
        }));
        let thread = json!({
            "id": "thread-cached-summary",
            "preview": "First prompt should stay title-only",
            "status": "notLoaded",
            "updatedAt": updated_ms,
            "cwd": r"X:\Workspace\AgentWatcher"
        });
        let summary = CodexTurnSummary {
            thread_updated_ms: updated_ms,
            fetched_ms: scan_time_ms,
            last_user_message: Some("Actually latest user input".to_string()),
            last_ai_message: Some("Actually latest AI reply".to_string()),
            message_count: 2,
            latest_turn_status: Some("completed".to_string()),
        };

        let session =
            read_codex_thread_with_summary(&thread, scan_time_ms, &options, Some(&summary))
                .unwrap();

        assert_eq!(session.status, "idle");
        assert_eq!(
            session.last_user_message.as_deref(),
            Some("Actually latest user input")
        );
        assert_eq!(
            session.last_ai_message.as_deref(),
            Some("Actually latest AI reply")
        );
        assert_eq!(session.message_count, 2);
    }

    #[test]
    fn copilot_cli_session_store_rows_are_scanned_without_chat_duplicates() {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE sessions (
                    id TEXT PRIMARY KEY,
                    cwd TEXT,
                    repository TEXT,
                    host_type TEXT,
                    branch TEXT,
                    summary TEXT,
                    agent_name TEXT,
                    agent_description TEXT,
                    created_at TEXT,
                    updated_at TEXT
                );
                CREATE TABLE turns (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    turn_index INTEGER NOT NULL,
                    user_message TEXT,
                    assistant_response TEXT,
                    timestamp TEXT
                );",
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO sessions VALUES (?1, ?2, NULL, 'vscode', 'dev', ?3, 'copilotcli', NULL, ?4, ?5)",
                rusqlite::params![
                    "cli-session-1",
                    r"F:\AiProject\AgentWatcher",
                    "Latest CLI summary",
                    "2026-08-05T01:00:00.000Z",
                    "2026-08-05T02:00:00.000Z"
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO sessions VALUES ('chat-session-1', NULL, NULL, 'vscode', NULL, NULL, 'GitHub Copilot Chat', NULL, ?1, ?1)",
                ["2026-08-05T02:00:00.000Z"],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO turns (session_id, turn_index, user_message, assistant_response, timestamp) VALUES (?1, 0, ?2, ?3, ?4)",
                rusqlite::params![
                    "cli-session-1",
                    "Please inspect the CLI session store",
                    "I found the missing session source.",
                    "2026-08-05T02:00:00.000Z"
                ],
            )
            .unwrap();

        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: Some(20),
            active_window_days: Some(7),
            hide_archived: Some(true),
            include_copilot: Some(true),
            include_copilot_cli: Some(true),
            include_claude: Some(false),
            include_codex: Some(false),
            include_open_code: Some(false),
            workspace_path_blacklist: None,
        }));
        let sessions = scan_copilot_cli_sessions_from_connection(
            &connection,
            Path::new("session-store.db"),
            iso_timestamp_ms("2026-08-05T02:05:00.000Z").unwrap(),
            &options,
        );

        assert_eq!(sessions.len(), 1);
        let session = &sessions[0];
        assert_eq!(session.id, "copilot-cli:cli-session-1");
        assert_eq!(session.provider, "copilot-cli");
        assert_eq!(session.provider_label, "GH›");
        assert_eq!(session.workspace, "AgentWatcher");
        assert_eq!(session.message_count, 1);
        assert_eq!(
            session.last_user_message.as_deref(),
            Some("Please inspect the CLI session store")
        );
        assert_eq!(
            session.last_ai_message.as_deref(),
            Some("I found the missing session source.")
        );
    }

    #[test]
    fn codex_turn_summary_cache_is_keyed_by_thread_updated_ms() {
        let thread_id = format!("cache-test-{}", current_time_ms());
        cache_codex_turn_summary(
            &thread_id,
            CodexTurnSummary {
                thread_updated_ms: 100,
                fetched_ms: 200,
                last_user_message: Some("cached user".to_string()),
                last_ai_message: Some("cached ai".to_string()),
                message_count: 2,
                latest_turn_status: Some("completed".to_string()),
            },
        );

        assert!(cached_codex_turn_summary(&thread_id, 101).is_none());
        assert_eq!(
            cached_codex_turn_summary(&thread_id, 100)
                .and_then(|summary| summary.last_user_message)
                .as_deref(),
            Some("cached user")
        );
    }

    #[test]
    fn codex_scan_turn_fetch_budget_stays_small() {
        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: Some(80),
            active_window_days: Some(7),
            hide_archived: Some(true),
            include_copilot: Some(false),
            include_copilot_cli: Some(false),
            include_claude: Some(false),
            include_codex: Some(true),
            include_open_code: Some(false),
            workspace_path_blacklist: None,
        }));

        assert_eq!(
            codex_scan_turn_fetch_budget(&options),
            CODEX_SCAN_TURN_FETCH_BUDGET
        );
        assert!(codex_thread_list_limit(&options) <= 120);
    }

    #[test]
    fn codex_live_handoff_and_todo_tracking_when_enabled() {
        if env::var("AGENTWATCHER_CODEX_LIVE_TEST").ok().as_deref() != Some("1") {
            return;
        }

        let mut workspace_dir = env::current_dir().unwrap();
        if workspace_dir
            .file_name()
            .and_then(|file_name| file_name.to_str())
            .map(|file_name| file_name.eq_ignore_ascii_case("src-tauri"))
            .unwrap_or(false)
        {
            if let Some(parent_dir) = workspace_dir.parent() {
                workspace_dir = parent_dir.to_path_buf();
            }
        }
        let workspace_path = path_to_cli_string(&workspace_dir);
        let token = format!("AGENTWATCHER-CODEX-LIVE-{:x}", current_time_ms());
        let todo_id = format!("todo-awlive-{:x}", current_time_ms());
        let prompt = format!(
            "AgentWatcher live Codex verification.\nAgentWatcher Todo ID: {}\nToken: {}\nPlease reply only: received {}.",
            todo_id, token, token
        );

        let thread_id = launch_codex_handoff(&workspace_path, &prompt).unwrap();
        let options = resolve_scan_options(Some(ScanOptions {
            max_sessions: Some(80),
            active_window_days: Some(7),
            hide_archived: Some(true),
            include_copilot: Some(false),
            include_copilot_cli: Some(false),
            include_claude: Some(false),
            include_codex: Some(true),
            include_open_code: Some(false),
            workspace_path_blacklist: None,
        }));

        let session = wait_for_live_codex_session(&thread_id, &token, &options)
            .unwrap_or_else(|| panic!("Codex live session was not visible: {}", thread_id));

        assert_eq!(session.provider, "codex");
        assert_eq!(session.provider_label, "CX");
        assert_eq!(
            session.workspace_path.as_deref(),
            Some(workspace_path.as_str())
        );
        let expected_resource = codex_thread_resource(&thread_id);
        assert_eq!(
            session.session_resource.as_deref(),
            Some(expected_resource.as_str())
        );
        assert!(session
            .last_user_message
            .as_deref()
            .unwrap_or("")
            .contains(&token));
        assert!(session.todo_ids.contains(&todo_id));
        assert!(matches!(
            session.status.as_str(),
            "running" | "waiting" | "idle"
        ));
        shutdown_codex_app_server();
    }

    fn wait_for_live_codex_session(
        thread_id: &str,
        token: &str,
        options: &ResolvedScanOptions,
    ) -> Option<AgentSession> {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if let Some(session) = read_live_codex_session(thread_id, options) {
                if session
                    .last_user_message
                    .as_deref()
                    .map(|message| message.contains(token))
                    .unwrap_or(false)
                {
                    return Some(session);
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        None
    }

    fn read_live_codex_session(
        thread_id: &str,
        options: &ResolvedScanOptions,
    ) -> Option<AgentSession> {
        with_codex_app_server_client(|client| {
            let response = client.request(
                "thread/list",
                json!({
                    "limit": 100,
                    "sortKey": "updated_at",
                    "sortDirection": "desc",
                    "archived": false
                }),
            )?;
            let Some(thread_json) =
                response
                    .get("data")
                    .and_then(Value::as_array)
                    .and_then(|threads| {
                        threads
                            .iter()
                            .find(|thread| {
                                string_at(thread, &["/id", "/sessionId"])
                                    .map(|candidate| candidate == thread_id)
                                    .unwrap_or(false)
                            })
                            .cloned()
                    })
            else {
                return Ok(None);
            };

            let enriched_thread = codex_thread_with_turns(client, &thread_json);
            Ok(read_codex_thread(
                &enriched_thread,
                current_time_ms(),
                options,
            ))
        })
        .ok()
        .flatten()
    }

    #[test]
    fn performance_snapshot_samples_current_process_tree() {
        let first_snapshot = get_performance_snapshot(None);
        std::thread::sleep(Duration::from_millis(30));
        let second_snapshot = get_performance_snapshot(None);

        assert!(second_snapshot.captured_ms >= first_snapshot.captured_ms);
        #[cfg(target_os = "windows")]
        {
            assert!(second_snapshot.process.process_count >= 1);
            assert!(second_snapshot
                .processes
                .iter()
                .any(|process| process.pid > 0 && !process.name.is_empty()));
        }
    }

    #[test]
    fn performance_snapshot_can_enable_agent_client_scope() {
        let snapshot = get_performance_snapshot(Some(PerformanceSnapshotOptions {
            include_agent_clients: Some(true),
        }));

        assert!(snapshot.gpu_note.contains("Advanced mode"));
        #[cfg(target_os = "windows")]
        assert!(snapshot
            .processes
            .iter()
            .any(|process| process.role == "AgentWatcher" && process.root_pid > 0));
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
