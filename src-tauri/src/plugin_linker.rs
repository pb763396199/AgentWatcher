use crate::plugin_manifest::{
    module_ref_mount_path, parse_plugin_descriptor, AwModuleLinkType, AwModuleReference,
    AwPluginDescriptor, PLUGIN_DESCRIPTOR_FILE,
};
use serde::Serialize;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;
#[cfg(windows)]
use windows_sys::Win32::Storage::FileSystem::{
    GetFileAttributesW, RemoveDirectoryW, INVALID_FILE_ATTRIBUTES,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwPluginLinkError {
    pub message: String,
}

impl AwPluginLinkError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for AwPluginLinkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AwPluginLinkError {}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AwPluginLinkStatus {
    pub plugin_name: String,
    pub plugin_root: String,
    pub workspace_root: String,
    pub modules: Vec<AwModuleLinkStatus>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AwModuleLinkStatus {
    pub module_name: String,
    pub display_name: String,
    pub mount_path: String,
    pub expected_target: String,
    #[serde(skip_serializing)]
    pub source_target: Option<String>,
    pub worktree_branch: Option<String>,
    pub managed_worktree: bool,
    pub actual_target: Option<String>,
    pub link_type: AwModuleLinkType,
    pub state: AwModuleLinkState,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AwModuleLinkState {
    Ready,
    MissingLink,
    MissingTarget,
    BrokenLink,
    WrongTarget,
    BlockedByExistingPath,
    MissingLinkDeclaration,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AwPluginLinkApplyResult {
    pub plugin_name: String,
    pub changed: bool,
    pub modules: Vec<AwModuleLinkApplyItem>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AwModuleLinkApplyItem {
    pub module_name: String,
    pub before: AwModuleLinkStatus,
    pub after: AwModuleLinkStatus,
    pub action: AwModuleLinkAction,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AwModuleLinkAction {
    AlreadyReady,
    Created,
    Repaired,
    Skipped,
}

pub fn get_plugin_link_status(
    plugin_root: &Path,
    plugin_name: &str,
) -> Result<AwPluginLinkStatus, AwPluginLinkError> {
    let (plugin_dir, descriptor) = read_plugin_descriptor(plugin_root, plugin_name)?;
    let workspace_root = workspace_root_for_plugin_root(plugin_root)?;
    let modules = descriptor
        .modules
        .iter()
        .map(|module_ref| module_link_status(&workspace_root, &plugin_dir, module_ref))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AwPluginLinkStatus {
        plugin_name: descriptor.name,
        plugin_root: path_to_string(&plugin_dir),
        workspace_root: path_to_string(&workspace_root),
        modules,
    })
}

pub fn install_plugin_links(
    plugin_root: &Path,
    plugin_name: &str,
    repair_existing_links: bool,
) -> Result<AwPluginLinkApplyResult, AwPluginLinkError> {
    let (plugin_dir, descriptor) = read_plugin_descriptor(plugin_root, plugin_name)?;
    let workspace_root = workspace_root_for_plugin_root(plugin_root)?;
    let mut changed = false;
    let mut modules = Vec::new();

    for module_ref in &descriptor.modules {
        let mut before = module_link_status(&workspace_root, &plugin_dir, module_ref)?;
        if before.managed_worktree && before.state == AwModuleLinkState::MissingTarget {
            ensure_managed_worktree(&before)?;
            before = module_link_status(&workspace_root, &plugin_dir, module_ref)?;
        }
        let action = apply_module_link(&before, repair_existing_links)?;
        changed |= matches!(
            action,
            AwModuleLinkAction::Created | AwModuleLinkAction::Repaired
        );
        let after = module_link_status(&workspace_root, &plugin_dir, module_ref)?;
        modules.push(AwModuleLinkApplyItem {
            module_name: module_ref.name.clone(),
            before,
            after,
            action,
        });
    }

    Ok(AwPluginLinkApplyResult {
        plugin_name: descriptor.name,
        changed,
        modules,
    })
}

fn read_plugin_descriptor(
    plugin_root: &Path,
    plugin_name: &str,
) -> Result<(PathBuf, AwPluginDescriptor), AwPluginLinkError> {
    validate_name(plugin_name)?;
    let plugin_dir = plugin_root.join(plugin_name);
    let descriptor_path = plugin_dir.join(PLUGIN_DESCRIPTOR_FILE);
    let descriptor_text = fs::read_to_string(&descriptor_path).map_err(|error| {
        AwPluginLinkError::new(format!(
            "failed to read plugin descriptor {}: {}",
            path_to_string(&descriptor_path),
            error
        ))
    })?;
    let descriptor = parse_plugin_descriptor(&descriptor_text)
        .map_err(|error| AwPluginLinkError::new(error.to_string()))?;
    if descriptor.name != plugin_name {
        return Err(AwPluginLinkError::new(format!(
            "plugin descriptor name {} does not match requested plugin {}",
            descriptor.name, plugin_name
        )));
    }
    Ok((plugin_dir, descriptor))
}

fn module_link_status(
    workspace_root: &Path,
    plugin_dir: &Path,
    module_ref: &AwModuleReference,
) -> Result<AwModuleLinkStatus, AwPluginLinkError> {
    let Some(link) = &module_ref.link else {
        let mount_path = module_mount_path(plugin_dir, module_ref)?;
        return Ok(AwModuleLinkStatus {
            module_name: module_ref.name.clone(),
            display_name: module_display_name(module_ref),
            mount_path: path_to_string(&mount_path),
            expected_target: String::new(),
            source_target: None,
            worktree_branch: None,
            managed_worktree: false,
            actual_target: None,
            link_type: AwModuleLinkType::Junction,
            state: AwModuleLinkState::MissingLinkDeclaration,
            message: "模块没有声明 link，不能由 UEWorkflow 正式装配".to_string(),
        });
    };
    if link.link_type != AwModuleLinkType::Junction {
        return Err(AwPluginLinkError::new(format!(
            "module {} declares unsupported link type",
            module_ref.name
        )));
    }

    let mount_path = module_mount_path(plugin_dir, module_ref)?;
    let agentwatcher_root = agentwatcher_root_for_plugin_dir(plugin_dir)?;
    let target = module_link_target(workspace_root, &agentwatcher_root, link)?;
    let expected_target = target.expected_target;
    let canonical_workspace_root = fs::canonicalize(workspace_root).map_err(|error| {
        AwPluginLinkError::new(format!(
            "failed to resolve workspace root {}: {}",
            path_to_string(workspace_root),
            error
        ))
    })?;
    if let Some(source_target) = &target.source_target {
        let canonical_source_target = fs::canonicalize(source_target).map_err(|error| {
            AwPluginLinkError::new(format!(
                "failed to resolve source project {}: {}",
                path_to_string(source_target),
                error
            ))
        })?;
        if !canonical_source_target.starts_with(&canonical_workspace_root) {
            return Err(AwPluginLinkError::new(format!(
                "module {} source project escapes trusted workspace root: {}",
                module_ref.name,
                path_to_string(&canonical_source_target)
            )));
        }
    }
    let canonical_expected_target = match fs::canonicalize(&expected_target) {
        Ok(path) if path.is_dir() => path,
        _ => {
            return Ok(AwModuleLinkStatus {
                module_name: module_ref.name.clone(),
                display_name: module_display_name(module_ref),
                mount_path: path_to_string(&mount_path),
                expected_target: path_to_string(&expected_target),
                source_target: target
                    .source_target
                    .as_ref()
                    .map(|path| path_to_string(path)),
                worktree_branch: target.worktree_branch.clone(),
                managed_worktree: target.managed_worktree,
                actual_target: None,
                link_type: link.link_type.clone(),
                state: AwModuleLinkState::MissingTarget,
                message: if target.managed_worktree {
                    format!("模块 worktree 不存在：{}", path_to_string(&expected_target))
                } else {
                    format!("目标工程不存在：{}", path_to_string(&expected_target))
                },
            });
        }
    };
    if !canonical_expected_target.starts_with(&canonical_workspace_root) {
        return Err(AwPluginLinkError::new(format!(
            "module {} target escapes trusted workspace root: {}",
            module_ref.name,
            path_to_string(&canonical_expected_target)
        )));
    }
    let canonical_agentwatcher_root = fs::canonicalize(&agentwatcher_root).map_err(|error| {
        AwPluginLinkError::new(format!(
            "failed to resolve AgentWatcher root {}: {}",
            path_to_string(&agentwatcher_root),
            error
        ))
    })?;
    if target.managed_worktree
        && !canonical_expected_target.starts_with(
            canonical_agentwatcher_root
                .join(".plugin-worktrees")
                .join("UEWorkflow"),
        )
    {
        return Err(AwPluginLinkError::new(format!(
            "module {} managed worktree target escapes .plugin-worktrees/UEWorkflow: {}",
            module_ref.name,
            path_to_string(&canonical_expected_target)
        )));
    }

    if !path_entry_exists(&mount_path) {
        return Ok(AwModuleLinkStatus {
            module_name: module_ref.name.clone(),
            display_name: module_display_name(module_ref),
            mount_path: path_to_string(&mount_path),
            expected_target: path_to_string(&canonical_expected_target),
            source_target: target
                .source_target
                .as_ref()
                .map(|path| path_to_string(path)),
            worktree_branch: target.worktree_branch.clone(),
            managed_worktree: target.managed_worktree,
            actual_target: None,
            link_type: link.link_type.clone(),
            state: AwModuleLinkState::MissingLink,
            message: "模块链接未安装".to_string(),
        });
    }

    let actual_target = match junction::get_target(&mount_path) {
        Ok(path) => path,
        Err(error) => {
            return Ok(AwModuleLinkStatus {
                module_name: module_ref.name.clone(),
                display_name: module_display_name(module_ref),
                mount_path: path_to_string(&mount_path),
                expected_target: path_to_string(&canonical_expected_target),
                source_target: target
                    .source_target
                    .as_ref()
                    .map(|path| path_to_string(path)),
                worktree_branch: target.worktree_branch.clone(),
                managed_worktree: target.managed_worktree,
                actual_target: None,
                link_type: link.link_type.clone(),
                state: AwModuleLinkState::BlockedByExistingPath,
                message: format!("挂载位置已存在，但不是 AgentWatcher 可管理的 Junction：{error}"),
            });
        }
    };
    let canonical_actual_target = match fs::canonicalize(&actual_target) {
        Ok(path) if path.is_dir() => path,
        _ => {
            return Ok(AwModuleLinkStatus {
                module_name: module_ref.name.clone(),
                display_name: module_display_name(module_ref),
                mount_path: path_to_string(&mount_path),
                expected_target: path_to_string(&canonical_expected_target),
                source_target: target
                    .source_target
                    .as_ref()
                    .map(|path| path_to_string(path)),
                worktree_branch: target.worktree_branch.clone(),
                managed_worktree: target.managed_worktree,
                actual_target: Some(path_to_string(&actual_target)),
                link_type: link.link_type.clone(),
                state: AwModuleLinkState::BrokenLink,
                message: "Junction 目标已不存在".to_string(),
            });
        }
    };

    if path_key(&canonical_actual_target) == path_key(&canonical_expected_target) {
        Ok(AwModuleLinkStatus {
            module_name: module_ref.name.clone(),
            display_name: module_display_name(module_ref),
            mount_path: path_to_string(&mount_path),
            expected_target: path_to_string(&canonical_expected_target),
            source_target: target
                .source_target
                .as_ref()
                .map(|path| path_to_string(path)),
            worktree_branch: target.worktree_branch.clone(),
            managed_worktree: target.managed_worktree,
            actual_target: Some(path_to_string(&canonical_actual_target)),
            link_type: link.link_type.clone(),
            state: AwModuleLinkState::Ready,
            message: "模块链接正常".to_string(),
        })
    } else {
        Ok(AwModuleLinkStatus {
            module_name: module_ref.name.clone(),
            display_name: module_display_name(module_ref),
            mount_path: path_to_string(&mount_path),
            expected_target: path_to_string(&canonical_expected_target),
            source_target: target
                .source_target
                .as_ref()
                .map(|path| path_to_string(path)),
            worktree_branch: target.worktree_branch.clone(),
            managed_worktree: target.managed_worktree,
            actual_target: Some(path_to_string(&canonical_actual_target)),
            link_type: link.link_type.clone(),
            state: AwModuleLinkState::WrongTarget,
            message: "Junction 指向了错误目标".to_string(),
        })
    }
}

struct ModuleLinkTarget {
    expected_target: PathBuf,
    source_target: Option<PathBuf>,
    worktree_branch: Option<String>,
    managed_worktree: bool,
}

fn module_link_target(
    workspace_root: &Path,
    agentwatcher_root: &Path,
    link: &crate::plugin_manifest::AwModuleLinkDescriptor,
) -> Result<ModuleLinkTarget, AwPluginLinkError> {
    if let (Some(source_module), Some(worktree_path), Some(worktree_branch)) = (
        link.source_module
            .as_deref()
            .or(link.source_project.as_deref()),
        link.worktree_path.as_deref(),
        link.worktree_branch.as_deref(),
    ) {
        if !is_safe_relative_path(worktree_path)
            || !worktree_path
                .replace('\\', "/")
                .starts_with(".plugin-worktrees/UEWorkflow/")
        {
            return Err(AwPluginLinkError::new(format!(
                "managed worktree path must stay under .plugin-worktrees/UEWorkflow: {worktree_path}"
            )));
        }
        let source_project = private_source_project_for_module(source_module);
        return Ok(ModuleLinkTarget {
            expected_target: join_relative(agentwatcher_root, worktree_path),
            source_target: Some(workspace_root.join(&source_project)),
            worktree_branch: Some(worktree_branch.to_string()),
            managed_worktree: true,
        });
    }

    let Some(target_project) = link.target_project.as_deref() else {
        return Err(AwPluginLinkError::new(
            "module link must declare targetProject or managed worktree fields",
        ));
    };
    Ok(ModuleLinkTarget {
        expected_target: workspace_root.join(target_project),
        source_target: None,
        worktree_branch: None,
        managed_worktree: false,
    })
}

fn private_source_project_for_module(source_module: &str) -> String {
    match source_module {
        "DevFlow" => legacy_source_name(&["Unreal", "DevFlow"], ""),
        "AgentHub" => legacy_source_name(&["UE", "Master", "Agent"], "_"),
        "KnowledgeBase" => legacy_source_name(&["UE5", "KnowledgeBaseMaker"], "_"),
        legacy => legacy.to_string(),
    }
}

fn legacy_source_name(parts: &[&str], separator: &str) -> String {
    parts.join(separator)
}

fn apply_module_link(
    status: &AwModuleLinkStatus,
    repair_existing_links: bool,
) -> Result<AwModuleLinkAction, AwPluginLinkError> {
    let mount_path = PathBuf::from(&status.mount_path);
    let expected_target = PathBuf::from(&status.expected_target);
    match status.state {
        AwModuleLinkState::Ready => Ok(AwModuleLinkAction::AlreadyReady),
        AwModuleLinkState::MissingTarget if status.managed_worktree => {
            ensure_managed_worktree(status)?;
            create_junction(&expected_target, &mount_path)?;
            Ok(AwModuleLinkAction::Created)
        }
        AwModuleLinkState::MissingLink => {
            create_junction(&expected_target, &mount_path)?;
            Ok(AwModuleLinkAction::Created)
        }
        AwModuleLinkState::BrokenLink | AwModuleLinkState::WrongTarget if repair_existing_links => {
            delete_junction(&mount_path)?;
            create_junction_after_repair(&expected_target, &mount_path)?;
            Ok(AwModuleLinkAction::Repaired)
        }
        AwModuleLinkState::BrokenLink | AwModuleLinkState::WrongTarget => {
            Ok(AwModuleLinkAction::Skipped)
        }
        AwModuleLinkState::MissingTarget => Err(AwPluginLinkError::new(format!(
            "cannot install module {} because target is missing: {}",
            status.module_name, status.expected_target
        ))),
        AwModuleLinkState::BlockedByExistingPath => Err(AwPluginLinkError::new(format!(
            "cannot install module {} because mount path is not a managed Junction: {}",
            status.module_name, status.mount_path
        ))),
        AwModuleLinkState::MissingLinkDeclaration => Err(AwPluginLinkError::new(format!(
            "cannot install module {} because link declaration is missing",
            status.module_name
        ))),
    }
}

fn ensure_managed_worktree(status: &AwModuleLinkStatus) -> Result<(), AwPluginLinkError> {
    if !status.managed_worktree {
        return Ok(());
    }
    let source_target = status.source_target.as_ref().ok_or_else(|| {
        AwPluginLinkError::new(format!(
            "managed module {} does not declare source project",
            status.module_name
        ))
    })?;
    let worktree_branch = status.worktree_branch.as_ref().ok_or_else(|| {
        AwPluginLinkError::new(format!(
            "managed module {} does not declare worktree branch",
            status.module_name
        ))
    })?;
    let source_target = PathBuf::from(source_target);
    let expected_target = PathBuf::from(&status.expected_target);
    if path_entry_exists(&expected_target) {
        return Ok(());
    }
    if let Some(parent) = expected_target.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AwPluginLinkError::new(format!(
                "failed to create managed worktree parent {}: {}",
                path_to_string(parent),
                error
            ))
        })?;
    }
    create_git_worktree(&source_target, &expected_target, worktree_branch)
}

fn create_git_worktree(
    source_target: &Path,
    worktree_path: &Path,
    branch: &str,
) -> Result<(), AwPluginLinkError> {
    let branch_exists = git_branch_exists(source_target, branch)?;
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(source_target)
        .arg("worktree")
        .arg("add");
    if branch_exists {
        command.arg(worktree_path).arg(branch);
    } else {
        command.arg("-b").arg(branch).arg(worktree_path).arg("HEAD");
    }
    run_git_command(command, source_target, "create managed module worktree")
}

fn git_branch_exists(source_target: &Path, branch: &str) -> Result<bool, AwPluginLinkError> {
    let status = Command::new("git")
        .arg("-C")
        .arg(source_target)
        .arg("show-ref")
        .arg("--verify")
        .arg("--quiet")
        .arg(format!("refs/heads/{branch}"))
        .status()
        .map_err(|error| {
            AwPluginLinkError::new(format!(
                "failed to check git branch {} in {}: {}",
                branch,
                path_to_string(source_target),
                error
            ))
        })?;
    Ok(status.success())
}

fn run_git_command(
    mut command: Command,
    source_target: &Path,
    action: &str,
) -> Result<(), AwPluginLinkError> {
    let output = command.output().map_err(|error| {
        AwPluginLinkError::new(format!(
            "failed to run git for {} in {}: {}",
            action,
            path_to_string(source_target),
            error
        ))
    })?;
    if output.status.success() {
        return Ok(());
    }
    Err(AwPluginLinkError::new(format!(
        "git failed to {} in {}: {}\n{}",
        action,
        path_to_string(source_target),
        String::from_utf8_lossy(&output.stderr).trim(),
        String::from_utf8_lossy(&output.stdout).trim()
    )))
}

fn create_junction(target: &Path, mount_path: &Path) -> Result<(), AwPluginLinkError> {
    if let Some(parent) = mount_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            AwPluginLinkError::new(format!(
                "failed to create module link parent {}: {}",
                path_to_string(parent),
                error
            ))
        })?;
    }
    junction::create(target, mount_path).map_err(|error| {
        AwPluginLinkError::new(format!(
            "failed to create Junction {} -> {}: {}",
            path_to_string(mount_path),
            path_to_string(target),
            error
        ))
    })
}

