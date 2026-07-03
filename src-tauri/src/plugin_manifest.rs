use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const AW_SCHEMA_VERSION: u32 = 1;
pub const PLUGIN_KIND: &str = "AgentWatcherPlugin";
pub const MODULE_KIND: &str = "AgentWatcherModule";
pub const RESULT_KIND: &str = "AgentWatcherModuleResult";
pub const TASK_KIND: &str = "AgentWatcherTaskExecution";
pub const PLUGIN_DESCRIPTOR_FILE: &str = "AgentWatcher.awplugin.json";
pub const MODULE_DESCRIPTOR_FILE: &str = "AgentWatcher.awmodule.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwValidationError {
    pub errors: Vec<String>,
}

impl AwValidationError {
    fn new(errors: Vec<String>) -> Self {
        Self { errors }
    }
}

impl fmt::Display for AwValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.errors.join("; "))
    }
}

impl std::error::Error for AwValidationError {}

pub fn validate_agentwatcher_name(label: &str, value: &str) -> Result<(), AwValidationError> {
    let mut errors = Vec::new();
    validate_name(label, value, &mut errors);
    finish_validation(errors)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwPluginScanError {
    pub errors: Vec<String>,
}

impl AwPluginScanError {
    fn new(errors: Vec<String>) -> Self {
        Self { errors }
    }
}

impl fmt::Display for AwPluginScanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.errors.join("; "))
    }
}

