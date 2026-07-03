use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub const UWF_MASTER_CONFIRMATION: &str = "UEWorkflow.UnrealMaster.execute.v1";
pub const UWF_DEVFLOW_SWITCH_CONFIRMATION: &str = "UEWorkflow.DevFlow.switch.v1";
pub const UWF_DEVFLOW_BUILD_CHECK_CONFIRMATION: &str = "UEWorkflow.DevFlow.build-check.v1";
pub const UWF_AGENTHUB_PROVIDER_STATUS_CONFIRMATION: &str =
    "UEWorkflow.AgentHub.provider-status.v1";
pub const UWF_AGENTHUB_VALIDATE_CONTRACT_CONFIRMATION: &str =
    "UEWorkflow.AgentHub.validate-contract.v1";
pub const UWF_AGENTHUB_PACKAGE_COPILOT_CONFIRMATION: &str =
    "UEWorkflow.AgentHub.package-copilot.v1";
pub const UWF_AGENTHUB_PACKAGE_CODEX_CONFIRMATION: &str = "UEWorkflow.AgentHub.package-codex.v1";
pub const UWF_AGENTHUB_PACKAGE_CLAUDE_CONFIRMATION: &str = "UEWorkflow.AgentHub.package-claude.v1";
pub const UWF_AGENTHUB_PACKAGE_OPENCODE_CONFIRMATION: &str =
    "UEWorkflow.AgentHub.package-opencode.v1";
pub const UWF_AGENTHUB_INSTALL_PROVIDER_CONFIRMATION: &str =
    "UEWorkflow.AgentHub.install-provider.v1";