fn create_junction_after_repair(target: &Path, mount_path: &Path) -> Result<(), AwPluginLinkError> {
    let mut last_error = None;
    for attempt in 0..8 {
        match junction::create(target, mount_path) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                last_error = Some(error);
                let _ = remove_junction_mount_dir(mount_path);
                thread::sleep(Duration::from_millis(25 * (attempt + 1)));
            }
            Err(error) => {
                return Err(AwPluginLinkError::new(format!(
                    "failed to create Junction {} -> {}: {}",
                    path_to_string(mount_path),
                    path_to_string(target),
                    error
                )));
            }
        }
    }
    Err(AwPluginLinkError::new(format!(
        "failed to create Junction {} -> {} after repairing stale mount path: {}",
        path_to_string(mount_path),
        path_to_string(target),
        last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "mount path still exists".to_string())
    )))
}

fn delete_junction(mount_path: &Path) -> Result<(), AwPluginLinkError> {
    remove_junction_mount_dir(mount_path)?;
    if path_entry_exists(mount_path) {
        return Err(AwPluginLinkError::new(format!(
            "failed to remove Junction directory {}: path still exists after deletion",
            path_to_string(mount_path)
        )));
    }
    Ok(())
}

fn path_entry_exists(path: &Path) -> bool {
    path_entry_exists_impl(path)
}