impl std::error::Error for AwPluginScanError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwPluginDescriptor {
    pub schema_version: u32,
    pub kind: String,
    pub name: String,
    pub display_name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    pub modules: Vec<AwModuleReference>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwModuleReference {
    pub name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub manifest_path: Option<String>,
    #[serde(default)]
    pub mount_path: Option<String>,
    #[serde(default)]
    pub workspace_project: Option<String>,
    #[serde(default)]
    pub link: Option<AwModuleLinkDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwModuleLinkDescriptor {
    #[serde(default)]
    pub target_project: Option<String>,
    #[serde(default)]
    pub source_module: Option<String>,
    #[serde(default)]
    pub source_project: Option<String>,
    #[serde(default)]
    pub worktree_path: Option<String>,
    #[serde(default)]
    pub worktree_branch: Option<String>,
    #[serde(default)]
    pub link_type: AwModuleLinkType,
    #[serde(default = "default_required")]
    pub required: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AwModuleLinkType {
    #[default]
    Junction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwModuleDescriptor {
    pub schema_version: u32,
    pub kind: String,
    pub name: String,
    pub display_name: String,
    pub module_type: AwModuleType,
    pub source: AwModuleSource,
    #[serde(default)]
    pub description: Option<String>,
    pub commands: Vec<AwCommandDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AwModuleType {
    Workflow,
    Agent,
    KnowledgeBase,
    Utility,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwModuleSource {
    pub project_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwCommandDescriptor {
    pub name: String,
    pub display_name: String,
    pub safety: AwCommandSafety,
    pub runner: AwCommandRunner,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub produces: AwCommandOutputKind,
    #[serde(default)]
    pub safety_contract: Option<AwCommandSafetyContract>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AwCommandSafety {
    ReadOnly,
    BoundedWrite,
    DryRun,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwCommandSafetyContract {
    #[serde(default)]
    pub requires_dry_run: bool,
    #[serde(default)]
    pub requires_confirmation: bool,
    #[serde(default)]
    pub confirmation_boundary: Option<String>,
    #[serde(default)]
    pub impact_scope: Vec<String>,
    #[serde(default)]
    pub audit_record: bool,
    #[serde(default)]
    pub forbids_arbitrary_shell: bool,
    #[serde(default)]
    pub forbids_raw_ue_build: bool,
    #[serde(default)]
    pub forbids_destructive_delete: bool,
    #[serde(default)]
    pub forbids_external_worktree: bool,
    #[serde(default)]
    pub allowed_actions: Vec<String>,
    #[serde(default)]
    pub allowed_runners: Vec<AwCommandRunnerKind>,
    #[serde(default)]
    pub runtime_block: Option<AwCommandRuntimeBlock>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AwCommandRunnerKind {
    NativeExecutable,
    PowerShellFile,
    NodeScript,
    PythonModule,
    CargoRun,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwCommandRuntimeBlock {
    pub reason: String,
    pub override_env: String,
    pub override_token: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AwCommandOutputKind {
    #[default]
    AgentWatcherModuleResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum AwCommandRunner {
    NativeExecutable {
        program: String,
        #[serde(default)]
        args: Vec<String>,
    },
    PowerShellFile {
        script: String,
        #[serde(default)]
        args: Vec<String>,
    },
    NodeScript {
        script: String,
        #[serde(default)]
        args: Vec<String>,
    },
    PythonModule {
        module: String,
        #[serde(default)]
        args: Vec<String>,
    },
    CargoRun {
        #[serde(default)]
        package: Option<String>,
        #[serde(default)]
        args: Vec<String>,
    },
}

impl AwCommandRunner {
    pub fn kind(&self) -> AwCommandRunnerKind {
        match self {
            AwCommandRunner::NativeExecutable { .. } => AwCommandRunnerKind::NativeExecutable,
            AwCommandRunner::PowerShellFile { .. } => AwCommandRunnerKind::PowerShellFile,
            AwCommandRunner::NodeScript { .. } => AwCommandRunnerKind::NodeScript,
            AwCommandRunner::PythonModule { .. } => AwCommandRunnerKind::PythonModule,
            AwCommandRunner::CargoRun { .. } => AwCommandRunnerKind::CargoRun,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwModuleExecutionResult {
    pub schema_version: u32,
    pub kind: String,
    #[serde(default)]
    pub plugin_name: Option<String>,
    pub module_name: String,
    pub command_name: String,
    pub status: AwExecutionStatus,
    pub summary: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub artifacts: Vec<AwResultArtifact>,
    #[serde(default)]
    pub metrics: Map<String, Value>,
    #[serde(default)]
    pub details: Option<Value>,
    #[serde(default)]
    pub stdout_excerpt: Option<String>,
    #[serde(default)]
    pub stderr_excerpt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AwExecutionStatus {
    Running,
    Success,
    Failed,
    Blocked,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwResultArtifact {
    pub label: String,
    pub path: String,
    pub artifact_type: AwResultArtifactType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AwResultArtifactType {
    Json,
    Markdown,
    Log,
    Directory,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AwTaskExecutionInfo {
    pub schema_version: u32,
    pub kind: String,
    pub task_id: String,
    pub plugin_name: String,
    pub module_name: String,
    pub command_name: String,
    pub status: AwExecutionStatus,
    pub started_ms: u64,
    #[serde(default)]
    pub finished_ms: Option<u64>,
    #[serde(default)]
    pub result_path: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AwDiscoveredPlugin {
    pub descriptor: AwPluginDescriptor,
    pub manifest_path: String,
    pub root_path: String,
    pub display_path: String,
    pub modules: Vec<AwDiscoveredModule>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AwDiscoveredModule {
    pub descriptor: AwModuleDescriptor,
    pub manifest_path: String,
    pub root_path: String,
    pub identity_path: String,
    pub display_path: String,
}

pub fn parse_plugin_descriptor(text: &str) -> Result<AwPluginDescriptor, AwValidationError> {
    let descriptor: AwPluginDescriptor = parse_json(text)?;
    validate_plugin_descriptor(&descriptor)?;
    Ok(descriptor)
}

pub fn parse_module_descriptor(text: &str) -> Result<AwModuleDescriptor, AwValidationError> {
    let descriptor: AwModuleDescriptor = parse_json(text)?;
    validate_module_descriptor(&descriptor)?;
    Ok(descriptor)
}

pub fn parse_module_execution_result(
    text: &str,
) -> Result<AwModuleExecutionResult, AwValidationError> {
    let result: AwModuleExecutionResult = parse_json(text)?;
    validate_module_execution_result(&result)?;
    Ok(result)
}

pub fn parse_task_execution_info(text: &str) -> Result<AwTaskExecutionInfo, AwValidationError> {
    let info: AwTaskExecutionInfo = parse_json(text)?;
    validate_task_execution_info(&info)?;
    Ok(info)
}

pub fn scan_plugin_root(plugin_root: &Path) -> Result<Vec<AwDiscoveredPlugin>, AwPluginScanError> {
    if !plugin_root.exists() {
        return Err(AwPluginScanError::new(vec![format!(
            "plugin root does not exist: {}",
            path_to_string(plugin_root)
        )]));
    }
    if !plugin_root.is_dir() {
        return Err(AwPluginScanError::new(vec![format!(
            "plugin root is not a directory: {}",
            path_to_string(plugin_root)
        )]));
    }

    let mut errors = Vec::new();
    let mut plugins = Vec::new();
    let entries = fs::read_dir(plugin_root).map_err(|error| {
        AwPluginScanError::new(vec![format!(
            "failed to read plugin root {}: {}",
            path_to_string(plugin_root),
            error
        )])
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                errors.push(format!("failed to read plugin directory entry: {error}"));
                continue;
            }
        };
        let plugin_dir = entry.path();
        if !plugin_dir.is_dir() {
            continue;
        }

        let descriptor_path = plugin_dir.join(PLUGIN_DESCRIPTOR_FILE);
        if !descriptor_path.exists() {
            continue;
        }

        match read_discovered_plugin(&plugin_dir, &descriptor_path) {
            Ok(plugin) => plugins.push(plugin),
            Err(error) => errors.extend(error.errors),
        }
    }

    plugins.sort_by(|left, right| left.descriptor.name.cmp(&right.descriptor.name));
    finish_scan(errors, plugins)
}

pub fn validate_plugin_descriptor(
    descriptor: &AwPluginDescriptor,
) -> Result<(), AwValidationError> {
    let mut errors = Vec::new();
    validate_schema_header(
        descriptor.schema_version,
        &descriptor.kind,
        PLUGIN_KIND,
        "plugin",
        &mut errors,
    );
    validate_name("plugin.name", &descriptor.name, &mut errors);
    validate_non_empty("plugin.displayName", &descriptor.display_name, &mut errors);

    if descriptor.modules.is_empty() {
        errors.push("plugin.modules must contain at least one module reference".to_string());
    }

    let mut names = HashSet::new();
    for (index, module) in descriptor.modules.iter().enumerate() {
        let prefix = format!("plugin.modules[{index}]");
        validate_name(&format!("{prefix}.name"), &module.name, &mut errors);
        if let Some(display_name) = &module.display_name {
            validate_non_empty(&format!("{prefix}.displayName"), display_name, &mut errors);
        }
        if module.manifest_path.is_none() && module.mount_path.is_none() {
            errors.push(format!("{prefix} must declare manifestPath or mountPath"));
        }
        if let Some(manifest_path) = &module.manifest_path {
            validate_relative_path(
                &format!("{prefix}.manifestPath"),
                manifest_path,
                &mut errors,
            );
        }
        if let Some(mount_path) = &module.mount_path {
            validate_relative_path(&format!("{prefix}.mountPath"), mount_path, &mut errors);
        }
        if let Some(workspace_project) = &module.workspace_project {
            validate_name(
                &format!("{prefix}.workspaceProject"),
                workspace_project,
                &mut errors,
            );
        }
        if let Some(link) = &module.link {
            validate_module_link_descriptor(&prefix, link, &mut errors);
        }
        if !names.insert(module.name.to_ascii_lowercase()) {
            errors.push(format!("{prefix}.name duplicates another module reference"));
        }
    }

    finish_validation(errors)
}

pub fn validate_module_descriptor(
    descriptor: &AwModuleDescriptor,
) -> Result<(), AwValidationError> {
    let mut errors = Vec::new();
    validate_schema_header(
        descriptor.schema_version,
        &descriptor.kind,
        MODULE_KIND,
        "module",
        &mut errors,
    );
    validate_name("module.name", &descriptor.name, &mut errors);
    validate_non_empty("module.displayName", &descriptor.display_name, &mut errors);
    validate_relative_path_or_current(
        "module.source.projectPath",
        &descriptor.source.project_path,
        &mut errors,
    );

    if descriptor.commands.is_empty() {
        errors.push("module.commands must contain at least one safe command".to_string());
    }

    let mut names = HashSet::new();
    for (index, command) in descriptor.commands.iter().enumerate() {
        let prefix = format!("module.commands[{index}]");
        validate_name(&format!("{prefix}.name"), &command.name, &mut errors);
        validate_non_empty(
            &format!("{prefix}.displayName"),
            &command.display_name,
            &mut errors,
        );
        validate_command_runner(&prefix, &command.runner, &mut errors);
        validate_command_safety_contract(&prefix, command, &mut errors);
        if command.timeout_ms == 0 {
            errors.push(format!("{prefix}.timeoutMs must be greater than 0"));
        }
        if !names.insert(command.name.to_ascii_lowercase()) {
            errors.push(format!("{prefix}.name duplicates another command"));
        }
    }

    finish_validation(errors)
}

pub fn validate_module_execution_result(
    result: &AwModuleExecutionResult,
) -> Result<(), AwValidationError> {
    let mut errors = Vec::new();
    validate_schema_header(
        result.schema_version,
        &result.kind,
        RESULT_KIND,
        "result",
        &mut errors,
    );
    if let Some(plugin_name) = &result.plugin_name {
        validate_name("result.pluginName", plugin_name, &mut errors);
    }
    validate_name("result.moduleName", &result.module_name, &mut errors);
    validate_name("result.commandName", &result.command_name, &mut errors);
    validate_non_empty("result.summary", &result.summary, &mut errors);
    if let Some(task_id) = &result.task_id {
        validate_name("result.taskId", task_id, &mut errors);
    }

    for (index, artifact) in result.artifacts.iter().enumerate() {
        let prefix = format!("result.artifacts[{index}]");
        validate_non_empty(&format!("{prefix}.label"), &artifact.label, &mut errors);
        validate_relative_path(&format!("{prefix}.path"), &artifact.path, &mut errors);
    }

    finish_validation(errors)
}

pub fn validate_task_execution_info(info: &AwTaskExecutionInfo) -> Result<(), AwValidationError> {
    let mut errors = Vec::new();
    validate_schema_header(
        info.schema_version,
        &info.kind,
        TASK_KIND,
        "taskExecution",
        &mut errors,
    );
    validate_name("taskExecution.taskId", &info.task_id, &mut errors);
    validate_name("taskExecution.pluginName", &info.plugin_name, &mut errors);
    validate_name("taskExecution.moduleName", &info.module_name, &mut errors);
    validate_name("taskExecution.commandName", &info.command_name, &mut errors);
    if info.started_ms == 0 {
        errors.push("taskExecution.startedMs must be greater than 0".to_string());
    }
    if let Some(finished_ms) = info.finished_ms {
        if finished_ms < info.started_ms {
            errors.push(
                "taskExecution.finishedMs must be greater than or equal to startedMs".to_string(),
            );
        }
    }
    if let Some(result_path) = &info.result_path {
        validate_relative_path("taskExecution.resultPath", result_path, &mut errors);
    }

    finish_validation(errors)
}

fn read_discovered_plugin(
    plugin_dir: &Path,
    descriptor_path: &Path,
) -> Result<AwDiscoveredPlugin, AwPluginScanError> {
    let descriptor_text = fs::read_to_string(descriptor_path).map_err(|error| {
        AwPluginScanError::new(vec![format!(
            "failed to read plugin descriptor {}: {}",
            path_to_string(descriptor_path),
            error
        )])
    })?;
    let descriptor = parse_plugin_descriptor(&descriptor_text).map_err(|error| {
        AwPluginScanError::new(
            error
                .errors
                .into_iter()
                .map(|message| {
                    format!(
                        "invalid plugin descriptor {}: {}",
                        path_to_string(descriptor_path),
                        message
                    )
                })
                .collect(),
        )
    })?;
    let plugin_root = fs::canonicalize(plugin_dir).map_err(|error| {
        AwPluginScanError::new(vec![format!(
            "failed to resolve plugin root {}: {}",
            path_to_string(plugin_dir),
            error
        )])
    })?;

    let mut errors = Vec::new();
    let mut modules = Vec::new();
    let mut identity_paths = HashSet::new();
    for module_ref in &descriptor.modules {
        match read_discovered_module(plugin_dir, module_ref) {
            Ok(module) => {
                if module.descriptor.name != module_ref.name {
                    errors.push(format!(
                        "module reference {} points to descriptor named {}",
                        module_ref.name, module.descriptor.name
                    ));
                }
                let identity_key = identity_path_key(&module.identity_path);
                if !identity_paths.insert(identity_key) {
                    errors.push(format!(
                        "module reference {} resolves to duplicate module identity {}",
                        module_ref.name, module.identity_path
                    ));
                }
                modules.push(module);
            }
            Err(error) => errors.extend(error.errors),
        }
    }

    if errors.is_empty() {
        let plugin_manifest_path = fs::canonicalize(descriptor_path).map_err(|error| {
            AwPluginScanError::new(vec![format!(
                "failed to resolve plugin descriptor {}: {}",
                path_to_string(descriptor_path),
                error
            )])
        })?;
        Ok(AwDiscoveredPlugin {
            descriptor,
            manifest_path: path_to_string(&plugin_manifest_path),
            root_path: path_to_string(&plugin_root),
            display_path: path_to_string(plugin_dir),
            modules,
        })
    } else {
        Err(AwPluginScanError::new(errors))
    }
}

fn read_discovered_module(
    plugin_dir: &Path,
    module_ref: &AwModuleReference,
) -> Result<AwDiscoveredModule, AwPluginScanError> {
    let manifest_path = module_ref_manifest_path(module_ref).ok_or_else(|| {
        AwPluginScanError::new(vec![format!(
            "module reference {} does not declare manifestPath or mountPath",
            module_ref.name
        )])
    })?;
    let display_manifest_path =
        safe_join_relative(plugin_dir, &manifest_path).ok_or_else(|| {
            AwPluginScanError::new(vec![format!(
                "module reference {} has an unsafe manifest path {}",
                module_ref.name, manifest_path
            )])
        })?;
    let canonical_manifest_path =
        fs::canonicalize(&display_manifest_path).map_err(|link_error| {
            AwPluginScanError::new(vec![format!(
                "module reference {} is broken or not installed: {} ({})",
                module_ref.name,
                path_to_string(&display_manifest_path),
                link_error
            )])
        })?;
    if !canonical_manifest_path.is_file() {
        return Err(AwPluginScanError::new(vec![format!(
            "module reference {} is not a file: {}",
            module_ref.name,
            path_to_string(&display_manifest_path)
        )]));
    }

    let descriptor_text = fs::read_to_string(&canonical_manifest_path).map_err(|error| {
        AwPluginScanError::new(vec![format!(
            "failed to read module descriptor {}: {}",
            path_to_string(&display_manifest_path),
            error
        )])
    })?;
    let descriptor = parse_module_descriptor(&descriptor_text).map_err(|error| {
        AwPluginScanError::new(
            error
                .errors
                .into_iter()
                .map(|message| {
                    format!(
                        "invalid module descriptor {}: {}",
                        path_to_string(&display_manifest_path),
                        message
                    )
                })
                .collect(),
        )
    })?;

    let descriptor_dir = canonical_manifest_path.parent().ok_or_else(|| {
        AwPluginScanError::new(vec![format!(
            "module reference {} has no parent directory",
            module_ref.name
        )])
    })?;
    let display_descriptor_dir = display_manifest_path.parent().ok_or_else(|| {
        AwPluginScanError::new(vec![format!(
            "module reference {} has no display parent directory",
            module_ref.name
        )])
    })?;
    let display_root_path =
        safe_join_relative(display_descriptor_dir, &descriptor.source.project_path).ok_or_else(
            || {
                AwPluginScanError::new(vec![format!(
                    "module {} has an unsafe source.projectPath {}",
                    descriptor.name, descriptor.source.project_path
                )])
            },
        )?;
    let identity_root_path = safe_join_relative(descriptor_dir, &descriptor.source.project_path)
        .and_then(|path| fs::canonicalize(path).ok())
        .ok_or_else(|| {
            AwPluginScanError::new(vec![format!(
                "module {} source.projectPath is broken: {}",
                descriptor.name, descriptor.source.project_path
            )])
        })?;
    if !identity_root_path.is_dir() {
        return Err(AwPluginScanError::new(vec![format!(
            "module {} source.projectPath is not a directory: {}",
            descriptor.name,
            path_to_string(&display_root_path)
        )]));
    }

    Ok(AwDiscoveredModule {
        descriptor,
        manifest_path: path_to_string(&canonical_manifest_path),
        root_path: path_to_string(&identity_root_path),
        identity_path: path_to_string(&identity_root_path),
        display_path: path_to_string(&display_root_path),
    })
}

fn finish_scan(
    errors: Vec<String>,
    plugins: Vec<AwDiscoveredPlugin>,
) -> Result<Vec<AwDiscoveredPlugin>, AwPluginScanError> {
    if errors.is_empty() {
        Ok(plugins)
    } else {
        Err(AwPluginScanError::new(errors))
    }
}

fn parse_json<T>(text: &str) -> Result<T, AwValidationError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(text)
        .map_err(|error| AwValidationError::new(vec![format!("invalid JSON: {error}")]))
}

fn finish_validation(errors: Vec<String>) -> Result<(), AwValidationError> {
    if errors.is_empty() {
        Ok(())
    } else {
        Err(AwValidationError::new(errors))
    }
}

fn validate_schema_header(
    schema_version: u32,
    kind: &str,
    expected_kind: &str,
    label: &str,
    errors: &mut Vec<String>,
) {
    if schema_version != AW_SCHEMA_VERSION {
        errors.push(format!("{label}.schemaVersion must be {AW_SCHEMA_VERSION}"));
    }
    if kind != expected_kind {
        errors.push(format!("{label}.kind must be {expected_kind}"));
    }
}

fn validate_name(label: &str, value: &str, errors: &mut Vec<String>) {
    if !is_valid_agentwatcher_name(value) {
        errors.push(format!(
            "{label} must start with an ASCII letter or digit and contain only ASCII letters, digits, '.', '_' or '-'"
        ));
    }
}

fn validate_non_empty(label: &str, value: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        errors.push(format!("{label} must not be empty"));
    }
}

fn validate_relative_path(label: &str, value: &str, errors: &mut Vec<String>) {
    if !is_safe_relative_path(value) {
        errors.push(format!(
            "{label} must be a relative path inside the plugin or module root and must not contain '..'"
        ));
    }
}

fn validate_relative_path_or_current(label: &str, value: &str, errors: &mut Vec<String>) {
    if !is_safe_relative_path_or_current(value) {
        errors.push(format!(
            "{label} must be '.' or a relative path inside the plugin or module root and must not contain '..'"
        ));
    }
}

fn validate_command_runner(prefix: &str, runner: &AwCommandRunner, errors: &mut Vec<String>) {
    match runner {
        AwCommandRunner::NativeExecutable { program, args } => {
            validate_relative_path(&format!("{prefix}.runner.program"), program, errors);
            if is_disallowed_shell_program(program) {
                errors.push(format!(
                    "{prefix}.runner.program must not be a shell executable"
                ));
            }
            validate_runner_args(prefix, args, errors);
        }
        AwCommandRunner::PowerShellFile { script, args } => {
            validate_relative_path(&format!("{prefix}.runner.script"), script, errors);
            validate_extension(&format!("{prefix}.runner.script"), script, &["ps1"], errors);
            validate_runner_args(prefix, args, errors);
        }
        AwCommandRunner::NodeScript { script, args } => {
            validate_relative_path(&format!("{prefix}.runner.script"), script, errors);
            validate_extension(
                &format!("{prefix}.runner.script"),
                script,
                &["js", "mjs", "cjs"],
                errors,
            );
            validate_runner_args(prefix, args, errors);
        }
        AwCommandRunner::PythonModule { module, args } => {
            if !is_valid_python_module(module) {
                errors.push(format!(
                    "{prefix}.runner.module must be a Python module path"
                ));
            }
            validate_runner_args(prefix, args, errors);
        }
        AwCommandRunner::CargoRun { package, args } => {
            if let Some(package) = package {
                validate_name(&format!("{prefix}.runner.package"), package, errors);
            }
            validate_runner_args(prefix, args, errors);
        }
    }
}

fn validate_command_safety_contract(
    prefix: &str,
    command: &AwCommandDescriptor,
    errors: &mut Vec<String>,
) {
    let Some(contract) = &command.safety_contract else {
        if matches!(command.safety, AwCommandSafety::BoundedWrite) {
            errors.push(format!(
                "{prefix}.safetyContract is required for boundedWrite commands"
            ));
        }
        return;
    };

    validate_optional_contract_text(
        &format!("{prefix}.safetyContract.confirmationBoundary"),
        contract.confirmation_boundary.as_deref(),
        errors,
    );
    validate_contract_list(
        &format!("{prefix}.safetyContract.impactScope"),
        &contract.impact_scope,
        errors,
    );
    validate_contract_actions(
        &format!("{prefix}.safetyContract.allowedActions"),
        &contract.allowed_actions,
        errors,
    );
    validate_runtime_block(
        &format!("{prefix}.safetyContract.runtimeBlock"),
        contract.runtime_block.as_ref(),
        errors,
    );

    if matches!(command.safety, AwCommandSafety::BoundedWrite) {
        if !contract.requires_dry_run {
            errors.push(format!(
                "{prefix}.safetyContract.requiresDryRun must be true for boundedWrite commands"
            ));
        }
        if !contract.requires_confirmation {
            errors.push(format!(
                "{prefix}.safetyContract.requiresConfirmation must be true for boundedWrite commands"
            ));
        }
        if contract
            .confirmation_boundary
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            errors.push(format!(
                "{prefix}.safetyContract.confirmationBoundary is required for boundedWrite commands"
            ));
        }
        if contract.impact_scope.is_empty() {
            errors.push(format!(
                "{prefix}.safetyContract.impactScope must declare at least one bounded impact"
            ));
        }
        if !contract.audit_record {
            errors.push(format!(
                "{prefix}.safetyContract.auditRecord must be true for boundedWrite commands"
            ));
        }
        if !contract.forbids_arbitrary_shell {
            errors.push(format!(
                "{prefix}.safetyContract.forbidsArbitraryShell must be true for boundedWrite commands"
            ));
        }
        if !contract.forbids_raw_ue_build {
            errors.push(format!(
                "{prefix}.safetyContract.forbidsRawUeBuild must be true for boundedWrite commands"
            ));
        }
        if !contract.forbids_destructive_delete {
            errors.push(format!(
                "{prefix}.safetyContract.forbidsDestructiveDelete must be true for boundedWrite commands"
            ));
        }
        if !contract.forbids_external_worktree {
            errors.push(format!(
                "{prefix}.safetyContract.forbidsExternalWorktree must be true in the current UEWorkflow phase"
            ));
        }
        if contract.allowed_actions.is_empty() {
            errors.push(format!(
                "{prefix}.safetyContract.allowedActions must declare at least one action"
            ));
        }
        if contract.allowed_runners.is_empty() {
            errors.push(format!(
                "{prefix}.safetyContract.allowedRunners must declare the runner kinds allowed for this boundedWrite command"
            ));
        } else if !contract.allowed_runners.contains(&command.runner.kind()) {
            errors.push(format!(
                "{prefix}.runner.type must be declared in {prefix}.safetyContract.allowedRunners"
            ));
        }
        if contract.runtime_block.is_none() {
            errors.push(format!(
                "{prefix}.safetyContract.runtimeBlock is required for boundedWrite commands in the current phase"
            ));
        }
    }
}

fn validate_optional_contract_text(label: &str, value: Option<&str>, errors: &mut Vec<String>) {
    let Some(value) = value else {
        return;
    };
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 160 || trimmed.contains('\0') {
        errors.push(format!("{label} must be non-empty text up to 160 bytes"));
    }
}

fn validate_contract_list(label: &str, values: &[String], errors: &mut Vec<String>) {
    for (index, value) in values.iter().enumerate() {
        let trimmed = value.trim();
        if trimmed.is_empty() || trimmed.len() > 160 || trimmed.contains('\0') {
            errors.push(format!(
                "{label}[{index}] must be non-empty text up to 160 bytes"
            ));
        }
    }
}

fn validate_contract_actions(label: &str, values: &[String], errors: &mut Vec<String>) {
    for (index, value) in values.iter().enumerate() {
        if !is_valid_agentwatcher_name(value) {
            errors.push(format!(
                "{label}[{index}] must start with an ASCII letter or digit and contain only ASCII letters, digits, '.', '_' or '-'"
            ));
        }
    }
}

fn validate_runtime_block(
    label: &str,
    value: Option<&AwCommandRuntimeBlock>,
    errors: &mut Vec<String>,
) {
    let Some(value) = value else {
        return;
    };
    validate_non_empty(&format!("{label}.reason"), &value.reason, errors);
    if value.reason.len() > 240 || value.reason.contains('\0') {
        errors.push(format!("{label}.reason must be text up to 240 bytes"));
    }
    validate_name(&format!("{label}.overrideEnv"), &value.override_env, errors);
    let token = value.override_token.trim();
    if token.len() < 12
        || token.len() > 160
        || token.contains('\0')
        || token.contains(char::is_whitespace)
    {
        errors.push(format!(
            "{label}.overrideToken must be non-empty token text without whitespace"
        ));
    }
}

fn validate_runner_args(prefix: &str, args: &[String], errors: &mut Vec<String>) {
    for (index, arg) in args.iter().enumerate() {
        if arg.contains('\0') || arg.contains('\n') || arg.contains('\r') {
            errors.push(format!(
                "{prefix}.runner.args[{index}] must not contain control characters"
            ));
        }
    }
}

fn validate_module_link_descriptor(
    prefix: &str,
    link: &AwModuleLinkDescriptor,
    errors: &mut Vec<String>,
) {
    let has_legacy_target = link
        .target_project
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let declared_source = link
        .source_module
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            link.source_project
                .as_deref()
                .filter(|value| !value.trim().is_empty())
        });
    let has_worktree = declared_source.is_some()
        || link
            .worktree_path
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || link
            .worktree_branch
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());

    if has_worktree {
        let Some(source_module) = declared_source else {
            errors.push(format!(
                "{prefix}.link.sourceModule must be set for managed worktree links"
            ));
            return;
        };
        let Some(worktree_path) = link.worktree_path.as_deref() else {
            errors.push(format!(
                "{prefix}.link.worktreePath must be set for managed worktree links"
            ));
            return;
        };
        let Some(worktree_branch) = link.worktree_branch.as_deref() else {
            errors.push(format!(
                "{prefix}.link.worktreeBranch must be set for managed worktree links"
            ));
            return;
        };
        validate_name(
            &format!("{prefix}.link.sourceModule"),
            source_module,
            errors,
        );
        validate_relative_path(
            &format!("{prefix}.link.worktreePath"),
            worktree_path,
            errors,
        );
        validate_git_branch_name(
            &format!("{prefix}.link.worktreeBranch"),
            worktree_branch,
            errors,
        );
        if !worktree_path
            .replace('\\', "/")
            .starts_with(".plugin-worktrees/UEWorkflow/")
        {
            errors.push(format!(
                "{prefix}.link.worktreePath must stay under .plugin-worktrees/UEWorkflow"
            ));
        }
        return;
    }

    if let Some(target_project) = link.target_project.as_deref() {
        validate_name(
            &format!("{prefix}.link.targetProject"),
            target_project,
            errors,
        );
    } else if !has_legacy_target {
        errors.push(format!(
            "{prefix}.link must declare targetProject or managed worktree fields"
        ));
    }
}

fn validate_extension(label: &str, value: &str, allowed: &[&str], errors: &mut Vec<String>) {
    let extension = value
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());
    let is_allowed = extension
        .as_deref()
        .map(|extension| allowed.contains(&extension))
        .unwrap_or(false);
    if !is_allowed {
        errors.push(format!(
            "{label} must end with one of: {}",
            allowed.join(", ")
        ));
    }
}

fn default_timeout_ms() -> u64 {
    60_000
}

fn default_required() -> bool {
    true
}

pub fn module_ref_mount_path(module_ref: &AwModuleReference) -> Option<String> {
    module_ref.mount_path.clone().or_else(|| {
        module_ref
            .manifest_path
            .as_deref()
            .and_then(|path| path.rsplit_once('/').or_else(|| path.rsplit_once('\\')))
            .map(|(parent, _)| parent.to_string())
    })
}

pub fn module_ref_manifest_path(module_ref: &AwModuleReference) -> Option<String> {
    module_ref.manifest_path.clone().or_else(|| {
        module_ref.mount_path.as_ref().map(|mount_path| {
            format!(
                "{}/{}",
                mount_path.trim_end_matches(['/', '\\']),
                MODULE_DESCRIPTOR_FILE
            )
        })
    })
}

fn is_valid_agentwatcher_name(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.len() > 96 {
        return false;
    }

    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphanumeric() {
        return false;
    }

    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

fn validate_git_branch_name(label: &str, value: &str, errors: &mut Vec<String>) {
    if !is_valid_git_branch_name(value) {
        errors.push(format!("{label} must be a safe relative git branch name"));
    }
}

fn is_valid_git_branch_name(value: &str) -> bool {
    let normalized = value.trim().replace('\\', "/");
    if normalized.is_empty()
        || normalized.len() > 160
        || normalized.starts_with('/')
        || normalized.ends_with('/')
        || normalized.starts_with('-')
        || normalized.contains("..")
        || normalized.contains("//")
        || normalized.contains("@{")
        || normalized.ends_with(".lock")
    {
        return false;
    }

    normalized.split('/').all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && !segment.starts_with('.')
            && !segment.ends_with('.')
            && segment
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    })
}

fn is_safe_relative_path(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.contains('\0') || trimmed.contains(':') {
        return false;
    }

    let normalized = trimmed.replace('\\', "/");
    if normalized.starts_with('/') || normalized.starts_with("//") {
        return false;
    }

    normalized
        .split('/')
        .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}

fn is_safe_relative_path_or_current(value: &str) -> bool {
    value.trim() == "." || is_safe_relative_path(value)
}

fn is_valid_python_module(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    trimmed.split('.').all(is_valid_python_module_segment)
}

fn is_valid_python_module_segment(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn is_disallowed_shell_program(program: &str) -> bool {
    let normalized = program.replace('\\', "/").to_ascii_lowercase();
    let name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    matches!(
        name,
        "cmd"
            | "cmd.exe"
            | "powershell"
            | "powershell.exe"
            | "pwsh"
            | "pwsh.exe"
            | "bash"
            | "bash.exe"
            | "sh"
            | "sh.exe"
            | "wsl"
            | "wsl.exe"
    )
}

fn safe_join_relative(root: &Path, relative: &str) -> Option<PathBuf> {
    if !is_safe_relative_path_or_current(relative) {
        return None;
    }
    if relative.trim() == "." {
        return Some(root.to_path_buf());
    }

    let mut path = root.to_path_buf();
    for segment in relative.replace('\\', "/").split('/') {
        path.push(segment);
    }
    Some(path)
}

fn identity_path_key(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    const VALID_PLUGIN: &str = r#"{
        "schemaVersion": 1,
        "kind": "AgentWatcherPlugin",
        "name": "UEWorkflow",
        "displayName": "虚幻项目开发工作流",
        "category": "unreal",
        "modules": [
            {
                "name": "DevFlow",
                "displayName": "开发流",
                "mountPath": "Modules/DevFlow",
                "link": {
                    "sourceModule": "DevFlow",
                    "worktreePath": ".plugin-worktrees/UEWorkflow/DevFlow",
                    "worktreeBranch": "agentwatcher/ueworkflow/devflow",
                    "linkType": "junction",
                    "required": true
                }
            },
            {
                "name": "AgentHub",
                "displayName": "智能体",
                "mountPath": "Modules/AgentHub",
                "link": {
                    "sourceModule": "AgentHub",
                    "worktreePath": ".plugin-worktrees/UEWorkflow/AgentHub",
                    "worktreeBranch": "agentwatcher/ueworkflow/agenthub",
                    "linkType": "junction",
                    "required": true
                }
            },
            {
                "name": "KnowledgeBase",
                "displayName": "知识库",
                "mountPath": "Modules/KnowledgeBase",
                "link": {
                    "sourceModule": "KnowledgeBase",
                    "worktreePath": ".plugin-worktrees/UEWorkflow/KnowledgeBase",
                    "worktreeBranch": "agentwatcher/ueworkflow/knowledgebase",
                    "linkType": "junction",
                    "required": true
                }
            }
        ]
    }"#;

    const VALID_MODULE: &str = r#"{
        "schemaVersion": 1,
        "kind": "AgentWatcherModule",
        "name": "DevFlow",
        "displayName": "开发流",
        "moduleType": "workflow",
        "source": { "projectPath": "." },
        "commands": [
            {
                "name": "status",
                "displayName": "状态检查",
                "safety": "readOnly",
                "runner": {
                    "type": "cargoRun",
                    "args": ["--", "aw-status", "--json"]
                },
                "timeoutMs": 60000
            }
        ]
    }"#;

    #[test]
    fn valid_plugin_descriptor_accepts_linked_unreal_modules() {
        let plugin = parse_plugin_descriptor(VALID_PLUGIN).unwrap();

        assert_eq!(plugin.name, "UEWorkflow");
        assert_eq!(plugin.modules.len(), 3);
        assert_eq!(plugin.modules[0].display_name.as_deref(), Some("开发流"));
        assert_eq!(
            module_ref_manifest_path(&plugin.modules[2]).as_deref(),
            Some("Modules/KnowledgeBase/AgentWatcher.awmodule.json")
        );
        assert_eq!(
            plugin.modules[2]
                .link
                .as_ref()
                .unwrap()
                .source_module
                .as_deref(),
            Some("KnowledgeBase")
        );
        assert_eq!(
            plugin.modules[2]
                .link
                .as_ref()
                .unwrap()
                .worktree_path
                .as_deref(),
            Some(".plugin-worktrees/UEWorkflow/KnowledgeBase")
        );
    }

    #[test]
    fn invalid_plugin_descriptor_reports_version_duplicate_and_unsafe_path() {
        let invalid = r#"{
            "schemaVersion": 2,
            "kind": "AgentWatcherPlugin",
            "name": "UE Workflow",
            "displayName": "",
            "modules": [
                { "name": "DevFlow", "manifestPath": "../DevFlow/AgentWatcher.awmodule.json" },
                { "name": "DevFlow", "manifestPath": "Modules/DevFlow/AgentWatcher.awmodule.json" }
            ]
        }"#;

        let error = parse_plugin_descriptor(invalid).unwrap_err();
        let joined = error.errors.join("\n");

        assert!(joined.contains("plugin.schemaVersion must be 1"));
        assert!(joined.contains("plugin.name must start"));
        assert!(joined.contains("plugin.displayName must not be empty"));
        assert!(joined.contains("plugin.modules[0].manifestPath must be a relative path"));
        assert!(joined.contains("plugin.modules[1].name duplicates"));
    }

    #[test]
    fn valid_module_descriptor_accepts_declared_safe_command() {
        let module = parse_module_descriptor(VALID_MODULE).unwrap();

        assert_eq!(module.name, "DevFlow");
        assert_eq!(module.source.project_path, ".");
        assert_eq!(module.commands[0].name, "status");
        assert_eq!(
            module.commands[0].produces,
            AwCommandOutputKind::AgentWatcherModuleResult
        );
    }

    #[test]
    fn bounded_write_command_requires_full_safety_contract() {
        let invalid = r#"{
            "schemaVersion": 1,
            "kind": "AgentWatcherModule",
            "name": "DevFlow",
            "displayName": "开发流",
            "moduleType": "workflow",
            "source": { "projectPath": "." },
            "commands": [
                {
                    "name": "execute",
                    "displayName": "执行",
                    "safety": "boundedWrite",
                    "runner": {
                        "type": "powerShellFile",
                        "script": "aw/Get-AgentWatcherModule.ps1",
                        "args": ["execute"]
                    }
                }
            ]
        }"#;

        let error = parse_module_descriptor(invalid).unwrap_err();
        let joined = error.errors.join("\n");

        assert!(joined.contains("safetyContract is required for boundedWrite"));
    }

    #[test]
    fn bounded_write_command_accepts_declared_safety_contract() {
        let valid = r#"{
            "schemaVersion": 1,
            "kind": "AgentWatcherModule",
            "name": "DevFlow",
            "displayName": "开发流",
            "moduleType": "workflow",
            "source": { "projectPath": "." },
            "commands": [
                {
                    "name": "execute",
                    "displayName": "执行",
                    "safety": "boundedWrite",
                    "safetyContract": {
                        "requiresDryRun": true,
                        "requiresConfirmation": true,
                        "confirmationBoundary": "UEWorkflow.devflow.createTask.v1",
                        "impactScope": ["AgentWatcher task draft", "future git worktree creation"],
                        "auditRecord": true,
                        "forbidsArbitraryShell": true,
                        "forbidsRawUeBuild": true,
                        "forbidsDestructiveDelete": true,
                        "forbidsExternalWorktree": true,
                        "allowedActions": ["createTask"],
                        "allowedRunners": ["powerShellFile"],
                        "runtimeBlock": {
                            "reason": "UEWorkflow real task creation is disabled by default.",
                            "overrideEnv": "AGENTWATCHER_ALLOW_REAL_UEWORKFLOW_CREATE_TASK",
                            "overrideToken": "I_UNDERSTAND_THIS_CAN_CREATE_WORKTREE"
                        }
                    },
                    "runner": {
                        "type": "powerShellFile",
                        "script": "aw/Get-AgentWatcherModule.ps1",
                        "args": ["execute"]
                    }
                }
            ]
        }"#;

        let module = parse_module_descriptor(valid).unwrap();
        let contract = module.commands[0].safety_contract.as_ref().unwrap();

        assert!(contract.requires_dry_run);
        assert_eq!(
            contract.confirmation_boundary.as_deref(),
            Some("UEWorkflow.devflow.createTask.v1")
        );
        assert_eq!(contract.allowed_actions, vec!["createTask"]);
    }

    #[test]
    fn invalid_module_descriptor_rejects_shell_command_and_bad_script_extension() {
        let invalid = r#"{
            "schemaVersion": 1,
            "kind": "AgentWatcherModule",
            "name": "AgentHub",
            "displayName": "智能体",
            "moduleType": "agent",
            "source": { "projectPath": "Modules/AgentHub" },
            "commands": [
                {
                    "name": "shell",
                    "displayName": "Shell",
                    "safety": "readOnly",
                    "runner": {
                        "type": "nativeExecutable",
                        "program": "pwsh.exe",
                        "args": ["Get-ChildItem"]
                    }
                },
                {
                    "name": "bad_script",
                    "displayName": "Bad Script",
                    "safety": "readOnly",
                    "runner": {
                        "type": "powerShellFile",
                        "script": "scripts/status.txt"
                    }
                }
            ]
        }"#;

        let error = parse_module_descriptor(invalid).unwrap_err();
        let joined = error.errors.join("\n");

        assert!(joined.contains("runner.program must not be a shell executable"));
        assert!(joined.contains("runner.script must end with one of: ps1"));
    }

    #[test]
    fn module_execution_result_and_task_info_share_display_keys() {
        let result = r#"{
            "schemaVersion": 1,
            "kind": "AgentWatcherModuleResult",
            "pluginName": "UEWorkflow",
            "moduleName": "KnowledgeBase",
            "commandName": "status",
            "taskId": "task-uekb-status",
            "status": "success",
            "summary": "知识库工程状态正常",
            "exitCode": 0,
            "artifacts": [
                { "label": "报告", "path": "output/status.json", "artifactType": "json" }
            ],
            "metrics": { "documents": 42 }
        }"#;
        let task = r#"{
            "schemaVersion": 1,
            "kind": "AgentWatcherTaskExecution",
            "taskId": "task-uekb-status",
            "pluginName": "UEWorkflow",
            "moduleName": "KnowledgeBase",
            "commandName": "status",
            "status": "success",
            "startedMs": 1781578349000,
            "finishedMs": 1781578351000,
            "resultPath": "runs/task-uekb-status/result.json",
            "summary": "知识库工程状态正常"
        }"#;

        let parsed_result = parse_module_execution_result(result).unwrap();
        let parsed_task = parse_task_execution_info(task).unwrap();

        assert_eq!(
            parsed_result.task_id.as_deref(),
            Some(parsed_task.task_id.as_str())
        );
        assert_eq!(parsed_result.module_name, parsed_task.module_name);
        assert_eq!(parsed_result.command_name, parsed_task.command_name);
    }

    #[test]
    fn invalid_task_info_reports_time_and_result_path_errors() {
        let invalid = r#"{
            "schemaVersion": 1,
            "kind": "AgentWatcherTaskExecution",
            "taskId": "task bad",
            "pluginName": "UEWorkflow",
            "moduleName": "KnowledgeBase",
            "commandName": "status",
            "status": "blocked",
            "startedMs": 20,
            "finishedMs": 10,
            "resultPath": "../outside/result.json"
        }"#;

        let error = parse_task_execution_info(invalid).unwrap_err();
        let joined = error.errors.join("\n");

        assert!(joined.contains("taskExecution.taskId must start"));
        assert!(joined.contains("finishedMs must be greater than or equal"));
        assert!(joined.contains("taskExecution.resultPath must be a relative path"));
    }

    #[test]
    fn scan_plugin_root_discovers_modules_with_identity_and_display_paths() {
        let temp = temp_scan_root("happy");
        let plugin_root = temp.join("Plugins");
        let plugin_dir = plugin_root.join("UEWorkflow");
        let module_dir = plugin_dir.join("Modules").join("DevFlow");
        fs::create_dir_all(&module_dir).unwrap();
        write_text(plugin_dir.join(PLUGIN_DESCRIPTOR_FILE), VALID_PLUGIN);
        write_text(module_dir.join(MODULE_DESCRIPTOR_FILE), VALID_MODULE);
        write_text(
            plugin_dir
                .join("Modules")
                .join("AgentHub")
                .join(MODULE_DESCRIPTOR_FILE),
            VALID_MODULE.replace("DevFlow", "AgentHub"),
        );
        write_text(
            plugin_dir
                .join("Modules")
                .join("KnowledgeBase")
                .join(MODULE_DESCRIPTOR_FILE),
            VALID_MODULE
                .replace("DevFlow", "KnowledgeBase")
                .replace("\"workflow\"", "\"knowledgeBase\""),
        );

        let plugins = scan_plugin_root(&plugin_root).unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].descriptor.name, "UEWorkflow");
        assert_eq!(plugins[0].modules.len(), 3);
        let module_names: Vec<_> = plugins[0]
            .modules
            .iter()
            .map(|module| module.descriptor.name.as_str())
            .collect();
        assert_eq!(module_names, vec!["DevFlow", "AgentHub", "KnowledgeBase"]);
        let devflow = plugins[0]
            .modules
            .iter()
            .find(|module| module.descriptor.name == "DevFlow")
            .unwrap();
        assert!(Path::new(&devflow.identity_path).is_absolute());
        assert!(devflow.display_path.contains("Modules"));
        assert_eq!(devflow.identity_path, devflow.root_path);

        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn scan_plugin_root_reports_broken_module_reference() {
        let temp = temp_scan_root("broken");
        let plugin_root = temp.join("Plugins");
        let plugin_dir = plugin_root.join("UEWorkflow");
        fs::create_dir_all(&plugin_dir).unwrap();
        write_text(plugin_dir.join(PLUGIN_DESCRIPTOR_FILE), VALID_PLUGIN);

        let error = scan_plugin_root(&plugin_root).unwrap_err();
        let joined = error.errors.join("\n");

        assert!(joined.contains("module reference DevFlow is broken or not installed"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn scan_plugin_root_reports_duplicate_canonical_module_identity() {
        let temp = temp_scan_root("duplicate");
        let plugin_root = temp.join("Plugins");
        let plugin_dir = plugin_root.join("UEWorkflow");
        let module_dir = plugin_dir.join("Modules").join("DevFlow");
        fs::create_dir_all(&module_dir).unwrap();
        let plugin = r#"{
            "schemaVersion": 1,
            "kind": "AgentWatcherPlugin",
            "name": "UEWorkflow",
            "displayName": "虚幻项目开发工作流",
            "modules": [
                { "name": "DevFlow", "manifestPath": "Modules/DevFlow/AgentWatcher.awmodule.json" },
                { "name": "DevFlowAlias", "manifestPath": "Modules/DevFlow/AgentWatcher.awmodule.json" }
            ]
        }"#;
        write_text(plugin_dir.join(PLUGIN_DESCRIPTOR_FILE), plugin);
        write_text(module_dir.join(MODULE_DESCRIPTOR_FILE), VALID_MODULE);

        let error = scan_plugin_root(&plugin_root).unwrap_err();
        let joined = error.errors.join("\n");

        assert!(joined.contains("descriptor named DevFlow"));
        assert!(joined.contains("duplicate module identity"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn scan_plugin_root_reports_unsafe_module_manifest_path() {
        let temp = temp_scan_root("unsafe");
        let plugin_root = temp.join("Plugins");
        let plugin_dir = plugin_root.join("UEWorkflow");
        fs::create_dir_all(&plugin_dir).unwrap();
        let plugin = r#"{
            "schemaVersion": 1,
            "kind": "AgentWatcherPlugin",
            "name": "UEWorkflow",
            "displayName": "虚幻项目开发工作流",
            "modules": [
                { "name": "DevFlow", "manifestPath": "../DevFlow/AgentWatcher.awmodule.json" }
            ]
        }"#;
        write_text(plugin_dir.join(PLUGIN_DESCRIPTOR_FILE), plugin);

        let error = scan_plugin_root(&plugin_root).unwrap_err();
        let joined = error.errors.join("\n");

        assert!(joined.contains("invalid plugin descriptor"));
        assert!(joined.contains("manifestPath must be a relative path"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn scan_plugin_root_does_not_use_workspace_project_without_link() {
        let temp = temp_scan_root("workspace-project");
        let agentwatcher_root = temp.join("AgentWatcher");
        let plugin_root = agentwatcher_root.join("Plugins");
        let plugin_dir = plugin_root.join("UEWorkflow");
        let project_dir = temp.join("DevFlow");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::create_dir_all(&project_dir).unwrap();
        let plugin = r#"{
            "schemaVersion": 1,
            "kind": "AgentWatcherPlugin",
            "name": "UEWorkflow",
            "displayName": "虚幻项目开发工作流",
            "modules": [
                {
                    "name": "DevFlow",
                    "manifestPath": "Modules/DevFlow/AgentWatcher.awmodule.json",
                    "workspaceProject": "DevFlow"
                }
            ]
        }"#;
        write_text(plugin_dir.join(PLUGIN_DESCRIPTOR_FILE), plugin);
        write_text(project_dir.join(MODULE_DESCRIPTOR_FILE), VALID_MODULE);

        let error = scan_plugin_root(&plugin_root).unwrap_err();
        let joined = error.errors.join("\n");

        assert!(joined.contains("module reference DevFlow is broken or not installed"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    #[ignore = "live AgentWatcher plugin scan; set AGENTWATCHER_LIVE_SCAN_UEWORKFLOW=1 to run"]
    fn live_scan_ueworkflow_modules_from_managed_worktrees() {
        if std::env::var("AGENTWATCHER_LIVE_SCAN_UEWORKFLOW")
            .ok()
            .as_deref()
            != Some("1")
        {
            return;
        }
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let agentwatcher_root = manifest_dir
            .parent()
            .expect("src-tauri must live under AgentWatcher");
        let plugin_root = agentwatcher_root.join("Plugins");

        let plugins = scan_plugin_root(&plugin_root).unwrap();
        let plugin = plugins
            .iter()
            .find(|plugin| plugin.descriptor.name == "UEWorkflow")
            .expect("UEWorkflow plugin must be discovered");
        let module_names: Vec<_> = plugin
            .modules
            .iter()
            .map(|module| module.descriptor.name.as_str())
            .collect();

        assert_eq!(module_names, vec!["DevFlow", "AgentHub", "KnowledgeBase"]);
        for module in &plugin.modules {
            assert!(
                identity_path_key(&module.identity_path)
                    .contains("agentwatcher/.plugin-worktrees/ueworkflow"),
                "module {} identity should be a managed worktree: {}",
                module.descriptor.name,
                module.identity_path
            );
            assert!(module.display_path.contains("Modules"));
        }
    }

    fn temp_scan_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "agentwatcher-plugin-scan-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn write_text(path: PathBuf, text: impl AsRef<str>) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text.as_ref()).unwrap();
    }
}
