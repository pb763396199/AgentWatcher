//! Generic, provider-neutral persistent workflow runtime.
pub mod backup;
pub mod codex;
pub mod git;
pub mod legacy;
pub mod service;
pub mod store;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

pub const REFLECTION_PROMPT: &str = "反思一下，方向对了还是错了，继续深挖还是换个思路";
pub const PROTOCOL: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub schema_version: u32,
    pub request_id: String,
    pub command: String,
    #[serde(default)]
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub code: String,
    pub message: String,
}

impl Error {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl From<rusqlite::Error> for Error {
    fn from(value: rusqlite::Error) -> Self {
        Self::new("storage_error", value.to_string())
    }
}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::new("io_error", value.to_string())
    }
}
pub type Result<T> = std::result::Result<T, Error>;
pub fn id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4())
}
pub fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn default_home() -> PathBuf {
    if let Some(path) = std::env::var_os("AGENTWATCHER_WORKFLOW_HOME") {
        return PathBuf::from(path);
    }
    #[cfg(windows)]
    if let Some(path) = std::env::var_os("LOCALAPPDATA") {
        return PathBuf::from(path).join("AgentWatcher").join("workflow");
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(target_os = "macos")]
    {
        return home.join("Library/Application Support/AgentWatcher/workflow");
    }
    #[cfg(not(target_os = "macos"))]
    home.join(".local/share/AgentWatcher/workflow")
}

/// Read-only coexistence check. Never starts the service or creates its database.
pub fn active_session_ids() -> std::collections::HashSet<String> {
    let path = default_home().join("workflow.sqlite3");
    let Ok(connection) =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
    else {
        return Default::default();
    };
    let Ok(mut statement)=connection.prepare("SELECT session_id FROM executions WHERE state IN ('working','needs_human','pause_requested','paused','submitted')") else{return Default::default()};
    let Ok(rows) = statement.query_map([], |row| row.get::<_, String>(0)) else {
        return Default::default();
    };
    rows.filter_map(std::result::Result::ok).collect()
}

pub fn invoke_plugin(name: &str, command: &str, request: Value) -> Result<Value> {
    crate::ensure_plugin_enabled(name).map_err(|e| Error::new("plugin_disabled", e))?;
    let catalog = crate::scan_plugin_catalog().map_err(|e| Error::new("plugin_unavailable", e))?;
    let plugin = crate::plugin_catalog::find_plugin(&catalog, name)
        .map_err(|e| Error::new("plugin_unavailable", e))?;
    let response = crate::plugin_catalog::invoke_plugin(
        plugin,
        &crate::plugin_catalog::AwPluginInvokeRequest {
            plugin_name: name.into(),
            command: command.into(),
            request,
        },
    )
    .map_err(|e| Error::new("plugin_command_failed", e))?;
    serde_json::to_value(response).map_err(|e| Error::new("plugin_response_invalid", e.to_string()))
}

pub fn required<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| {
            Error::new(
                "invalid_request",
                format!("{key} must be a nonempty string"),
            )
        })
}
pub fn human(value: &Value) -> Result<()> {
    if value.get("humanConfirmed").and_then(Value::as_bool) != Some(true) {
        return Err(Error::new(
            "human_confirmation_required",
            "Explicit human confirmation is required",
        ));
    }
    Ok(())
}