pub const UWF_KNOWLEDGEBASE_QUERY_CONFIRMATION: &str = "UEWorkflow.KnowledgeBase.query.v1";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwUwfTaskRequest {
    pub goal: String,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub host_root: Option<String>,
    #[serde(default)]
    pub main_project: Option<String>,
    #[serde(default)]
    pub primary_path: Option<String>,
    #[serde(default)]
    pub plugin_dependencies: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub confirmed: Option<bool>,
    #[serde(default)]
    pub dry_run_accepted: Option<bool>,
    #[serde(default)]
    pub confirmation_boundary: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwUwfExternalCommandRequest {
    pub command_id: String,
    #[serde(default)]
    pub goal: Option<String>,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub host_root: Option<String>,
    #[serde(default)]
    pub main_project: Option<String>,
    #[serde(default)]
    pub primary_path: Option<String>,
    #[serde(default)]
    pub plugin_dependencies: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub confirmed: Option<bool>,
    #[serde(default)]
    pub dry_run_accepted: Option<bool>,
    #[serde(default)]
    pub confirmation_boundary: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwUwfCommandResponse {
    pub command: Vec<String>,
    pub result: Value,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwUwfExternalCommandPolicyView {
    pub command_id: String,
    pub top_level: Option<String>,
    pub module: Option<String>,
    pub command: Option<String>,
    pub action: Option<String>,
    pub requires_confirmation: bool,
    pub confirmation_boundary: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct ExternalCommandPolicy {
    id: &'static str,
    top_level: Option<&'static str>,
    module: Option<&'static str>,
    command: Option<&'static str>,
    action: Option<&'static str>,
    confirmation: Option<&'static str>,
}

const EXTERNAL_COMMAND_POLICIES: &[ExternalCommandPolicy] = &[
    top_level_policy("doctor", "doctor"),
    top_level_policy("modules", "modules"),
    module_policy("dev.status", "dev", "status"),
    module_policy("dev.capabilities", "dev", "capabilities"),
    module_policy("dev.commands", "dev", "commands"),
    module_policy("dev.schema", "dev", "schema"),
    module_policy("dev.history", "dev", "history"),
    module_policy("dev.artifacts", "dev", "artifacts"),
    action_policy("dev.switch.dryRun", "dev", "dry-run", "switch", None),
    action_policy(
        "dev.switch.execute",
        "dev",
        "execute",
        "switch",
        Some(UWF_DEVFLOW_SWITCH_CONFIRMATION),
    ),
    action_policy(
        "dev.buildCheck.dryRun",
        "dev",
        "dry-run",
        "build-check",
        None,
    ),
    action_policy(
        "dev.buildCheck.execute",
        "dev",
        "execute",
        "build-check",
        Some(UWF_DEVFLOW_BUILD_CHECK_CONFIRMATION),
    ),
    module_policy("agents.status", "agents", "status"),
    module_policy("agents.capabilities", "agents", "capabilities"),
    module_policy("agents.commands", "agents", "commands"),
    module_policy("agents.schema", "agents", "schema"),
    module_policy("agents.history", "agents", "history"),
    module_policy("agents.artifacts", "agents", "artifacts"),
    action_policy(
        "agents.providerStatus.dryRun",
        "agents",
        "dry-run",
        "provider-status",
        None,
    ),
    action_policy(
        "agents.providerStatus.execute",
        "agents",
        "execute",
        "provider-status",
        Some(UWF_AGENTHUB_PROVIDER_STATUS_CONFIRMATION),
    ),
    action_policy(
        "agents.verify.dryRun",
        "agents",
        "dry-run",
        "validate-contract",
        None,
    ),
    action_policy(
        "agents.verify.execute",
        "agents",
        "execute",
        "validate-contract",
        Some(UWF_AGENTHUB_VALIDATE_CONTRACT_CONFIRMATION),
    ),
    action_policy(
        "agents.package.copilot.dryRun",
        "agents",
        "dry-run",
        "package-copilot",
        None,
    ),
    action_policy(
        "agents.package.copilot.execute",
        "agents",
        "execute",
        "package-copilot",
        Some(UWF_AGENTHUB_PACKAGE_COPILOT_CONFIRMATION),
    ),
    action_policy(
        "agents.package.codex.dryRun",
        "agents",
        "dry-run",
        "package-codex",
        None,
    ),
    action_policy(
        "agents.package.codex.execute",
        "agents",
        "execute",
        "package-codex",
        Some(UWF_AGENTHUB_PACKAGE_CODEX_CONFIRMATION),
    ),
    action_policy(
        "agents.package.claude.dryRun",
        "agents",
        "dry-run",
        "package-claude",
        None,
    ),
    action_policy(
        "agents.package.claude.execute",
        "agents",
        "execute",
        "package-claude",
        Some(UWF_AGENTHUB_PACKAGE_CLAUDE_CONFIRMATION),
    ),
    action_policy(
        "agents.package.opencode.dryRun",
        "agents",
        "dry-run",
        "package-opencode",
        None,
    ),
    action_policy(
        "agents.package.opencode.execute",
        "agents",
        "execute",
        "package-opencode",
        Some(UWF_AGENTHUB_PACKAGE_OPENCODE_CONFIRMATION),
    ),
    action_policy(
        "agents.install.dryRun",
        "agents",
        "dry-run",
        "install-provider",
        None,
    ),
    action_policy(
        "agents.install.execute",
        "agents",
        "execute",
        "install-provider",
        Some(UWF_AGENTHUB_INSTALL_PROVIDER_CONFIRMATION),
    ),
    module_policy("kb.status", "kb", "status"),
    module_policy("kb.capabilities", "kb", "capabilities"),
    module_policy("kb.commands", "kb", "commands"),
    module_policy("kb.schema", "kb", "schema"),
    module_policy("kb.history", "kb", "history"),
    module_policy("kb.artifacts", "kb", "artifacts"),
    action_policy("kb.query.dryRun", "kb", "dry-run", "query", None),
    action_policy(
        "kb.query.execute",
        "kb",
        "execute",
        "query",
        Some(UWF_KNOWLEDGEBASE_QUERY_CONFIRMATION),
    ),
];

const fn top_level_policy(id: &'static str, command: &'static str) -> ExternalCommandPolicy {
    ExternalCommandPolicy {
        id,
        top_level: Some(command),
        module: None,
        command: None,
        action: None,
        confirmation: None,
    }
}

const fn module_policy(
    id: &'static str,
    module: &'static str,
    command: &'static str,
) -> ExternalCommandPolicy {
    ExternalCommandPolicy {
        id,
        top_level: None,
        module: Some(module),
        command: Some(command),
        action: None,
        confirmation: None,
    }
}

const fn action_policy(
    id: &'static str,
    module: &'static str,
    command: &'static str,
    action: &'static str,
    confirmation: Option<&'static str>,
) -> ExternalCommandPolicy {
    ExternalCommandPolicy {
        id,
        top_level: None,
        module: Some(module),
        command: Some(command),
        action: Some(action),
        confirmation,
    }
}

pub fn run_doctor() -> Result<AwUwfCommandResponse, String> {
    run_uwf_json(&["doctor".to_string(), "--json".to_string()], None)
}

pub fn run_modules() -> Result<AwUwfCommandResponse, String> {
    run_uwf_json(&["modules".to_string(), "--json".to_string()], None)
}

pub fn run_module_status(module: &str) -> Result<AwUwfCommandResponse, String> {
    let module = normalize_module_group(module)?;
    run_uwf_json(&[module, "status".to_string(), "--json".to_string()], None)
}

pub fn run_master_dry_run(request: &AwUwfTaskRequest) -> Result<AwUwfCommandResponse, String> {
    validate_task_request(request)?;
    run_uwf_json(&master_args("dry-run", request, None)?, None)
}

pub fn run_master_execute(
    request: &AwUwfTaskRequest,
    knowledge_root: &Path,
) -> Result<AwUwfCommandResponse, String> {
    validate_task_request(request)?;
    if request.confirmed != Some(true)
        || request.dry_run_accepted != Some(true)
        || request.confirmation_boundary.as_deref() != Some(UWF_MASTER_CONFIRMATION)
    {
        return Err(format!(
            "UEWorkflow execute requires confirmed=true, dryRunAccepted=true, and confirmationBoundary={UWF_MASTER_CONFIRMATION}"
        ));
    }
    run_uwf_json(
        &master_args("execute", request, Some(UWF_MASTER_CONFIRMATION))?,
        Some(knowledge_root),
    )
}

pub fn run_external_command(
    request: &AwUwfExternalCommandRequest,
) -> Result<AwUwfCommandResponse, String> {
    let args = external_command_args(request)?;
    run_uwf_json(&args, None)
}

pub fn external_command_policy_views() -> Vec<AwUwfExternalCommandPolicyView> {
    EXTERNAL_COMMAND_POLICIES
        .iter()
        .map(|policy| AwUwfExternalCommandPolicyView {
            command_id: policy.id.to_string(),
            top_level: policy.top_level.map(str::to_string),
            module: policy.module.map(str::to_string),
            command: policy.command.map(str::to_string),
            action: policy.action.map(str::to_string),
            requires_confirmation: policy.confirmation.is_some(),
            confirmation_boundary: policy.confirmation.map(str::to_string),
        })
        .collect()
}

fn run_uwf_json(
    args: &[String],
    knowledge_root: Option<&Path>,
) -> Result<AwUwfCommandResponse, String> {
    let agentwatcher_root = agentwatcher_root();
    let uwf = resolve_uwf_executable_from_agentwatcher_root(&agentwatcher_root)?;
    let mut process = Command::new(&uwf);
    process.args(args).current_dir(&agentwatcher_root);
    if let Some(knowledge_root) = knowledge_root {
        process.env("UWF_KNOWLEDGE_ROOT", knowledge_root);
    }
    #[cfg(target_os = "windows")]
    {
        process.creation_flags(CREATE_NO_WINDOW);
    }
    let output = process
        .output()
        .map_err(|error| format!("Failed to run uwf: {error}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stdout.len() > 512 * 1024 {
        return Err("uwf output exceeded AgentWatcher limit".to_string());
    }
    let value: Value = serde_json::from_str(&stdout).map_err(|error| {
        format!(
            "uwf did not return valid JSON: {error}; stderr={}",
            if stderr.is_empty() {
                "<empty>"
            } else {
                &stderr
            }
        )
    })?;
    if !output.status.success() {
        let message = value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("uwf command failed");
        return Err(format!("uwf command failed: {message}"));
    }
    Ok(AwUwfCommandResponse {
        command: args.to_vec(),
        result: value,
    })
}

fn external_command_args(request: &AwUwfExternalCommandRequest) -> Result<Vec<String>, String> {
    let command_id = clean_command_id(&request.command_id)?;
    let policy = external_command_policy(&command_id)?;
    let mut args = match (
        policy.top_level,
        policy.module,
        policy.command,
        policy.action,
        policy.confirmation,
    ) {
        (Some(command), None, None, None, None) => vec![command.to_string()],
        (None, Some(module), Some(command), None, None) => module_command_args(module, command),
        (None, Some(module), Some(command), Some(action), None) => {
            action_command_args(module, command, action, request)?
        }
        (None, Some(module), Some(command), Some(action), Some(confirmation)) => {
            confirmed_action_command_args(module, command, action, confirmation, request)?
        }
        _ => return Err(format!("Invalid UEWorkflow command policy: {}", policy.id)),
    };
    args.push("--json".to_string());
    Ok(args)
}

fn external_command_policy(command_id: &str) -> Result<ExternalCommandPolicy, String> {
    EXTERNAL_COMMAND_POLICIES
        .iter()
        .copied()
        .find(|policy| policy.id == command_id)
        .ok_or_else(|| {
            format!(
                "Unsupported UEWorkflow command id: {command_id}; AgentWatcher can only call declared uwf commands"
            )
        })
}

fn module_command_args(module: &str, command: &str) -> Vec<String> {
    vec![module.to_string(), command.to_string()]
}

fn action_command_args(
    module: &str,
    command: &str,
    action: &str,
    request: &AwUwfExternalCommandRequest,
) -> Result<Vec<String>, String> {
    let mut args = vec![
        module.to_string(),
        command.to_string(),
        "--action".to_string(),
        action.to_string(),
    ];
    push_external_context_options(&mut args, request)?;
    Ok(args)
}

fn confirmed_action_command_args(
    module: &str,
    command: &str,
    action: &str,
    confirmation: &str,
    request: &AwUwfExternalCommandRequest,
) -> Result<Vec<String>, String> {
    if request.confirmed != Some(true)
        || request.dry_run_accepted != Some(true)
        || request.confirmation_boundary.as_deref() != Some(confirmation)
    {
        return Err(format!(
            "UEWorkflow command {module}.{action} requires confirmed=true, dryRunAccepted=true, and confirmationBoundary={confirmation}"
        ));
    }
    let mut args = action_command_args(module, command, action, request)?;
    args.push("--confirm".to_string());
    args.push(confirmation.to_string());
    Ok(args)
}

fn push_external_context_options(
    args: &mut Vec<String>,
    request: &AwUwfExternalCommandRequest,
) -> Result<(), String> {
    push_option(args, "--goal", request.goal.as_deref(), 2000)?;
    push_option(args, "--workspace", request.workspace.as_deref(), 128)?;
    push_option(args, "--task-id", request.task_id.as_deref(), 128)?;
    push_option(args, "--provider", request.provider.as_deref(), 64)?;
    push_option(args, "--project", request.project.as_deref(), 128)?;
    push_option(args, "--primary", request.primary.as_deref(), 128)?;
    push_option(args, "--host-root", request.host_root.as_deref(), 260)?;
    push_option(args, "--main-project", request.main_project.as_deref(), 260)?;
    push_option(args, "--primary-path", request.primary_path.as_deref(), 260)?;
    push_option(
        args,
        "--plugin-deps",
        request.plugin_dependencies.as_deref(),
        1000,
    )?;
    push_option(args, "--scope", request.scope.as_deref(), 64)?;
    if let Some(provider) = request.provider.as_deref() {
        validate_provider(provider)?;
    }
    Ok(())
}

fn master_args(
    command: &str,
    request: &AwUwfTaskRequest,
    confirmation: Option<&str>,
) -> Result<Vec<String>, String> {
    let mut args = vec![
        "master".to_string(),
        command.to_string(),
        "--goal".to_string(),
        clean_arg("goal", &request.goal, 2000)?,
    ];
    push_option(&mut args, "--workspace", request.workspace.as_deref(), 128)?;
    push_option(&mut args, "--task-id", request.task_id.as_deref(), 128)?;
    push_option(&mut args, "--provider", request.provider.as_deref(), 64)?;
    push_option(&mut args, "--project", request.project.as_deref(), 128)?;
    push_option(&mut args, "--primary", request.primary.as_deref(), 128)?;
    push_option(&mut args, "--host-root", request.host_root.as_deref(), 260)?;
    push_option(
        &mut args,
        "--main-project",
        request.main_project.as_deref(),
        260,
    )?;
    push_option(
        &mut args,
        "--primary-path",
        request.primary_path.as_deref(),
        260,
    )?;
    push_option(
        &mut args,
        "--plugin-deps",
        request.plugin_dependencies.as_deref(),
        1000,
    )?;
    push_option(&mut args, "--scope", request.scope.as_deref(), 64)?;
    if let Some(confirmation) = confirmation {
        args.push("--confirm".to_string());
        args.push(confirmation.to_string());
    }
    args.push("--json".to_string());
    Ok(args)
}

fn push_option(
    args: &mut Vec<String>,
    flag: &str,
    value: Option<&str>,
    max_len: usize,
) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    args.push(flag.to_string());
    args.push(clean_arg(flag, value, max_len)?);
    Ok(())
}

fn clean_arg(label: &str, value: &str, max_len: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!("{label} must not be empty"));
    }
    if value.chars().count() > max_len {
        return Err(format!("{label} is too long"));
    }
    if value.chars().any(|ch| ch.is_control()) {
        return Err(format!("{label} must not contain control characters"));
    }
    Ok(value.to_string())
}

fn validate_task_request(request: &AwUwfTaskRequest) -> Result<(), String> {
    clean_arg("goal", &request.goal, 2000)?;
    if let Some(provider) = request.provider.as_deref() {
        validate_provider(provider)?;
    }
    Ok(())
}

fn validate_provider(provider: &str) -> Result<(), String> {
    match provider {
        "copilot" | "codex" | "claude" | "claude-code" | "opencode" => Ok(()),
        _ => Err(format!("Unsupported UEWorkflow provider: {provider}")),
    }
}

fn clean_command_id(command_id: &str) -> Result<String, String> {
    let command_id = command_id.trim();
    if command_id.is_empty() {
        return Err("UEWorkflow command id must not be empty".to_string());
    }
    if command_id.len() > 96
        || !command_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.')
    {
        return Err("UEWorkflow command id is invalid".to_string());
    }
    Ok(command_id.to_string())
}

fn normalize_module_group(module: &str) -> Result<String, String> {
    match module {
        "dev" | "devflow" | "DevFlow" | "开发流" => Ok("dev".to_string()),
        "agents" | "agenthub" | "AgentHub" | "智能体" => Ok("agents".to_string()),
        "kb" | "knowledgebase" | "KnowledgeBase" | "知识库" => Ok("kb".to_string()),
        _ => Err(format!("Unsupported UEWorkflow module: {module}")),
    }
}

fn agentwatcher_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn resolve_uwf_executable_from_agentwatcher_root(root: &Path) -> Result<PathBuf, String> {
    let candidates = [
        root.join("Plugins")
            .join("UEWorkflow")
            .join("target")
            .join("debug")
            .join(exe_name("uwf")),
        root.join("Plugins")
            .join("UEWorkflow")
            .join("target")
            .join("release")
            .join(exe_name("uwf")),
        root.join("Plugins")
            .join("UEWorkflow")
            .join("Source")
            .join("Cli")
            .join("target")
            .join("debug")
            .join(exe_name("uwf")),
    ];
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            format!(
                "uwf executable not found under {}; build Plugins/UEWorkflow first",
                root.display()
            )
        })
}

