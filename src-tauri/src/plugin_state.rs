use crate::plugin_manifest::validate_agentwatcher_name;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const STORE_VERSION: u32 = 1;
const MAX_AUDIT_ENTRIES: usize = 400;
const STATE_AUDIT_TAIL: usize = 40;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AwPluginRuntimeState {
    pub plugin_name: String,
    pub enabled: bool,
    pub updated_ms: Option<u64>,
    #[serde(default)]
    pub audit: Vec<AwPluginAuditEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AwPluginAuditEntry {
    pub id: String,
    pub plugin_name: String,
    pub action: String,
    pub enabled: bool,
    pub changed: bool,
    pub at_ms: u64,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AwPluginStateStore {
    version: u32,
    #[serde(default)]
    plugins: BTreeMap<String, AwPluginStateEntry>,
    #[serde(default)]
    audit: Vec<AwPluginAuditEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AwPluginStateEntry {
    enabled: bool,
    updated_ms: u64,
}

pub fn plugin_state_path_from_appdata(appdata: &Path) -> PathBuf {
    appdata.join("AgentWatcher").join("plugins.v1.json")
}

pub fn get_plugin_runtime_state(
    path: &Path,
    plugin_name: &str,
) -> Result<AwPluginRuntimeState, String> {
    validate_plugin_name(plugin_name)?;
    let store = read_store(path)?;
    Ok(runtime_state_from_store(&store, plugin_name))
}

pub fn set_plugin_enabled(
    path: &Path,
    plugin_name: &str,
    enabled: bool,
    reason: Option<String>,
) -> Result<AwPluginRuntimeState, String> {
    validate_plugin_name(plugin_name)?;
    let mut store = read_store(path)?;
    let at_ms = now_ms();
    let previous = store
        .plugins
        .get(plugin_name)
        .map(|entry| entry.enabled)
        .unwrap_or(true);
    let changed = previous != enabled;

    store.plugins.insert(
        plugin_name.to_string(),
        AwPluginStateEntry {
            enabled,
            updated_ms: at_ms,
        },
    );
    store.audit.push(AwPluginAuditEntry {
        id: format!("{plugin_name}-{at_ms}-{}", store.audit.len() + 1),
        plugin_name: plugin_name.to_string(),
        action: if enabled {
            "enable".to_string()
        } else {
            "disable".to_string()
        },
        enabled,
        changed,
        at_ms,
        reason: clean_reason(reason),
    });
    if store.audit.len() > MAX_AUDIT_ENTRIES {
        let drop_count = store.audit.len() - MAX_AUDIT_ENTRIES;
        store.audit.drain(0..drop_count);
    }
    write_store(path, &store)?;
    Ok(runtime_state_from_store(&store, plugin_name))
}

pub fn record_plugin_audit(
    path: &Path,
    plugin_name: &str,
    action: &str,
    reason: Option<String>,
) -> Result<AwPluginRuntimeState, String> {
    validate_plugin_name(plugin_name)?;
    let mut store = read_store(path)?;
    let at_ms = now_ms();
    let enabled = store
        .plugins
        .get(plugin_name)
        .map(|entry| entry.enabled)
        .unwrap_or(true);
    store.audit.push(AwPluginAuditEntry {
        id: format!("{plugin_name}-{at_ms}-{}", store.audit.len() + 1),
        plugin_name: plugin_name.to_string(),
        action: action.chars().take(48).collect(),
        enabled,
        changed: false,
        at_ms,
        reason: clean_reason(reason),
    });
    if store.audit.len() > MAX_AUDIT_ENTRIES {
        let drop_count = store.audit.len() - MAX_AUDIT_ENTRIES;
        store.audit.drain(0..drop_count);
    }
    write_store(path, &store)?;
    Ok(runtime_state_from_store(&store, plugin_name))
}

fn runtime_state_from_store(store: &AwPluginStateStore, plugin_name: &str) -> AwPluginRuntimeState {
    let entry = store.plugins.get(plugin_name);
    let mut audit = store
        .audit
        .iter()
        .filter(|item| item.plugin_name == plugin_name)
        .cloned()
        .collect::<Vec<_>>();
    if audit.len() > STATE_AUDIT_TAIL {
        audit = audit[audit.len() - STATE_AUDIT_TAIL..].to_vec();
    }

    AwPluginRuntimeState {
        plugin_name: plugin_name.to_string(),
        enabled: entry.map(|item| item.enabled).unwrap_or(true),
        updated_ms: entry.map(|item| item.updated_ms),
        audit,
    }
}

fn validate_plugin_name(plugin_name: &str) -> Result<(), String> {
    validate_agentwatcher_name("pluginName", plugin_name).map_err(|error| error.to_string())
}

fn read_store(path: &Path) -> Result<AwPluginStateStore, String> {
    if !path.exists() {
        return Ok(default_store());
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read plugin state: {error}"))?;
    if text.trim().is_empty() {
        return Ok(default_store());
    }
    let mut store = serde_json::from_str::<AwPluginStateStore>(&text)
        .map_err(|error| format!("Invalid plugin state JSON: {error}"))?;
    if store.version == 0 {
        store.version = STORE_VERSION;
    }
    Ok(store)
}

fn write_store(path: &Path, store: &AwPluginStateStore) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Plugin state path has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Failed to prepare plugin state directory: {error}"))?;
    let text = serde_json::to_string_pretty(store)
        .map_err(|error| format!("Failed to serialize plugin state: {error}"))?;
    fs::write(path, text).map_err(|error| format!("Failed to write plugin state: {error}"))
}

fn default_store() -> AwPluginStateStore {
    AwPluginStateStore {
        version: STORE_VERSION,
        ..Default::default()
    }
}

fn clean_reason(reason: Option<String>) -> Option<String> {
    reason
        .map(|value| value.trim().chars().take(240).collect::<String>())
        .filter(|value| !value.is_empty())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn plugin_state_path_lives_under_agentwatcher_appdata() {
        let path = plugin_state_path_from_appdata(Path::new(r"C:\Users\me\AppData\Roaming"));
        assert_eq!(
            path,
            Path::new(r"C:\Users\me\AppData\Roaming")
                .join("AgentWatcher")
                .join("plugins.v1.json")
        );
    }

    #[test]
    fn missing_plugin_state_defaults_to_enabled() {
        let path = temp_state_path("missing");
        let state = get_plugin_runtime_state(&path, "UEWorkflow").unwrap();

        assert_eq!(state.plugin_name, "UEWorkflow");
        assert!(state.enabled);
        assert!(state.updated_ms.is_none());
        assert!(state.audit.is_empty());
    }

    #[test]
    fn set_plugin_enabled_persists_state_and_audit() {
        let path = temp_state_path("persist");
        let disabled = set_plugin_enabled(
            &path,
            "UEWorkflow",
            false,
            Some("用户在设置中停用".to_string()),
        )
        .unwrap();
        let loaded = get_plugin_runtime_state(&path, "UEWorkflow").unwrap();

        assert!(!disabled.enabled);
        assert!(!loaded.enabled);
        assert_eq!(loaded.audit.len(), 1);
        assert_eq!(loaded.audit[0].action, "disable");
        assert_eq!(loaded.audit[0].reason.as_deref(), Some("用户在设置中停用"));

        let _ = fs::remove_file(path);
    }

    fn temp_state_path(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "agentwatcher-plugin-state-{label}-{}.json",
            now_ms()
        ))
    }
}