#[cfg(windows)]
fn remove_junction_mount_dir(mount_path: &Path) -> Result<(), AwPluginLinkError> {
    let wide_path = wide_null_path(mount_path);
    let removed = unsafe { RemoveDirectoryW(wide_path.as_ptr()) };
    if removed != 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == ErrorKind::NotFound {
        return Ok(());
    }
    Err(AwPluginLinkError::new(format!(
        "failed to remove Junction directory {}: {}",
        path_to_string(mount_path),
        error
    )))
}

#[cfg(windows)]
fn path_entry_exists_impl(path: &Path) -> bool {
    let wide_path = wide_null_path(path);
    unsafe { GetFileAttributesW(wide_path.as_ptr()) != INVALID_FILE_ATTRIBUTES }
}

#[cfg(windows)]
fn wide_null_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(not(windows))]
fn remove_junction_mount_dir(mount_path: &Path) -> Result<(), AwPluginLinkError> {
    match fs::remove_dir(mount_path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(AwPluginLinkError::new(format!(
                "failed to remove Junction directory {}: {}",
                path_to_string(mount_path),
                error
            )));
        }
    }
    Ok(())
}

#[cfg(not(windows))]
fn path_entry_exists_impl(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn module_mount_path(
    plugin_dir: &Path,
    module_ref: &AwModuleReference,
) -> Result<PathBuf, AwPluginLinkError> {
    let mount_path = module_ref_mount_path(module_ref).ok_or_else(|| {
        AwPluginLinkError::new(format!(
            "module {} does not declare mountPath or manifestPath",
            module_ref.name
        ))
    })?;
    if !is_safe_relative_path(&mount_path) {
        return Err(AwPluginLinkError::new(format!(
            "module {} has unsafe mount path {}",
            module_ref.name, mount_path
        )));
    }
    Ok(join_relative(plugin_dir, &mount_path))
}

fn module_display_name(module_ref: &AwModuleReference) -> String {
    module_ref
        .display_name
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(&module_ref.name)
        .to_string()
}

fn workspace_root_for_plugin_root(plugin_root: &Path) -> Result<PathBuf, AwPluginLinkError> {
    plugin_root
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            AwPluginLinkError::new(format!(
                "cannot derive workspace root from plugin root {}",
                path_to_string(plugin_root)
            ))
        })
}