fn exe_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn master_args_are_whitelisted_and_json_only() {
        let request = AwUwfTaskRequest {
            goal: "修复插件任务".to_string(),
            workspace: Some("neon-dev1".to_string()),
            task_id: Some("task-001".to_string()),
            provider: Some("copilot".to_string()),
            project: Some("Neon".to_string()),
            primary: Some("AesWorld".to_string()),
            host_root: Some("F:\\ShanghaiP4\\neon\\Hosts".to_string()),
            main_project: Some("F:\\ShanghaiP4\\neon\\UGA\\DEV_1".to_string()),
            primary_path: Some("F:\\ShanghaiP4\\neon\\Plugins\\AesWorld".to_string()),
            plugin_dependencies: Some("GeometryProcessing,ProceduralMeshComponent".to_string()),
            scope: Some("project_plugin".to_string()),
            confirmed: None,
            dry_run_accepted: None,
            confirmation_boundary: None,
            ..Default::default()
        };
        let args = master_args("dry-run", &request, None).unwrap();
        assert_eq!(args[0], "master");
        assert_eq!(args[1], "dry-run");
        assert!(args.contains(&"--json".to_string()));
        assert!(args.contains(&"--host-root".to_string()));
        assert!(args.contains(&"--main-project".to_string()));
        assert!(args.contains(&"--primary-path".to_string()));
        assert!(args.contains(&"--plugin-deps".to_string()));
        assert!(!args.iter().any(|arg| arg == "shell" || arg == "cmd"));
    }

    #[test]
    fn user_goal_is_a_value_not_a_shell_command() {
        let request = AwUwfTaskRequest {
            goal: "cmd.exe /c del F:\\ShanghaiP4\\neon".to_string(),
            workspace: Some("neon-dev1".to_string()),
            task_id: Some("task-001".to_string()),
            provider: Some("codex".to_string()),
            project: None,
            primary: None,
            scope: None,
            confirmed: None,
            dry_run_accepted: None,
            confirmation_boundary: None,
            ..Default::default()
        };
        let args = master_args("dry-run", &request, None).unwrap();

        assert_eq!(args[0], "master");
        assert_eq!(args[1], "dry-run");
        assert_eq!(args[2], "--goal");
        assert_eq!(args[3], request.goal);
        assert_eq!(args.last().map(String::as_str), Some("--json"));
        assert!(!args.iter().take(3).any(|arg| {
            matches!(
                arg.to_ascii_lowercase().as_str(),
                "cmd" | "cmd.exe" | "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
            )
        }));
    }

    #[test]
    fn task_request_rejects_unknown_provider_and_control_characters() {
        let unsupported = AwUwfTaskRequest {
            goal: "smoke".to_string(),
            workspace: None,
            task_id: None,
            provider: Some("raw-shell".to_string()),
            project: None,
            primary: None,
            scope: None,
            confirmed: None,
            dry_run_accepted: None,
            confirmation_boundary: None,
            ..Default::default()
        };
        assert!(validate_task_request(&unsupported)
            .unwrap_err()
            .contains("Unsupported UEWorkflow provider"));

        let control_character = AwUwfTaskRequest {
            goal: "smoke\ncmd.exe".to_string(),
            workspace: None,
            task_id: None,
            provider: Some("codex".to_string()),
            project: None,
            primary: None,
            scope: None,
            confirmed: None,
            dry_run_accepted: None,
            confirmation_boundary: None,
            ..Default::default()
        };
        assert!(validate_task_request(&control_character)
            .unwrap_err()
            .contains("control characters"));
    }

    #[test]
    fn execute_requires_master_confirmation() {
        let request = AwUwfTaskRequest {
            goal: "smoke".to_string(),
            workspace: None,
            task_id: None,
            provider: None,
            project: None,
            primary: None,
            scope: None,
            confirmed: Some(true),
            dry_run_accepted: Some(true),
            confirmation_boundary: Some("wrong".to_string()),
            ..Default::default()
        };
        let error = run_master_execute(&request, Path::new("unused")).unwrap_err();
        assert!(error.contains(UWF_MASTER_CONFIRMATION));
    }

    #[test]
    fn master_execute_requires_dry_run_acceptance() {
        let request = AwUwfTaskRequest {
            goal: "smoke".to_string(),
            workspace: None,
            task_id: None,
            provider: None,
            project: None,
            primary: None,
            scope: None,
            confirmed: Some(true),
            dry_run_accepted: None,
            confirmation_boundary: Some(UWF_MASTER_CONFIRMATION.to_string()),
            ..Default::default()
        };
        let error = run_master_execute(&request, Path::new("unused")).unwrap_err();
        assert!(error.contains("dryRunAccepted=true"));
    }

    #[test]
    fn module_group_names_are_business_names() {
        assert_eq!(normalize_module_group("开发流").unwrap(), "dev");
        assert_eq!(normalize_module_group("智能体").unwrap(), "agents");
        assert_eq!(normalize_module_group("知识库").unwrap(), "kb");
    }

    #[test]
    fn external_command_ids_map_to_fixed_uwf_args() {
        let request = AwUwfExternalCommandRequest {
            command_id: "dev.buildCheck.execute".to_string(),
            goal: Some("检查构建路由".to_string()),
            workspace: Some("neon-dev1".to_string()),
            task_id: Some("task-001".to_string()),
            provider: None,
            project: Some("Neon".to_string()),
            primary: Some("AesWorld".to_string()),
            scope: None,
            confirmed: Some(true),
            dry_run_accepted: Some(true),
            confirmation_boundary: Some(UWF_DEVFLOW_BUILD_CHECK_CONFIRMATION.to_string()),
            ..Default::default()
        };
        let args = external_command_args(&request).unwrap();

        assert_eq!(args[0], "dev");
        assert_eq!(args[1], "execute");
        assert!(args.contains(&"--action".to_string()));
        assert!(args.contains(&"build-check".to_string()));
        assert!(args.contains(&"--confirm".to_string()));
        assert_eq!(args.last().map(String::as_str), Some("--json"));
        assert!(!args.iter().any(|arg| {
            matches!(
                arg.to_ascii_lowercase().as_str(),
                "cmd" | "cmd.exe" | "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
            )
        }));
    }

    #[test]
    fn external_command_policy_ids_are_unique_and_shell_free() {
        let mut ids = HashSet::new();
        for policy in EXTERNAL_COMMAND_POLICIES {
            assert!(ids.insert(policy.id), "duplicate policy id {}", policy.id);
            assert!(!policy.id.to_ascii_lowercase().contains("shell"));
            assert!(!policy.id.to_ascii_lowercase().contains("powershell"));
            assert!(!policy.id.to_ascii_lowercase().contains("cmd"));
            assert_ne!(policy.top_level, Some("cmd"));
            assert_ne!(policy.top_level, Some("powershell"));
            assert_ne!(policy.top_level, Some("pwsh"));
        }
    }

    #[test]
    fn external_command_policy_covers_initial_agentwatcher_command_surface() {
        let policy_ids = EXTERNAL_COMMAND_POLICIES
            .iter()
            .map(|policy| policy.id)
            .collect::<HashSet<_>>();
        for required in [
            "doctor",
            "modules",
            "dev.status",
            "dev.capabilities",
            "dev.commands",
            "dev.schema",
            "dev.history",
            "dev.artifacts",
            "dev.switch.dryRun",
            "dev.switch.execute",
            "dev.buildCheck.dryRun",
            "dev.buildCheck.execute",
            "agents.status",
            "agents.install.dryRun",
            "agents.install.execute",
            "agents.verify.dryRun",
            "agents.verify.execute",
            "agents.package.copilot.dryRun",
            "agents.package.copilot.execute",
            "agents.package.codex.dryRun",
            "agents.package.codex.execute",
            "agents.package.claude.dryRun",
            "agents.package.claude.execute",
            "agents.package.opencode.dryRun",
            "agents.package.opencode.execute",
            "kb.status",
            "kb.query.dryRun",
            "kb.query.execute",
            "kb.artifacts",
        ] {
            assert!(
                policy_ids.contains(required),
                "{required} must be callable through AgentWatcher policy"
            );
        }

        let views = external_command_policy_views();
        assert_eq!(views.len(), EXTERNAL_COMMAND_POLICIES.len());
        assert!(views
            .iter()
            .any(|view| view.command_id == "dev.switch.execute"
                && view.requires_confirmation
                && view.confirmation_boundary.as_deref() == Some(UWF_DEVFLOW_SWITCH_CONFIRMATION)));
    }

    #[test]
    fn external_command_policy_excludes_ai_task_work() {
        let policy_ids = EXTERNAL_COMMAND_POLICIES
            .iter()
            .map(|policy| policy.id)
            .collect::<HashSet<_>>();
        for forbidden in [
            "agents.runMaster.execute",
            "agents.diagnose.execute",
            "agents.review.execute",
            "master.plan",
            "master.execute",
        ] {
            assert!(
                !policy_ids.contains(forbidden),
                "{forbidden} must go through Provider Agent or UnrealMaster task flow"
            );
            let request = AwUwfExternalCommandRequest {
                command_id: forbidden.to_string(),
                goal: Some("需要 AI 处理的任务".to_string()),
                workspace: None,
                task_id: None,
                provider: Some("copilot".to_string()),
                project: None,
                primary: None,
                scope: None,
                confirmed: Some(true),
                dry_run_accepted: Some(true),
                confirmation_boundary: Some("unsafe".to_string()),
                ..Default::default()
            };
            assert!(external_command_args(&request)
                .unwrap_err()
                .contains("Unsupported UEWorkflow command id"));
        }
    }

    #[test]
    fn external_action_policies_match_live_uwf_declared_actions_when_available() {
        if resolve_uwf_executable_from_agentwatcher_root(&agentwatcher_root()).is_err() {
            return;
        }
        for module in ["dev", "agents", "kb"] {
            let response = run_uwf_json(
                &[
                    module.to_string(),
                    "commands".to_string(),
                    "--json".to_string(),
                ],
                None,
            )
            .unwrap();
            let declared_actions = response
                .result
                .get("declaredActions")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|action| action.get("name").and_then(Value::as_str))
                .collect::<HashSet<_>>();

            for policy in EXTERNAL_COMMAND_POLICIES
                .iter()
                .filter(|policy| policy.module == Some(module))
                .filter_map(|policy| policy.action)
            {
                assert!(
                    declared_actions.contains(policy),
                    "{module} policy action {policy} must be declared by uwf commands"
                );
            }
        }
    }

    #[test]
    fn external_command_rejects_unknown_or_shell_like_ids() {
        let unsupported = AwUwfExternalCommandRequest {
            command_id: "dev.shell".to_string(),
            goal: None,
            workspace: None,
            task_id: None,
            provider: None,
            project: None,
            primary: None,
            scope: None,
            confirmed: None,
            dry_run_accepted: None,
            confirmation_boundary: None,
            ..Default::default()
        };
        assert!(external_command_args(&unsupported)
            .unwrap_err()
            .contains("Unsupported UEWorkflow command id"));

        let shell_like = AwUwfExternalCommandRequest {
            command_id: "cmd.exe /c del".to_string(),
            ..unsupported
        };
        assert!(external_command_args(&shell_like)
            .unwrap_err()
            .contains("command id is invalid"));
    }

    #[test]
    fn external_switch_execute_requires_confirmation() {
        let request = AwUwfExternalCommandRequest {
            command_id: "dev.switch.execute".to_string(),
            goal: None,
            workspace: Some("neon-dev1".to_string()),
            task_id: Some("task-001".to_string()),
            provider: None,
            project: None,
            primary: None,
            scope: None,
            confirmed: Some(true),
            dry_run_accepted: Some(true),
            confirmation_boundary: Some("wrong".to_string()),
            ..Default::default()
        };
        assert!(external_command_args(&request)
            .unwrap_err()
            .contains(UWF_DEVFLOW_SWITCH_CONFIRMATION));
    }

    #[test]
    fn external_execute_requires_dry_run_acceptance() {
        let request = AwUwfExternalCommandRequest {
            command_id: "agents.install.execute".to_string(),
            goal: None,
            workspace: None,
            task_id: None,
            provider: None,
            project: None,
            primary: None,
            scope: None,
            confirmed: Some(true),
            dry_run_accepted: None,
            confirmation_boundary: Some(UWF_AGENTHUB_INSTALL_PROVIDER_CONFIRMATION.to_string()),
            ..Default::default()
        };
        assert!(external_command_args(&request)
            .unwrap_err()
            .contains("dryRunAccepted=true"));
    }

    #[test]
    fn run_doctor_reads_uwf_contract_when_binary_exists() {
        if resolve_uwf_executable_from_agentwatcher_root(&agentwatcher_root()).is_err() {
            return;
        }
        let response = run_doctor().unwrap();
        assert_eq!(
            response.result.get("kind").and_then(Value::as_str),
            Some("UnrealWorkflowDoctor")
        );
        assert_eq!(
            response.result.get("status").and_then(Value::as_str),
            Some("ok")
        );
    }
}
