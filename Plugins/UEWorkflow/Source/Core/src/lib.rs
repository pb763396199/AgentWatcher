pub const CONTRACT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModuleId {
    DevFlow,
    AgentHub,
    KnowledgeBase,
}

impl ModuleId {
    pub fn code_name(self) -> &'static str {
        match self {
            Self::DevFlow => "DevFlow",
            Self::AgentHub => "AgentHub",
            Self::KnowledgeBase => "KnowledgeBase",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            Self::DevFlow => "devflow",
            Self::AgentHub => "agents",
            Self::KnowledgeBase => "kb",
        }
    }

    pub fn command_group(self) -> &'static str {
        match self {
            Self::DevFlow => "uwf dev",
            Self::AgentHub => "uwf agents",
            Self::KnowledgeBase => "uwf kb",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::DevFlow => "虚幻开发流",
            Self::AgentHub => "虚幻智能体",
            Self::KnowledgeBase => "虚幻知识库",
        }
    }

    pub fn english_display_name(self) -> &'static str {
        match self {
            Self::DevFlow => "Dev Flow",
            Self::AgentHub => "Agent Hub",
            Self::KnowledgeBase => "Knowledge Base",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafetyLevel {
    ReadOnly,
    Bounded,
    Dangerous,
}

impl SafetyLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "readOnly",
            Self::Bounded => "bounded",
            Self::Dangerous => "dangerous",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandStatus {
    Ready,
    Planned,
    Running,
    Complete,
    Blocked,
    Failed,
}

impl CommandStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Planned => "planned",
            Self::Running => "running",
            Self::Complete => "complete",
            Self::Blocked => "blocked",
            Self::Failed => "failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    UserInput,
    Environment,
    Tool,
    Internal,
}

impl ErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserInput => "userInput",
            Self::Environment => "environment",
            Self::Tool => "tool",
            Self::Internal => "internal",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderRole {
    Master,
    Planner,
    Executor,
    Reviewer,
    Diagnostician,
    KnowledgeCurator,
}

impl ProviderRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Master => "master",
            Self::Planner => "planner",
            Self::Executor => "executor",
            Self::Reviewer => "reviewer",
            Self::Diagnostician => "diagnostician",
            Self::KnowledgeCurator => "knowledgeCurator",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeScopeKind {
    EngineCore,
    EnginePlugin,
    ProjectPlugin,
    Project,
    Global,
}

impl KnowledgeScopeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::EngineCore => "engine_core",
            Self::EnginePlugin => "engine_plugin",
            Self::ProjectPlugin => "project_plugin",
            Self::Project => "project",
            Self::Global => "global",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceId {
    pub value: String,
}