fn agentwatcher_root_for_plugin_dir(plugin_dir: &Path) -> Result<PathBuf, AwPluginLinkError> {
    plugin_dir
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            AwPluginLinkError::new(format!(
                "cannot derive AgentWatcher root from plugin dir {}",
                path_to_string(plugin_dir)
            ))
        })
}

fn validate_name(value: &str) -> Result<(), AwPluginLinkError> {
    let valid = !value.is_empty()
        && value.len() <= 96
        && value
            .chars()
            .next()
            .map(|ch| ch.is_ascii_alphanumeric())
            .unwrap_or(false)
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'));
    if valid {
        Ok(())
    } else {
        Err(AwPluginLinkError::new(format!(
            "invalid AgentWatcher plugin or module name: {value}"
        )))
    }
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

fn join_relative(root: &Path, relative: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for segment in relative.replace('\\', "/").split('/') {
        path.push(segment);
    }
    path
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase()
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_manifest::{scan_plugin_root, MODULE_DESCRIPTOR_FILE};
    use std::time::{SystemTime, UNIX_EPOCH};

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
      "runner": { "type": "cargoRun", "args": ["aw-status"] },
      "timeoutMs": 60000
    }
  ]
}"#;

    #[test]
    fn reports_missing_link_until_installed() {
        let temp = temp_root("missing");
        let plugin_root = write_plugin_fixture(&temp);

        let status = get_plugin_link_status(&plugin_root, "UEWorkflow").unwrap();

        assert_eq!(status.modules[0].state, AwModuleLinkState::MissingLink);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn link_status_json_hides_private_source_target() {
        let status = AwModuleLinkStatus {
            module_name: "DevFlow".to_string(),
            display_name: "开发流".to_string(),
            mount_path: "Modules/DevFlow".to_string(),
            expected_target: ".plugin-worktrees/UEWorkflow/DevFlow".to_string(),
            source_target: Some(format!(
                "X:/Workspace/{}",
                private_source_project_for_module("DevFlow")
            )),
            worktree_branch: Some("agentwatcher/ueworkflow/devflow".to_string()),
            managed_worktree: true,
            actual_target: None,
            link_type: AwModuleLinkType::Junction,
            state: AwModuleLinkState::MissingLink,
            message: "模块链接未安装".to_string(),
        };

        let json = serde_json::to_string(&status).unwrap();

        assert!(!json.contains("sourceTarget"));
        assert!(!json.contains(&private_source_project_for_module("DevFlow")));
        assert!(json.contains("DevFlow"));
    }

    #[test]
    fn creates_missing_junction_and_reports_ready() {
        let temp = temp_root("create");
        let plugin_root = write_plugin_fixture(&temp);

        let result = install_plugin_links(&plugin_root, "UEWorkflow", false).unwrap();
        let status = get_plugin_link_status(&plugin_root, "UEWorkflow").unwrap();

        assert!(result.changed);
        assert_eq!(result.modules[0].action, AwModuleLinkAction::Created);
        assert_eq!(status.modules[0].display_name, "开发流");
        assert_eq!(status.modules[0].state, AwModuleLinkState::Ready);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn installed_junction_can_be_scanned_as_real_module() {
        let temp = temp_root("scan");
        let plugin_root = write_plugin_fixture(&temp);
        write_text(
            temp.join("DevFlow").join(MODULE_DESCRIPTOR_FILE),
            VALID_MODULE,
        );

        install_plugin_links(&plugin_root, "UEWorkflow", false).unwrap();
        let plugins = scan_plugin_root(&plugin_root).unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].modules.len(), 1);
        assert_eq!(plugins[0].modules[0].descriptor.name, "DevFlow");
        assert_eq!(
            path_key(Path::new(&plugins[0].modules[0].identity_path)),
            path_key(&fs::canonicalize(temp.join("DevFlow")).unwrap())
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn creates_managed_worktree_before_junction() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let temp = temp_root("managed-worktree");
        let plugin_root = write_managed_plugin_fixture(&temp);
        let expected_worktree = temp
            .join("AgentWatcher")
            .join(".plugin-worktrees")
            .join("UEWorkflow")
            .join("DevFlow");

        let result = install_plugin_links(&plugin_root, "UEWorkflow", false).unwrap();
        let status = get_plugin_link_status(&plugin_root, "UEWorkflow").unwrap();

        assert!(result.changed);
        assert_eq!(result.modules[0].action, AwModuleLinkAction::Created);
        assert_eq!(status.modules[0].state, AwModuleLinkState::Ready);
        assert!(status.modules[0].managed_worktree);
        assert_eq!(
            status.modules[0].worktree_branch.as_deref(),
            Some("agentwatcher/ueworkflow/devflow")
        );
        assert_eq!(
            path_key(Path::new(&status.modules[0].expected_target)),
            path_key(&fs::canonicalize(&expected_worktree).unwrap())
        );
        let branch = git_stdout(&expected_worktree, &["branch", "--show-current"]);
        assert_eq!(branch.trim(), "agentwatcher/ueworkflow/devflow");
        let _ = Command::new("git")
            .arg("-C")
            .arg(temp.join(private_source_project_for_module("DevFlow")))
            .arg("worktree")
            .arg("remove")
            .arg("--force")
            .arg(&expected_worktree)
            .status();
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn blocks_regular_directory_at_mount_path() {
        let temp = temp_root("blocked");
        let plugin_root = write_plugin_fixture(&temp);
        fs::create_dir_all(
            plugin_root
                .join("UEWorkflow")
                .join("Modules")
                .join("DevFlow"),
        )
        .unwrap();

        let status = get_plugin_link_status(&plugin_root, "UEWorkflow").unwrap();
        let error = install_plugin_links(&plugin_root, "UEWorkflow", true).unwrap_err();

        assert_eq!(
            status.modules[0].state,
            AwModuleLinkState::BlockedByExistingPath
        );
        assert!(error.message.contains("not a managed Junction"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn repairs_wrong_target_when_repair_is_enabled() {
        let temp = temp_root("repair");
        let plugin_root = write_plugin_fixture(&temp);
        let wrong = temp.join("WrongTarget");
        fs::create_dir_all(&wrong).unwrap();
        let mount = plugin_root
            .join("UEWorkflow")
            .join("Modules")
            .join("DevFlow");
        if let Some(parent) = mount.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        junction::create(&wrong, &mount).unwrap();

        let before = get_plugin_link_status(&plugin_root, "UEWorkflow").unwrap();
        let result = install_plugin_links(&plugin_root, "UEWorkflow", true).unwrap();
        let after = get_plugin_link_status(&plugin_root, "UEWorkflow").unwrap();

        assert_eq!(before.modules[0].state, AwModuleLinkState::WrongTarget);
        assert_eq!(result.modules[0].action, AwModuleLinkAction::Repaired);
        assert_eq!(after.modules[0].state, AwModuleLinkState::Ready);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn repairs_broken_junction_when_repair_is_enabled() {
        let temp = temp_root("broken");
        let plugin_root = write_plugin_fixture(&temp);
        let broken_target = temp.join("DeletedTarget");
        fs::create_dir_all(&broken_target).unwrap();
        let mount = plugin_root
            .join("UEWorkflow")
            .join("Modules")
            .join("DevFlow");
        if let Some(parent) = mount.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        junction::create(&broken_target, &mount).unwrap();
        fs::remove_dir_all(&broken_target).unwrap();

        let before = get_plugin_link_status(&plugin_root, "UEWorkflow").unwrap();
        let result = install_plugin_links(&plugin_root, "UEWorkflow", true).unwrap();
        let after = get_plugin_link_status(&plugin_root, "UEWorkflow").unwrap();

        assert_eq!(before.modules[0].state, AwModuleLinkState::BrokenLink);
        assert_eq!(result.modules[0].action, AwModuleLinkAction::Repaired);
        assert_eq!(after.modules[0].state, AwModuleLinkState::Ready);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    #[ignore = "live workspace installer; set AGENTWATCHER_INSTALL_UEWORKFLOW_LINKS=1 to run"]
    fn live_install_ueworkflow_links_when_requested() {
        if std::env::var("AGENTWATCHER_INSTALL_UEWORKFLOW_LINKS")
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

        let result = install_plugin_links(&plugin_root, "UEWorkflow", true).unwrap();
        let status = get_plugin_link_status(&plugin_root, "UEWorkflow").unwrap();

        assert_eq!(result.modules.len(), 3);
        assert!(status
            .modules
            .iter()
            .all(|module| module.state == AwModuleLinkState::Ready));
        for module in &status.modules {
            assert!(module.managed_worktree);
            assert!(path_key(Path::new(&module.expected_target))
                .contains("agentwatcher/.plugin-worktrees/ueworkflow"));
            assert_eq!(
                module
                    .actual_target
                    .as_ref()
                    .map(|value| path_key(Path::new(value))),
                Some(path_key(Path::new(&module.expected_target)))
            );
        }
    }

    fn write_plugin_fixture(root: &Path) -> PathBuf {
        let agentwatcher = root.join("AgentWatcher");
        let plugin_root = agentwatcher.join("Plugins");
        let plugin_dir = plugin_root.join("UEWorkflow");
        let target = root.join("DevFlow");
        fs::create_dir_all(&plugin_dir).unwrap();
        fs::create_dir_all(&target).unwrap();
        fs::write(
            plugin_dir.join(PLUGIN_DESCRIPTOR_FILE),
            r#"{
  "schemaVersion": 1,
  "kind": "AgentWatcherPlugin",
  "name": "UEWorkflow",
  "displayName": "虚幻项目开发工作流",
  "modules": [
    {
      "name": "DevFlow",
      "displayName": "开发流",
      "mountPath": "Modules/DevFlow",
      "link": {
        "targetProject": "DevFlow",
        "linkType": "junction",
        "required": true
      }
    }
  ]
}"#,
        )
        .unwrap();
        plugin_root
    }

    fn write_managed_plugin_fixture(root: &Path) -> PathBuf {
        let agentwatcher = root.join("AgentWatcher");
        let plugin_root = agentwatcher.join("Plugins");
        let plugin_dir = plugin_root.join("UEWorkflow");
        let source = root.join(private_source_project_for_module("DevFlow"));
        fs::create_dir_all(&plugin_dir).unwrap();
        init_git_source_project(&source);
        fs::write(
            plugin_dir.join(PLUGIN_DESCRIPTOR_FILE),
            r#"{
  "schemaVersion": 1,
  "kind": "AgentWatcherPlugin",
  "name": "UEWorkflow",
  "displayName": "虚幻项目开发工作流",
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
    }
  ]
}"#,
        )
        .unwrap();
        plugin_root
    }

    fn init_git_source_project(source: &Path) {
        fs::create_dir_all(source).unwrap();
        git(source, &["init"]);
        git(
            source,
            &["config", "user.email", "agentwatcher@example.local"],
        );
        git(source, &["config", "user.name", "AgentWatcher Test"]);
        write_text(source.join("README.md"), "test source\n");
        git(source, &["add", "."]);
        git(source, &["commit", "-m", "init"]);
    }

    fn git(cwd: &Path, args: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .status()
            .unwrap();
        assert!(
            status.success(),
            "git {:?} failed in {}",
            args,
            path_to_string(cwd)
        );
    }

    fn git_stdout(cwd: &Path, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {:?} failed in {}",
            args,
            path_to_string(cwd)
        );
        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn write_text(path: PathBuf, text: impl AsRef<str>) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, text.as_ref()).unwrap();
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "agentwatcher-plugin-linker-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
