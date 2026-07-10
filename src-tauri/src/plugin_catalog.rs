use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::Write;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

const DESCRIPTOR_EXTENSION: &str = "awplugin";
const MOUNT_STORE_VERSION: u32 = 1;
const MAX_SCAN_DEPTH: usize = 8;
const MAX_PLUGIN_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct AwPluginDescriptor {
    pub file_version: u32,
    pub version: u32,
    pub version_name: String,
    pub name: String,
    pub friendly_name: String,
    pub description: String,
    pub category: String,
    #[serde(default)]
    pub created_by: String,
    #[serde(default)]
    pub enabled_by_default: bool,
    #[serde(default)]
    pub installed: bool,
    #[serde(default)]
    pub can_contain_content: bool,
    pub modules: Vec<AwPluginModule>,
    pub runtime: AwPluginRuntime,
    pub commands: Vec<AwPluginCommand>,
    #[serde(default)]
    pub contributions: AwPluginContributions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct AwPluginModule {
    pub name: String,
    pub friendly_name: String,
    pub r#type: String,
    pub loading_phase: String,
    pub package: String,
    pub path: String,
    #[serde(default)]
    pub user_facing: bool,
    #[serde(default)]
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct AwPluginRuntime {
    pub protocol: String,
    pub executable_candidates: Vec<String>,
    pub invoke_arguments: Vec<String>,
    pub handshake_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct AwPluginCommand {
    pub name: String,
    #[serde(default)]
    pub friendly_name: Option<String>,
    #[serde(default)]
    pub placement: Option<String>,
    pub safety: String,
    #[serde(default)]
    pub requires_confirmation: bool,
    #[serde(default)]
    pub confirmation_boundary: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct AwPluginContributions {
    #[serde(default)]
    pub workflows: Vec<AwWorkflowContribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct AwWorkflowContribution {
    pub name: String,
    pub friendly_name: String,
    pub description: String,
    #[serde(default)]
    pub legacy_ids: Vec<String>,
    pub task_schema: String,
    pub plan_command: String,
    pub execute_command: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AwDiscoveredPlugin {
    pub descriptor: AwPluginDescriptor,
    pub workflows: Vec<AwResolvedWorkflow>,
    pub descriptor_path: String,
    pub root_path: String,
    pub mounted: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AwResolvedWorkflow {
    pub name: String,
    pub friendly_name: String,
    pub description: String,
    pub legacy_ids: Vec<String>,
    pub task_schema_path: String,
    pub task_schema: Value,
    pub plan_command: String,
    pub execute_command: String,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AwPluginCatalogSnapshot {
    pub plugins: Vec<AwDiscoveredPlugin>,
    pub errors: Vec<String>,
    pub mount_paths: Vec<String>,
    pub installed_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AwPluginMountStore {
    version: u32,
    #[serde(default)]
    mounts: Vec<String>,
}

impl Default for AwPluginMountStore {
    fn default() -> Self {
        Self {
            version: MOUNT_STORE_VERSION,
            mounts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwPluginInvokeRequest {
    pub plugin_name: String,
    pub command: String,
    #[serde(default)]
    pub request: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwPluginInvokeResponse {
    pub plugin_name: String,
    pub command: String,
    pub executable: String,
    pub result: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<String>,
}

pub fn mount_store_path_from_appdata(appdata: &Path) -> PathBuf {
    appdata.join("AgentWatcher").join("plugin-mounts.v1.json")
}

pub fn installed_plugin_root_from_localappdata(localappdata: &Path) -> PathBuf {
    localappdata.join("AgentWatcher").join("Plugins")
}

pub fn scan_catalog(mount_store_path: &Path, installed_root: &Path) -> AwPluginCatalogSnapshot {
    let store = load_mount_store(mount_store_path).unwrap_or_default();
    let mut snapshot = AwPluginCatalogSnapshot {
        plugins: Vec::new(),
        errors: Vec::new(),
        mount_paths: store.mounts.clone(),
        installed_root: path_text(installed_root),
    };
    let mut by_name = BTreeMap::<String, AwDiscoveredPlugin>::new();
    let mut duplicate_names = HashSet::<String>::new();

    if installed_root.is_dir() {
        scan_tree(
            installed_root,
            false,
            0,
            &mut by_name,
            &mut duplicate_names,
            &mut snapshot.errors,
        );
    }
    for mount in &store.mounts {
        let path = PathBuf::from(mount);
        if !path.is_dir() {
            snapshot
                .errors
                .push(format!("mounted plugin directory does not exist: {mount}"));
            continue;
        }
        scan_tree(
            &path,
            true,
            0,
            &mut by_name,
            &mut duplicate_names,
            &mut snapshot.errors,
        );
    }

    snapshot.plugins = by_name.into_values().collect();
    snapshot
}

pub fn mount_plugin(
    mount_store_path: &Path,
    plugin_dir: &Path,
) -> Result<AwDiscoveredPlugin, String> {
    let canonical = plugin_dir
        .canonicalize()
        .map_err(|error| format!("failed to resolve plugin directory: {error}"))?;
    let plugin = read_plugin_at_root(&canonical, true)?;
    let mut store = load_mount_store(mount_store_path)?;
    let canonical_text = path_text(&canonical);
    if !store
        .mounts
        .iter()
        .any(|path| path.eq_ignore_ascii_case(&canonical_text))
    {
        store.mounts.push(canonical_text);
        store.mounts.sort_by_key(|path| path.to_ascii_lowercase());
        save_mount_store(mount_store_path, &store)?;
    }
    Ok(plugin)
}

pub fn unmount_plugin(mount_store_path: &Path, plugin_name: &str) -> Result<bool, String> {
    validate_identifier("plugin name", plugin_name)?;
    let mut store = load_mount_store(mount_store_path)?;
    let before = store.mounts.len();
    store.mounts.retain(|mount| {
        read_plugin_at_root(Path::new(mount), true)
            .map(|plugin| plugin.descriptor.name != plugin_name)
            .unwrap_or(true)
    });
    let changed = store.mounts.len() != before;
    if changed {
        save_mount_store(mount_store_path, &store)?;
    }
    Ok(changed)
}

pub fn invoke_plugin(
    plugin: &AwDiscoveredPlugin,
    request: &AwPluginInvokeRequest,
) -> Result<AwPluginInvokeResponse, String> {
    if plugin.descriptor.name != request.plugin_name {
        return Err("plugin invocation identity mismatch".to_string());
    }
    if !plugin
        .descriptor
        .commands
        .iter()
        .any(|command| command.name == request.command)
    {
        return Err(format!(
            "plugin command is not declared by {}: {}",
            plugin.descriptor.name, request.command
        ));
    }

    let root = Path::new(&plugin.root_path);
    let executable = resolve_executable(root, &plugin.descriptor.runtime)?;
    let envelope = serde_json::to_vec(&serde_json::json!({
        "schemaVersion": 1,
        "command": request.command,
        "request": request.request,
    }))
    .map_err(|error| format!("failed to serialize plugin request: {error}"))?;

    let mut command = Command::new(&executable);
    command
        .args(&plugin.descriptor.runtime.invoke_arguments)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);

    let mut child = command
        .spawn()
        .map_err(|error| format!("failed to start plugin process: {error}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| "plugin stdin is not available".to_string())?
        .write_all(&envelope)
        .map_err(|error| format!("failed to write plugin request: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("failed to wait for plugin process: {error}"))?;

    if output.stdout.len() > MAX_PLUGIN_OUTPUT_BYTES
        || output.stderr.len() > MAX_PLUGIN_OUTPUT_BYTES
    {
        return Err("plugin output exceeded the 16 MiB contract limit".to_string());
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("plugin stdout is not UTF-8: {error}"))?;
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(format!(
            "plugin command failed with {}: {}",
            output.status,
            if stderr.is_empty() {
                stdout.trim()
            } else {
                &stderr
            }
        ));
    }
    let result = serde_json::from_str(stdout.trim())
        .map_err(|error| format!("plugin stdout is not valid JSON: {error}"))?;
    Ok(AwPluginInvokeResponse {
        plugin_name: plugin.descriptor.name.clone(),
        command: request.command.clone(),
        executable: path_text(&executable),
        result,
        diagnostics: (!stderr.is_empty()).then_some(stderr),
    })
}

pub fn find_plugin<'a>(
    snapshot: &'a AwPluginCatalogSnapshot,
    plugin_name: &str,
) -> Result<&'a AwDiscoveredPlugin, String> {
    validate_identifier("plugin name", plugin_name)?;
    snapshot
        .plugins
        .iter()
        .find(|plugin| plugin.descriptor.name == plugin_name)
        .ok_or_else(|| format!("plugin is not mounted: {plugin_name}"))
}

fn scan_tree(
    directory: &Path,
    mounted: bool,
    depth: usize,
    plugins: &mut BTreeMap<String, AwDiscoveredPlugin>,
    duplicate_names: &mut HashSet<String>,
    errors: &mut Vec<String>,
) {
    if depth > MAX_SCAN_DEPTH {
        errors.push(format!(
            "plugin scan depth exceeded at {}",
            path_text(directory)
        ));
        return;
    }
    match descriptor_paths(directory) {
        Ok(paths) if !paths.is_empty() => {
            if paths.len() != 1 {
                errors.push(format!(
                    "plugin directory must contain exactly one *.awplugin descriptor: {}",
                    path_text(directory)
                ));
                return;
            }
            match read_plugin_descriptor(directory, &paths[0], mounted) {
                Ok(plugin) => {
                    let plugin_name = plugin.descriptor.name.clone();
                    if duplicate_names.contains(&plugin_name) {
                        errors.push(format!(
                            "duplicate plugin {plugin_name}: {}",
                            plugin.root_path
                        ));
                    } else if let Some(existing) = plugins.remove(&plugin_name) {
                        errors.push(format!(
                            "duplicate plugin {}: {} and {}",
                            plugin_name, existing.root_path, plugin.root_path
                        ));
                        duplicate_names.insert(plugin_name);
                    } else {
                        plugins.insert(plugin_name, plugin);
                    }
                }
                Err(error) => errors.push(error),
            }
            return;
        }
        Ok(_) => {}
        Err(error) => {
            errors.push(error);
            return;
        }
    }

    let Ok(entries) = fs::read_dir(directory) else {
        errors.push(format!(
            "failed to read plugin directory: {}",
            path_text(directory)
        ));
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || ignored_directory(&path) {
            continue;
        }
        scan_tree(&path, mounted, depth + 1, plugins, duplicate_names, errors);
    }
}

fn read_plugin_at_root(plugin_dir: &Path, mounted: bool) -> Result<AwDiscoveredPlugin, String> {
    let descriptors = descriptor_paths(plugin_dir)?;
    if descriptors.len() != 1 {
        return Err(format!(
            "plugin mount must point at a directory containing exactly one *.awplugin: {}",
            path_text(plugin_dir)
        ));
    }
    read_plugin_descriptor(plugin_dir, &descriptors[0], mounted)
}

fn descriptor_paths(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("failed to read {}: {error}", path_text(directory)))?;
    let mut paths = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value.eq_ignore_ascii_case(DESCRIPTOR_EXTENSION))
        })
        .collect::<Vec<_>>();
    paths.sort();
    Ok(paths)
}

fn read_plugin_descriptor(
    plugin_dir: &Path,
    descriptor_path: &Path,
    mounted: bool,
) -> Result<AwDiscoveredPlugin, String> {
    let text = fs::read_to_string(descriptor_path)
        .map_err(|error| format!("failed to read {}: {error}", path_text(descriptor_path)))?;
    let descriptor: AwPluginDescriptor = serde_json::from_str(&text)
        .map_err(|error| format!("invalid {}: {error}", path_text(descriptor_path)))?;
    validate_descriptor(plugin_dir, descriptor_path, &descriptor)?;
    let workflows = resolve_workflows(plugin_dir, &descriptor)?;
    Ok(AwDiscoveredPlugin {
        descriptor,
        workflows,
        descriptor_path: path_text(descriptor_path),
        root_path: path_text(plugin_dir),
        mounted,
        enabled: false,
    })
}

fn resolve_workflows(
    plugin_dir: &Path,
    descriptor: &AwPluginDescriptor,
) -> Result<Vec<AwResolvedWorkflow>, String> {
    let canonical_root = plugin_dir
        .canonicalize()
        .map_err(|error| format!("failed to resolve plugin root: {error}"))?;
    descriptor
        .contributions
        .workflows
        .iter()
        .map(|workflow| {
            let schema_path = plugin_dir.join(&workflow.task_schema);
            let canonical_schema = schema_path.canonicalize().map_err(|error| {
                format!(
                    "workflow {} task schema is not readable: {error}",
                    workflow.name
                )
            })?;
            if !canonical_schema.starts_with(&canonical_root) || !canonical_schema.is_file() {
                return Err(format!(
                    "workflow {} task schema escapes the plugin root",
                    workflow.name
                ));
            }
            let schema_text = fs::read_to_string(&canonical_schema).map_err(|error| {
                format!(
                    "failed to read workflow {} task schema: {error}",
                    workflow.name
                )
            })?;
            let task_schema: Value = serde_json::from_str(&schema_text).map_err(|error| {
                format!(
                    "workflow {} task schema is not valid JSON: {error}",
                    workflow.name
                )
            })?;
            if !task_schema.is_object() {
                return Err(format!(
                    "workflow {} task schema must be a JSON object",
                    workflow.name
                ));
            }
            Ok(AwResolvedWorkflow {
                name: workflow.name.clone(),
                friendly_name: workflow.friendly_name.clone(),
                description: workflow.description.clone(),
                legacy_ids: workflow.legacy_ids.clone(),
                task_schema_path: path_text(&canonical_schema),
                task_schema,
                plan_command: workflow.plan_command.clone(),
                execute_command: workflow.execute_command.clone(),
            })
        })
        .collect()
}

fn validate_descriptor(
    plugin_dir: &Path,
    descriptor_path: &Path,
    descriptor: &AwPluginDescriptor,
) -> Result<(), String> {
    if descriptor.file_version != 1 {
        return Err(format!(
            "unsupported plugin FileVersion: {}",
            descriptor.file_version
        ));
    }
    validate_identifier("plugin Name", &descriptor.name)?;
    let expected_file = format!("{}.awplugin", descriptor.name);
    let actual_file = descriptor_path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !actual_file.eq_ignore_ascii_case(&expected_file) {
        return Err(format!(
            "plugin descriptor filename must match Name: expected {expected_file}, got {actual_file}"
        ));
    }
    if plugin_dir.file_name().and_then(|value| value.to_str()) != Some(descriptor.name.as_str()) {
        return Err(format!(
            "plugin directory must match Name: expected {}, got {}",
            descriptor.name,
            plugin_dir
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
        ));
    }
    if descriptor.runtime.protocol != "AgentWatcherPluginStdio/1" {
        return Err(format!(
            "unsupported plugin protocol: {}",
            descriptor.runtime.protocol
        ));
    }
    if descriptor.runtime.executable_candidates.is_empty() {
        return Err("plugin Runtime.ExecutableCandidates must not be empty".to_string());
    }
    for relative in &descriptor.runtime.executable_candidates {
        validate_relative_path("Runtime.ExecutableCandidates", relative)?;
    }

    let mut module_names = HashSet::new();
    for module in &descriptor.modules {
        validate_identifier("module Name", &module.name)?;
        if !module_names.insert(module.name.as_str()) {
            return Err(format!("duplicate plugin module: {}", module.name));
        }
        validate_relative_path("module Path", &module.path)?;
    }
    for module in &descriptor.modules {
        for dependency in &module.dependencies {
            if !module_names.contains(dependency.as_str()) {
                return Err(format!(
                    "module {} depends on unknown module {dependency}",
                    module.name
                ));
            }
        }
    }

    let mut command_names = HashSet::new();
    for command in &descriptor.commands {
        validate_command_name(&command.name)?;
        if !command_names.insert(command.name.as_str()) {
            return Err(format!("duplicate plugin command: {}", command.name));
        }
        if command.requires_confirmation && command.confirmation_boundary.is_none() {
            return Err(format!(
                "plugin command {} requires ConfirmationBoundary",
                command.name
            ));
        }
    }
    if !command_names.contains(descriptor.runtime.handshake_command.as_str()) {
        return Err("Runtime.HandshakeCommand is not declared in Commands".to_string());
    }
    for workflow in &descriptor.contributions.workflows {
        validate_identifier("workflow Name", &workflow.name)?;
        for legacy_id in &workflow.legacy_ids {
            validate_command_name(legacy_id)?;
        }
        validate_relative_path("workflow TaskSchema", &workflow.task_schema)?;
        if !command_names.contains(workflow.plan_command.as_str())
            || !command_names.contains(workflow.execute_command.as_str())
        {
            return Err(format!(
                "workflow {} references undeclared commands",
                workflow.name
            ));
        }
    }
    Ok(())
}

fn resolve_executable(root: &Path, runtime: &AwPluginRuntime) -> Result<PathBuf, String> {
    let canonical_root = root
        .canonicalize()
        .map_err(|error| format!("failed to resolve plugin root: {error}"))?;
    for candidate in &runtime.executable_candidates {
        let path = root.join(candidate);
        if !path.is_file() {
            continue;
        }
        let canonical = path
            .canonicalize()
            .map_err(|error| format!("failed to resolve plugin executable: {error}"))?;
        if !canonical.starts_with(&canonical_root) {
            return Err("plugin executable escapes plugin root".to_string());
        }
        return Ok(canonical);
    }
    Err(format!(
        "plugin executable not found under {}",
        path_text(root)
    ))
}

fn load_mount_store(path: &Path) -> Result<AwPluginMountStore, String> {
    if !path.exists() {
        return Ok(AwPluginMountStore::default());
    }
    let text = fs::read_to_string(path)
        .map_err(|error| format!("failed to read plugin mount store: {error}"))?;
    let store: AwPluginMountStore = serde_json::from_str(&text)
        .map_err(|error| format!("invalid plugin mount store: {error}"))?;
    if store.version != MOUNT_STORE_VERSION {
        return Err(format!(
            "unsupported plugin mount store version: {}",
            store.version
        ));
    }
    Ok(store)
}

fn save_mount_store(path: &Path, store: &AwPluginMountStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create plugin state directory: {error}"))?;
    }
    let text = serde_json::to_string_pretty(store)
        .map_err(|error| format!("failed to serialize plugin mount store: {error}"))?;
    fs::write(path, format!("{text}\n"))
        .map_err(|error| format!("failed to write plugin mount store: {error}"))
}

fn validate_identifier(label: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '_')
    {
        return Err(format!(
            "{label} must use ASCII letters, digits, or underscore"
        ));
    }
    Ok(())
}

fn validate_command_name(value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
    {
        return Err("plugin command name contains unsupported characters".to_string());
    }
    Ok(())
}

fn validate_relative_path(label: &str, value: &str) -> Result<(), String> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!("{label} must stay inside the plugin root: {value}"));
    }
    Ok(())
}

fn ignored_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                ".git" | "target" | "node_modules" | "binaries" | "intermediate" | "saved"
            )
        })
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn zero_plugin_catalog_is_valid() {
        let root = temp_dir("zero");
        let snapshot = scan_catalog(&root.join("mounts.json"), &root.join("Plugins"));
        assert!(snapshot.plugins.is_empty());
        assert!(snapshot.errors.is_empty());
    }

    #[test]
    fn mounted_plugin_is_discovered_but_not_implicitly_enabled() {
        let root = temp_dir("mounted");
        let plugin = root.join("SamplePlugin");
        fs::create_dir_all(&plugin).unwrap();
        fs::write(
            plugin.join("SamplePlugin.awplugin"),
            descriptor_json("SamplePlugin"),
        )
        .unwrap();
        let store_path = root.join("mounts.json");
        let mounted = mount_plugin(&store_path, &plugin).unwrap();
        assert!(mounted.mounted);
        assert!(!mounted.descriptor.enabled_by_default);

        let snapshot = scan_catalog(&store_path, &root.join("Plugins"));
        assert_eq!(snapshot.plugins.len(), 1);
        assert_eq!(snapshot.plugins[0].descriptor.name, "SamplePlugin");
    }

    #[test]
    fn descriptor_filename_must_match_plugin_name() {
        let root = temp_dir("name-mismatch");
        let plugin = root.join("SamplePlugin");
        fs::create_dir_all(&plugin).unwrap();
        fs::write(
            plugin.join("Wrong.awplugin"),
            descriptor_json("SamplePlugin"),
        )
        .unwrap();
        let error = mount_plugin(&root.join("mounts.json"), &plugin).unwrap_err();
        assert!(error.contains("filename must match Name"));
    }

    #[test]
    fn scanner_stops_below_a_discovered_plugin() {
        let root = temp_dir("scan-boundary");
        let plugin = root.join("ParentPlugin");
        let nested = plugin.join("NestedPlugin");
        fs::create_dir_all(&nested).unwrap();
        fs::write(
            plugin.join("ParentPlugin.awplugin"),
            descriptor_json("ParentPlugin"),
        )
        .unwrap();
        fs::write(
            nested.join("NestedPlugin.awplugin"),
            descriptor_json("NestedPlugin"),
        )
        .unwrap();

        let snapshot = scan_catalog(&root.join("mounts.json"), &root);
        assert_eq!(snapshot.plugins.len(), 1);
        assert_eq!(snapshot.plugins[0].descriptor.name, "ParentPlugin");
    }

    #[test]
    fn duplicate_plugin_names_reject_every_candidate() {
        let root = temp_dir("duplicate-name");
        let installed_root = root.join("Installed");
        let installed = installed_root.join("SamplePlugin");
        let mounted = root.join("Development").join("SamplePlugin");
        fs::create_dir_all(&installed).unwrap();
        fs::create_dir_all(&mounted).unwrap();
        for plugin in [&installed, &mounted] {
            fs::write(
                plugin.join("SamplePlugin.awplugin"),
                descriptor_json("SamplePlugin"),
            )
            .unwrap();
        }
        let mount_store = root.join("mounts.json");
        mount_plugin(&mount_store, &mounted).unwrap();

        let snapshot = scan_catalog(&mount_store, &installed_root);

        assert!(snapshot.plugins.is_empty());
        assert!(snapshot
            .errors
            .iter()
            .any(|error| error.contains("duplicate plugin SamplePlugin")));
    }

    #[test]
    fn workflow_schema_is_resolved_from_the_plugin_root() {
        let root = temp_dir("workflow-schema");
        let plugin = root.join("SamplePlugin");
        fs::create_dir_all(plugin.join("Config")).unwrap();
        fs::write(
            plugin.join("SamplePlugin.awplugin"),
            descriptor_with_workflow_json("SamplePlugin"),
        )
        .unwrap();
        fs::write(
            plugin.join("Config").join("task.schema.json"),
            r#"{"type":"object","required":["branch"],"properties":{"branch":{"type":"string"}}}"#,
        )
        .unwrap();

        let mounted = mount_plugin(&root.join("mounts.json"), &plugin).unwrap();

        assert_eq!(mounted.workflows.len(), 1);
        assert_eq!(mounted.workflows[0].name, "SampleFlow");
        assert_eq!(mounted.workflows[0].legacy_ids, vec!["sample-flow"]);
        assert_eq!(mounted.workflows[0].task_schema["type"], "object");
    }

    #[test]
    fn agentwatcher_source_tree_bundles_no_plugins() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("src-tauri should have an AgentWatcher parent");
        let plugins = source_root.join("Plugins");
        let snapshot = scan_catalog(&source_root.join("missing-mounts.json"), &plugins);
        assert!(snapshot.plugins.is_empty());
        assert!(snapshot.errors.is_empty());
    }

    #[test]
    #[ignore = "live external plugin handshake; set AGENTWATCHER_LIVE_PLUGIN_ROOT"]
    fn live_external_plugin_handshake() {
        let root = std::env::var_os("AGENTWATCHER_LIVE_PLUGIN_ROOT")
            .map(PathBuf::from)
            .expect("AGENTWATCHER_LIVE_PLUGIN_ROOT is required");
        let plugin = read_plugin_at_root(&root, true).expect("plugin should be valid");
        let response = invoke_plugin(
            &plugin,
            &AwPluginInvokeRequest {
                plugin_name: plugin.descriptor.name.clone(),
                command: plugin.descriptor.runtime.handshake_command.clone(),
                request: serde_json::json!({}),
            },
        )
        .expect("plugin handshake should succeed");
        assert_eq!(response.result["protocol"], "AgentWatcherPluginStdio/1");
        assert_eq!(response.result["plugin"], plugin.descriptor.name);
    }

    fn descriptor_json(name: &str) -> String {
        format!(
            r#"{{
  "FileVersion": 1,
  "Version": 1,
  "VersionName": "0.1.0",
  "Name": "{name}",
  "FriendlyName": "Test",
  "Description": "Test plugin",
  "Category": "Tests",
  "EnabledByDefault": false,
  "Installed": false,
  "CanContainContent": false,
  "Modules": [{{"Name":"Core","FriendlyName":"Core","Type":"Runtime","LoadingPhase":"Default","Package":"core","Path":"Source/Core","UserFacing":false,"Dependencies":[]}}],
  "Runtime": {{"Protocol":"AgentWatcherPluginStdio/1","ExecutableCandidates":["target/debug/test.exe"],"InvokeArguments":["host","invoke","--json"],"HandshakeCommand":"plugin.handshake"}},
  "Commands": [{{"Name":"plugin.handshake","Safety":"ReadOnly"}}],
  "Contributions": {{"Workflows":[]}}
}}"#
        )
    }

    fn descriptor_with_workflow_json(name: &str) -> String {
        descriptor_json(name).replace(
            r#""Contributions": {"Workflows":[]}"#,
            r#""Contributions": {"Workflows":[{"Name":"SampleFlow","FriendlyName":"Sample Flow","Description":"Test workflow","LegacyIds":["sample-flow"],"TaskSchema":"Config/task.schema.json","PlanCommand":"workflow.plan","ExecuteCommand":"workflow.execute"}]}"#,
        )
        .replace(
            r#""Commands": [{"Name":"plugin.handshake","Safety":"ReadOnly"}]"#,
            r#""Commands": [{"Name":"plugin.handshake","Safety":"ReadOnly"},{"Name":"workflow.plan","Safety":"DryRun"},{"Name":"workflow.execute","Safety":"BoundedWrite","RequiresConfirmation":true,"ConfirmationBoundary":"Sample.execute.v1"}]"#,
        )
    }

    fn temp_dir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("agentwatcher-plugin-{label}-{stamp}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