impl WorkspaceId {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub fn to_json(&self) -> String {
        format!("{{\"value\":{}}}", json_string(&self.value))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRef {
    pub id: String,
    pub kind: String,
    pub path: String,
    pub description: String,
}

impl ArtifactRef {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"id\":{},\"kind\":{},\"path\":{},\"description\":{}}}",
            json_string(&self.id),
            json_string(&self.kind),
            json_string(&self.path),
            json_string(&self.description)
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeScope {
    pub scope_id: String,
    pub scope_kind: KnowledgeScopeKind,
    pub display_name: String,
    pub engine_version: Option<String>,
    pub project_id: Option<String>,
    pub plugin_id: Option<String>,
}

impl KnowledgeScope {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"scopeId\":{},\"scopeKind\":{},\"displayName\":{},\"engineVersion\":{},\"projectId\":{},\"pluginId\":{}}}",
            json_string(&self.scope_id),
            json_string(self.scope_kind.as_str()),
            json_string(&self.display_name),
            json_option(self.engine_version.as_deref()),
            json_option(self.project_id.as_deref()),
            json_option(self.plugin_id.as_deref())
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DryRunStage {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub safety: SafetyLevel,
    pub will_execute: bool,
}

impl DryRunStage {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"id\":{},\"title\":{},\"summary\":{},\"safety\":{},\"willExecute\":{}}}",
            json_string(&self.id),
            json_string(&self.title),
            json_string(&self.summary),
            json_string(self.safety.as_str()),
            self.will_execute
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DryRunPlan {
    pub goal: String,
    pub stages: Vec<DryRunStage>,
    pub blocked_actions: Vec<String>,
    pub confirmation_boundary: Option<String>,
}

impl DryRunPlan {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"goal\":{},\"willExecute\":false,\"stages\":{},\"blockedActions\":{},\"confirmationBoundary\":{}}}",
            json_string(&self.goal),
            json_array(self.stages.iter().map(DryRunStage::to_json)),
            json_array(self.blocked_actions.iter().map(|action| json_string(action))),
            json_option(self.confirmation_boundary.as_deref())
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskContext {
    pub task_id: String,
    pub workspace_id: WorkspaceId,
    pub goal: String,
    pub module: ModuleId,
    pub knowledge_scope: Option<KnowledgeScope>,
}

impl TaskContext {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"taskId\":{},\"workspaceId\":{},\"goal\":{},\"module\":{},\"knowledgeScope\":{}}}",
            json_string(&self.task_id),
            self.workspace_id.to_json(),
            json_string(&self.goal),
            json_string(self.module.code_name()),
            self.knowledge_scope
                .as_ref()
                .map(KnowledgeScope::to_json)
                .unwrap_or_else(|| "null".to_string())
        )
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CommandRequest {
    pub goal: Option<String>,
    pub workspace: Option<String>,
    pub task_id: Option<String>,
    pub action: Option<String>,
    pub confirmation: Option<String>,
    pub provider: Option<String>,
    pub scope: Option<String>,
    pub project: Option<String>,
    pub primary: Option<String>,
    pub host_root: Option<String>,
    pub main_project: Option<String>,
    pub primary_path: Option<String>,
    pub plugin_dependencies: Option<String>,
    pub compact: bool,
}

impl CommandRequest {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"goal\":{},\"workspace\":{},\"taskId\":{},\"action\":{},\"confirmation\":{},\"provider\":{},\"scope\":{},\"project\":{},\"primary\":{},\"hostRoot\":{},\"mainProject\":{},\"primaryPath\":{},\"pluginDependencies\":{},\"compact\":{}}}",
            json_option(self.goal.as_deref()),
            json_option(self.workspace.as_deref()),
            json_option(self.task_id.as_deref()),
            json_option(self.action.as_deref()),
            json_option(self.confirmation.as_deref()),
            json_option(self.provider.as_deref()),
            json_option(self.scope.as_deref()),
            json_option(self.project.as_deref()),
            json_option(self.primary.as_deref()),
            json_option(self.host_root.as_deref()),
            json_option(self.main_project.as_deref()),
            json_option(self.primary_path.as_deref()),
            json_option(self.plugin_dependencies.as_deref()),
            self.compact
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandOutcome {
    pub status: CommandStatus,
    pub message: String,
    pub artifacts: Vec<ArtifactRef>,
    pub dry_run: Option<DryRunPlan>,
}

impl CommandOutcome {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"status\":{},\"message\":{},\"artifacts\":{},\"dryRun\":{}}}",
            json_string(self.status.as_str()),
            json_string(&self.message),
            json_array(self.artifacts.iter().map(ArtifactRef::to_json)),
            self.dry_run
                .as_ref()
                .map(DryRunPlan::to_json)
                .unwrap_or_else(|| "null".to_string())
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionRecord {
    pub record_id: String,
    pub task_id: String,
    pub command: String,
    pub provider_role: Option<ProviderRole>,
    pub outcome: CommandOutcome,
}

impl ExecutionRecord {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"recordId\":{},\"taskId\":{},\"command\":{},\"providerRole\":{},\"outcome\":{}}}",
            json_string(&self.record_id),
            json_string(&self.task_id),
            json_string(&self.command),
            self.provider_role
                .map(|role| json_string(role.as_str()))
                .unwrap_or_else(|| "null".to_string()),
            self.outcome.to_json()
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UwfError {
    pub kind: ErrorKind,
    pub message: String,
    pub evidence: Option<String>,
}

impl UwfError {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"kind\":\"UnrealWorkflowError\",\"contractVersion\":{},\"errorKind\":{},\"message\":{},\"evidence\":{}}}",
            CONTRACT_VERSION,
            json_string(self.kind.as_str()),
            json_string(&self.message),
            json_option(self.evidence.as_deref())
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Capability {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub safety: SafetyLevel,
    pub supports_dry_run: bool,
    pub enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleContract {
    pub id: ModuleId,
    pub capabilities: Vec<Capability>,
    pub commands: Vec<CommandSpec>,
    pub schema: &'static str,
}

impl ModuleContract {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"codeName\":{},\"shortName\":{},\"displayName\":{},\"englishDisplayName\":{},\"commandGroup\":{},\"schemaVersion\":{},\"capabilities\":{},\"commands\":{}}}",
            json_string(self.id.code_name()),
            json_string(self.id.short_name()),
            json_string(self.id.display_name()),
            json_string(self.id.english_display_name()),
            json_string(self.id.command_group()),
            CONTRACT_VERSION,
            capabilities_json(&self.capabilities),
            commands_json(&self.commands)
        )
    }

    pub fn result_json(&self, command: &str, status: &str, message: &str, extra: &str) -> String {
        let extra_field = if extra.trim().is_empty() {
            String::new()
        } else {
            format!(",{extra}")
        };
        format!(
            "{{\"kind\":\"UnrealWorkflowModuleResult\",\"contractVersion\":{},\"module\":{},\"command\":{},\"status\":{},\"message\":{},\"capabilities\":{},\"commands\":{},\"schema\":{}{} }}",
            CONTRACT_VERSION,
            self.to_json(),
            json_string(command),
            json_string(status),
            json_string(message),
            capabilities_json(&self.capabilities),
            commands_json(&self.commands),
            json_string(self.schema),
            extra_field
        )
    }
}

pub fn capabilities_json(capabilities: &[Capability]) -> String {
    json_array(capabilities.iter().map(|capability| {
        format!(
            "{{\"id\":{},\"title\":{},\"description\":{}}}",
            json_string(capability.id),
            json_string(capability.title),
            json_string(capability.description)
        )
    }))
}

pub fn commands_json(commands: &[CommandSpec]) -> String {
    json_array(commands.iter().map(|command| {
        format!(
            "{{\"name\":{},\"summary\":{},\"safety\":{},\"supportsDryRun\":{},\"enabled\":{}}}",
            json_string(command.name),
            json_string(command.summary),
            json_string(command.safety.as_str()),
            command.supports_dry_run,
            command.enabled
        )
    }))
}

pub fn json_array<I>(items: I) -> String
where
    I: IntoIterator<Item = String>,
{
    let mut result = String::from("[");
    let mut first = true;
    for item in items {
        if !first {
            result.push(',');
        }
        first = false;
        result.push_str(&item);
    }
    result.push(']');
    result
}

pub fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

pub fn json_option(value: Option<&str>) -> String {
    value.map(json_string).unwrap_or_else(|| "null".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_string_escapes_control_characters() {
        assert_eq!(json_string("a\"b\\c\n"), "\"a\\\"b\\\\c\\n\"");
    }

    #[test]
    fn module_id_names_are_stable() {
        assert_eq!(ModuleId::DevFlow.command_group(), "uwf dev");
        assert_eq!(ModuleId::AgentHub.short_name(), "agents");
        assert_eq!(ModuleId::KnowledgeBase.code_name(), "KnowledgeBase");
    }

    #[test]
    fn core_execution_types_emit_stable_json() {
        let scope = KnowledgeScope {
            scope_id: "project/neon/plugin/aesworld".to_string(),
            scope_kind: KnowledgeScopeKind::ProjectPlugin,
            display_name: "AesWorld".to_string(),
            engine_version: Some("5.5.4".to_string()),
            project_id: Some("neon".to_string()),
            plugin_id: Some("aesworld".to_string()),
        };
        let context = TaskContext {
            task_id: "task-001".to_string(),
            workspace_id: WorkspaceId::new("neon-dev1"),
            goal: "smoke".to_string(),
            module: ModuleId::DevFlow,
            knowledge_scope: Some(scope),
        };
        let json = context.to_json();
        assert!(json.contains("\"taskId\":\"task-001\""));
        assert!(json.contains("\"scopeKind\":\"project_plugin\""));

        let plan = DryRunPlan {
            goal: "smoke".to_string(),
            stages: vec![DryRunStage {
                id: "plan".to_string(),
                title: "计划".to_string(),
                summary: "只读预演".to_string(),
                safety: SafetyLevel::ReadOnly,
                will_execute: false,
            }],
            blocked_actions: vec!["raw UE build".to_string()],
            confirmation_boundary: Some("UEWorkflow.devflow.createTask.v1".to_string()),
        };
        let outcome = CommandOutcome {
            status: CommandStatus::Planned,
            message: "dry-run complete".to_string(),
            artifacts: vec![ArtifactRef {
                id: "artifact-1".to_string(),
                kind: "plan".to_string(),
                path: "%USERPROFILE%/.unrealworkflow/tasks/task-001/plan.json".to_string(),
                description: "计划产物".to_string(),
            }],
            dry_run: Some(plan),
        };
        let record = ExecutionRecord {
            record_id: "record-001".to_string(),
            task_id: "task-001".to_string(),
            command: "uwf master dry-run".to_string(),
            provider_role: Some(ProviderRole::Master),
            outcome,
        };
        let json = record.to_json();
        assert!(json.contains("\"providerRole\":\"master\""));
        assert!(json.contains("\"willExecute\":false"));
        assert!(json.contains("\"blockedActions\":[\"raw UE build\"]"));
    }

    #[test]
    fn core_error_types_emit_stable_json() {
        let error = UwfError {
            kind: ErrorKind::Environment,
            message: "missing binding".to_string(),
            evidence: Some("projects.DevFlow.root".to_string()),
        };
        let json = error.to_json();
        assert!(json.contains("\"kind\":\"UnrealWorkflowError\""));
        assert!(json.contains("\"errorKind\":\"environment\""));
        assert!(json.contains("\"evidence\":\"projects.DevFlow.root\""));
    }

    #[test]
    fn command_request_uses_stable_camel_case_json() {
        let request = CommandRequest {
            goal: Some("fix terrain".to_string()),
            workspace: Some("neon-dev1".to_string()),
            task_id: Some("task-001".to_string()),
            action: Some("build-check".to_string()),
            confirmation: Some("UEWorkflow.DevFlow.build-check.v1".to_string()),
            provider: Some("copilot".to_string()),
            scope: Some("project_plugin/aesworld".to_string()),
            project: Some("neon".to_string()),
            primary: Some("AesWorld".to_string()),
            host_root: Some("F:\\ShanghaiP4\\neon\\Hosts".to_string()),
            main_project: Some("F:\\ShanghaiP4\\neon\\UGA\\DEV_1".to_string()),
            primary_path: Some("F:\\ShanghaiP4\\neon\\Plugins\\AesWorld".to_string()),
            plugin_dependencies: Some("GeometryProcessing".to_string()),
            compact: false,
        };
        let json = request.to_json();
        assert!(json.contains("\"taskId\":\"task-001\""));
        assert!(json.contains("\"action\":\"build-check\""));
        assert!(json.contains("\"confirmation\":\"UEWorkflow.DevFlow.build-check.v1\""));
        assert!(json.contains("\"mainProject\":\"F:\\\\ShanghaiP4\\\\neon\\\\UGA\\\\DEV_1\""));
    }
}
