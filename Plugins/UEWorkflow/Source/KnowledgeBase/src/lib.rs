use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use uwf_core::{
    json_array, json_option, json_string, ArtifactRef, Capability, CommandOutcome, CommandRequest,
    CommandSpec, CommandStatus, DryRunPlan, DryRunStage, KnowledgeScope, KnowledgeScopeKind,
    ModuleContract, ModuleId, SafetyLevel,
};

pub fn contract() -> ModuleContract {
    ModuleContract {
        id: ModuleId::KnowledgeBase,
        capabilities: vec![
            Capability {
                id: "scope.registry",
                title: "知识范围注册",
                description:
                    "按 engine_core、engine_plugin、project_plugin、project、global 隔离知识。",
            },
            Capability {
                id: "query.routing",
                title: "查询路由",
                description: "默认带 scope 查询，避免跨插件、跨引擎误命中。",
            },
            Capability {
                id: "artifact.curation",
                title: "知识沉淀",
                description: "把任务复盘、解决方案、规则和轻量索引沉淀到团队知识库。",
            },
        ],
        commands: vec![
            CommandSpec {
                name: "status",
                summary: "输出知识库模块状态。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "capabilities",
                summary: "输出知识库能力清单。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "commands",
                summary: "输出知识库命令清单。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "schema",
                summary: "输出知识库结果契约。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "dry-run",
                summary: "预演知识范围、查询和沉淀路径，不生成重型索引。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: true,
                enabled: true,
            },
            CommandSpec {
                name: "execute",
                summary: "执行安全知识动作；重型生成和导出默认阻断。",
                safety: SafetyLevel::Dangerous,
                supports_dry_run: true,
                enabled: true,
            },
            CommandSpec {
                name: "history",
                summary: "读取知识沉淀历史。本阶段返回空集合。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "artifacts",
                summary: "读取知识产物索引。本阶段返回空集合。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
        ],
        schema: "uwf.module.knowledgebase.v1",
    }
}

pub fn command_json(command: &str, request: &CommandRequest) -> String {
    match command {
        "status" => status_json(command, request),
        "capabilities" => contract().result_json(
            command,
            "ready",
            "虚幻知识库能力清单已输出。",
            &format!("\"request\":{},\"sideEffects\":[]", request.to_json()),
        ),
        "commands" => commands_json(command, request),
        "schema" => schema_json(command, request),
        "dry-run" => dry_run_json(command, request),
        "execute" => execute_json(command, request),
        "history" => history_json(command, request),
        "artifacts" => artifacts_json(command, request),
        _ => contract().result_json(
            command,
            "blocked",
            "未知知识库命令；请通过 commands 查看清单。",
            &format!("\"request\":{},\"blocked\":true", request.to_json()),
        ),
    }
}

fn status_json(command: &str, request: &CommandRequest) -> String {
    let registry = KnowledgeRegistry::discover();
    let scope = resolve_scope(request);
    let extra = format!(
        "\"request\":{},\"installed\":true,\"sideEffects\":[],\"knowledgeBase\":{{\"scope\":{},\"scopePath\":{},\"registry\":{},\"storagePolicy\":{},\"pipelinePolicy\":{}}}",
        request.to_json(),
        scope.to_json(),
        json_string(&scope_relative_path(&scope)),
        registry.to_json(),
        storage_policy_json(),
        pipeline_policy_json()
    );
    contract().result_json(
        command,
        "ready",
        "虚幻知识库已加载；scope 隔离和存储策略已就绪。",
        &extra,
    )
}

fn commands_json(command: &str, request: &CommandRequest) -> String {
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"declaredActions\":{}",
        request.to_json(),
        declared_actions_json()
    );
    contract().result_json(command, "ready", "知识库命令清单已输出。", &extra)
}

fn schema_json(command: &str, request: &CommandRequest) -> String {
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"schemaDetail\":{{\"command\":\"uwf kb <status|capabilities|commands|schema|dry-run|execute|history|artifacts>\",\"request\":{},\"scopeKinds\":{},\"lightArtifacts\":{}}}",
        request.to_json(),
        json_string("CommandRequest"),
        json_array(scope_kind_names().into_iter().map(json_string)),
        json_array(["scope-manifest.json", "index/light-index.md", "retrospectives/*.md", "solutions/*.md", "rules/*.md"].into_iter().map(json_string))
    );
    contract().result_json(command, "ready", "知识库 schema 已输出。", &extra)
}

fn dry_run_json(command: &str, request: &CommandRequest) -> String {
    let action = normalize_action(request.action.as_deref());
    let scope = resolve_scope(request);
    let plan = dry_run_plan(action, request, &scope);
    let outcome = CommandOutcome {
        status: CommandStatus::Planned,
        message: "知识库 dry-run 已生成；不会生成重型索引，不会写入缓存。".to_string(),
        artifacts: vec![ArtifactRef {
            id: "knowledge-dry-run".to_string(),
            kind: "knowledgePlan".to_string(),
            path: format!(
                "UnrealWorkflowKnowledge/{}/index/light-index.md",
                scope_relative_path(&scope)
            ),
            description: "知识沉淀预演路径".to_string(),
        }],
        dry_run: Some(plan.clone()),
    };
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"action\":{},\"scope\":{},\"scopePath\":{},\"confirmationBoundary\":{},\"dryRun\":{},\"outcome\":{},\"storagePolicy\":{}",
        request.to_json(),
        json_string(action),
        scope.to_json(),
        json_string(&scope_relative_path(&scope)),
        json_string(&confirmation_boundary(action)),
        plan.to_json(),
        outcome.to_json(),
        storage_policy_json()
    );
    contract().result_json(
        command,
        "planned",
        "知识库 dry-run 已完成；scope、产物路径和阻断动作已列出。",
        &extra,
    )
}

fn execute_json(command: &str, request: &CommandRequest) -> String {
    let action = normalize_action(request.action.as_deref());
    let expected_boundary = confirmation_boundary(action);
    if request.confirmation.as_deref() != Some(expected_boundary.as_str()) {
        return contract().result_json(
            command,
            "blocked",
            "execute 缺少匹配的 dry-run 确认边界；未执行任何动作。",
            &format!(
                "\"request\":{},\"sideEffects\":[],\"blocked\":true,\"requiredConfirmation\":{},\"action\":{}",
                request.to_json(),
                json_string(&expected_boundary),
                json_string(action)
            ),
        );
    }

    if !is_safe_execute_action(action) {
        return contract().result_json(
            command,
            "blocked",
            "该知识库动作可能触发重型 pipeline、缓存或导出；当前阶段保持阻断。",
            &format!(
                "\"request\":{},\"sideEffects\":[],\"blocked\":true,\"action\":{},\"blockedActions\":{}",
                request.to_json(),
                json_string(action),
                json_array(blocked_actions(action).into_iter().map(|action| json_string(&action)))
            ),
        );
    }

    let scope = resolve_scope(request);
    match action {
        "scope-status" => {
            let extra = format!(
                "\"request\":{},\"sideEffects\":[],\"executedAction\":{},\"scope\":{},\"scopePath\":{},\"storagePolicy\":{}",
                request.to_json(),
                json_string(action),
                scope.to_json(),
                json_string(&scope_relative_path(&scope)),
                storage_policy_json()
            );
            contract().result_json(command, "complete", "知识 scope 状态已读取。", &extra)
        }
        "query" => {
            let result = query_scope(&scope, request.goal.as_deref().unwrap_or(""));
            let extra = format!(
                "\"request\":{},\"sideEffects\":[],\"executedAction\":{},\"scope\":{},\"query\":{},\"matches\":{}",
                request.to_json(),
                json_string(action),
                scope.to_json(),
                json_option(request.goal.as_deref()),
                result
            );
            contract().result_json(command, "complete", "知识库查询已完成。", &extra)
        }
        "record-task" => match record_task_knowledge(&scope, request) {
            Ok(record) => {
                let extra = format!(
                    "\"request\":{},\"sideEffects\":[{}],\"executedAction\":{},\"scope\":{},\"artifacts\":{}",
                    request.to_json(),
                    json_string("write-light-knowledge"),
                    json_string(action),
                    scope.to_json(),
                    record
                );
                contract().result_json(command, "complete", "任务知识已写入正确 scope。", &extra)
            }
            Err(error) => contract().result_json(
                command,
                "failed",
                "任务知识写入失败；已保留错误证据。",
                &format!(
                    "\"request\":{},\"sideEffects\":[],\"error\":{}",
                    request.to_json(),
                    json_string(&error)
                ),
            ),
        },
        _ => unreachable!("safe execute action checked above"),
    }
}

fn history_json(command: &str, request: &CommandRequest) -> String {
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"historyRoot\":{},\"history\":[]",
        request.to_json(),
        json_string("UnrealWorkflowKnowledge/global/workflows")
    );
    contract().result_json(command, "ready", "知识库历史索引已输出。", &extra)
}

fn artifacts_json(command: &str, request: &CommandRequest) -> String {
    let scope = resolve_scope(request);
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"scope\":{},\"artifactRoot\":{},\"artifacts\":[]",
        request.to_json(),
        scope.to_json(),
        json_string(&format!(
            "UnrealWorkflowKnowledge/{}",
            scope_relative_path(&scope)
        ))
    );
    contract().result_json(command, "ready", "知识库产物索引已输出。", &extra)
}

fn dry_run_plan(action: &str, request: &CommandRequest, scope: &KnowledgeScope) -> DryRunPlan {
    DryRunPlan {
        goal: request
            .goal
            .clone()
            .unwrap_or_else(|| format!("knowledgebase action: {action}")),
        stages: vec![
            DryRunStage {
                id: "resolve-scope".to_string(),
                title: "解析知识 scope".to_string(),
                summary: format!("目标 scope: {}", scope.scope_kind.as_str()),
                safety: SafetyLevel::ReadOnly,
                will_execute: false,
            },
            DryRunStage {
                id: "separate-cache".to_string(),
                title: "分离缓存和团队知识库".to_string(),
                summary: "本机重型缓存进入 %USERPROFILE%/.unrealworkflow/knowledge-cache，团队知识库只收轻量 Markdown。".to_string(),
                safety: SafetyLevel::ReadOnly,
                will_execute: false,
            },
            DryRunStage {
                id: "block-heavy-pipeline".to_string(),
                title: "阻断重型 pipeline".to_string(),
                summary: "不会生成 SQLite/Pickle/全量索引，不会运行旧 Python pipeline。".to_string(),
                safety: SafetyLevel::Dangerous,
                will_execute: false,
            },
        ],
        blocked_actions: blocked_actions(action),
        confirmation_boundary: Some(confirmation_boundary(action)),
    }
}

fn normalize_action(action: Option<&str>) -> &'static str {
    match action.unwrap_or("scope-status") {
        "scope-status" | "status" => "scope-status",
        "query" | "search" => "query",
        "record-task" | "settle" | "curate" => "record-task",
        "generate" => "generate",
        "export" => "export",
        _ => "scope-status",
    }
}

fn confirmation_boundary(action: &str) -> String {
    format!("UEWorkflow.KnowledgeBase.{action}.v1")
}

fn is_safe_execute_action(action: &str) -> bool {
    matches!(action, "scope-status" | "query" | "record-task")
}

fn blocked_actions(action: &str) -> Vec<String> {
    let mut actions = vec![
        "本机重型缓存提交到 git".to_string(),
        "跨 scope 混写".to_string(),
        "运行旧 pipeline 生成 SQLite/Pickle".to_string(),
    ];
    if matches!(action, "generate" | "export") {
        actions.push("未经确认生成或导出重型知识库".to_string());
    }
    actions
}

fn declared_actions_json() -> String {
    let actions = ["scope-status", "query", "record-task", "generate", "export"];
    json_array(actions.into_iter().map(|action| {
        format!(
            "{{\"name\":{},\"safeExecute\":{},\"confirmationBoundary\":{}}}",
            json_string(action),
            is_safe_execute_action(action),
            json_string(&confirmation_boundary(action))
        )
    }))
}

fn resolve_scope(request: &CommandRequest) -> KnowledgeScope {
    let kind = scope_kind(request.scope.as_deref());
    let project_id = request
        .project
        .clone()
        .or_else(|| Some("project".to_string()));
    let plugin_id = request
        .primary
        .clone()
        .or_else(|| Some("plugin".to_string()));
    let display_name = match kind {
        KnowledgeScopeKind::EngineCore => "引擎核心".to_string(),
        KnowledgeScopeKind::EnginePlugin => {
            format!("引擎插件 {}", plugin_id.as_deref().unwrap_or("plugin"))
        }
        KnowledgeScopeKind::ProjectPlugin => {
            format!("项目插件 {}", plugin_id.as_deref().unwrap_or("plugin"))
        }
        KnowledgeScopeKind::Project => {
            format!("项目 {}", project_id.as_deref().unwrap_or("project"))
        }
        KnowledgeScopeKind::Global => "全局知识".to_string(),
    };
    let scope_id = match kind {
        KnowledgeScopeKind::EngineCore => "engine/ue/engine-core".to_string(),
        KnowledgeScopeKind::EnginePlugin => {
            format!(
                "engine/ue/engine-plugin/{}",
                plugin_id.as_deref().unwrap_or("plugin")
            )
        }
        KnowledgeScopeKind::ProjectPlugin => format!(
            "project/{}/plugin/{}",
            project_id.as_deref().unwrap_or("project"),
            plugin_id.as_deref().unwrap_or("plugin")
        ),
        KnowledgeScopeKind::Project => format!(
            "project/{}/project",
            project_id.as_deref().unwrap_or("project")
        ),
        KnowledgeScopeKind::Global => "global".to_string(),
    };
    KnowledgeScope {
        scope_id,
        scope_kind: kind,
        display_name,
        engine_version: Some("ue".to_string()),
        project_id,
        plugin_id,
    }
}

fn scope_kind(scope: Option<&str>) -> KnowledgeScopeKind {
    match scope.unwrap_or("project_plugin") {
        "engine_core" | "engine-core" => KnowledgeScopeKind::EngineCore,
        "engine_plugin" | "engine-plugin" => KnowledgeScopeKind::EnginePlugin,
        "project" => KnowledgeScopeKind::Project,
        "global" => KnowledgeScopeKind::Global,
        _ => KnowledgeScopeKind::ProjectPlugin,
    }
}

fn scope_kind_names() -> Vec<&'static str> {
    vec![
        "engine_core",
        "engine_plugin",
        "project_plugin",
        "project",
        "global",
    ]
}

fn scope_relative_path(scope: &KnowledgeScope) -> String {
    let project = sanitize_segment(scope.project_id.as_deref().unwrap_or("project"));
    let plugin = sanitize_segment(scope.plugin_id.as_deref().unwrap_or("plugin"));
    match scope.scope_kind {
        KnowledgeScopeKind::EngineCore => "scopes/engines/ue/engine-core".to_string(),
        KnowledgeScopeKind::EnginePlugin => format!("scopes/engines/ue/engine-plugins/{plugin}"),
        KnowledgeScopeKind::ProjectPlugin => format!("scopes/projects/{project}/plugins/{plugin}"),
        KnowledgeScopeKind::Project => format!("scopes/projects/{project}/project"),
        KnowledgeScopeKind::Global => "global".to_string(),
    }
}

fn storage_policy_json() -> String {
    format!(
        "{{\"localHeavyCache\":{},\"teamKnowledgeRoot\":{},\"gitAllowed\":{},\"gitIgnored\":{}}}",
        json_string("%USERPROFILE%/.unrealworkflow/knowledge-cache"),
        json_string("UnrealWorkflowKnowledge"),
        json_array(
            [
                "Markdown",
                "scope-manifest.json",
                "schemas",
                "light-index.md"
            ]
            .into_iter()
            .map(json_string)
        ),
        json_array(
            [
                "SQLite",
                "Pickle",
                "target",
                "pipeline data",
                "artifacts/cache"
            ]
            .into_iter()
            .map(json_string)
        )
    )
}

fn pipeline_policy_json() -> String {
    format!(
        "{{\"oldPipelineWrapped\":true,\"heavyGenerationDefault\":\"blocked\",\"queryDefault\":\"scope-bound\",\"taskClosureWrites\":[{}, {}, {}]}}",
        json_string("retrospective"),
        json_string("solution-draft"),
        json_string("rule-candidate")
    )
}

#[derive(Clone, Debug)]
struct KnowledgeRegistry {
    source_available: bool,
    team_root_exists: bool,
}

impl KnowledgeRegistry {
    fn discover() -> Self {
        Self {
            source_available: source_root().as_deref().is_some_and(Path::is_dir),
            team_root_exists: team_root().is_dir(),
        }
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"adapterSourceAvailable\":{},\"teamRootExists\":{},\"scopeKinds\":{}}}",
            self.source_available,
            self.team_root_exists,
            json_array(scope_kind_names().into_iter().map(json_string))
        )
    }
}

fn query_scope(scope: &KnowledgeScope, query: &str) -> String {
    let scope_root = team_root().join(scope_relative_path(scope));
    let mut matches = Vec::new();
    if scope_root.is_dir() {
        collect_markdown_matches(&scope_root, query, &mut matches, 20);
    }
    json_array(matches.into_iter().map(|path| {
        format!(
            "{{\"path\":{},\"scopeId\":{},\"summary\":{}}}",
            json_string(&path),
            json_string(&scope.scope_id),
            json_string("scope-bound markdown match")
        )
    }))
}

fn collect_markdown_matches(root: &Path, query: &str, matches: &mut Vec<String>, limit: usize) {
    if matches.len() >= limit {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_matches(&path, query, matches, limit);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md")
            && markdown_matches(&path, query)
        {
            matches.push(path_to_repo_relative(&path));
        }
        if matches.len() >= limit {
            break;
        }
    }
}

fn markdown_matches(path: &Path, query: &str) -> bool {
    if query.trim().is_empty() {
        return true;
    }
    fs::read_to_string(path)
        .map(|content| content.contains(query))
        .unwrap_or(false)
}

fn record_task_knowledge(
    scope: &KnowledgeScope,
    request: &CommandRequest,
) -> Result<String, String> {
    let task_id = sanitize_segment(request.task_id.as_deref().unwrap_or("draft-task"));
    let goal = request.goal.as_deref().unwrap_or("未提供目标");
    let root = team_root().join(scope_relative_path(scope));
    let retrospective = root.join("retrospectives").join(format!("{task_id}.md"));
    let solution = root.join("solutions").join(format!("{task_id}.md"));
    let rule = root.join("rules").join(format!("{task_id}.md"));
    let index = root.join("index").join("light-index.md");
    let manifest = root.join("scope-manifest.json");

    for path in [
        retrospective.parent(),
        solution.parent(),
        rule.parent(),
        index.parent(),
        manifest.parent(),
    ]
    .into_iter()
    .flatten()
    {
        fs::create_dir_all(path).map_err(|error| format!("create dir failed: {error}"))?;
    }

    fs::write(&manifest, scope_manifest(scope))
        .map_err(|error| format!("write manifest failed: {error}"))?;
    fs::write(
        &retrospective,
        retrospective_markdown(scope, &task_id, goal),
    )
    .map_err(|error| format!("write retrospective failed: {error}"))?;
    fs::write(&solution, solution_markdown(scope, &task_id, goal))
        .map_err(|error| format!("write solution failed: {error}"))?;
    fs::write(&rule, rule_markdown(scope, &task_id, goal))
        .map_err(|error| format!("write rule failed: {error}"))?;
    fs::write(&index, light_index_markdown(scope, &task_id))
        .map_err(|error| format!("write index failed: {error}"))?;

    Ok(json_array(
        [manifest, retrospective, solution, rule, index]
            .into_iter()
            .map(|path| {
                format!(
                    "{{\"kind\":{},\"path\":{},\"scopeId\":{}}}",
                    json_string("knowledge-light-artifact"),
                    json_string(&path_to_repo_relative(&path)),
                    json_string(&scope.scope_id)
                )
            }),
    ))
}

fn scope_manifest(scope: &KnowledgeScope) -> String {
    format!(
        "{{\"kind\":\"UnrealWorkflowKnowledgeScope\",\"contractVersion\":1,\"scope\":{},\"heavyCachePolicy\":\"local-only\",\"gitPolicy\":\"markdown-and-light-index-only\"}}\n",
        scope.to_json()
    )
}

fn retrospective_markdown(scope: &KnowledgeScope, task_id: &str, goal: &str) -> String {
    format!(
        "# 任务复盘：{task_id}\n\n- scope: `{}`\n- 目标：{}\n- 结论：dry-run 或安全执行结束后沉淀，等待人工补充最终证据。\n",
        scope.scope_kind.as_str(),
        goal
    )
}

fn solution_markdown(scope: &KnowledgeScope, task_id: &str, goal: &str) -> String {
    format!(
        "# 方案草案：{task_id}\n\n- scope: `{}`\n- 目标：{}\n- 方案：记录本任务可复用的实现路径、约束和验证命令。\n",
        scope.scope_kind.as_str(),
        goal
    )
}

fn rule_markdown(scope: &KnowledgeScope, task_id: &str, goal: &str) -> String {
    format!(
        "# 规则候选：{task_id}\n\n- scope: `{}`\n- 来源目标：{}\n- 规则候选：同类任务应先确认 scope，再写入轻量知识库。\n",
        scope.scope_kind.as_str(),
        goal
    )
}

fn light_index_markdown(scope: &KnowledgeScope, task_id: &str) -> String {
    format!(
        "# 轻量索引\n\n- scope: `{}`\n- 最近任务：`{}`\n- 条目：复盘、方案草案、规则候选。\n",
        scope.scope_id, task_id
    )
}

fn team_root() -> PathBuf {
    if let Ok(path) = env::var("UWF_KNOWLEDGE_ROOT") {
        return PathBuf::from(path);
    }
    env::current_dir()
        .map(|cwd| {
            cwd.join("Plugins")
                .join("UEWorkflow")
                .join("UnrealWorkflowKnowledge")
        })
        .unwrap_or_else(|_| PathBuf::from("UnrealWorkflowKnowledge"))
}

fn source_root() -> Option<PathBuf> {
    if let Ok(path) = env::var("UWF_KB_SOURCE_ROOT") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
    }
    if let Ok(current_dir) = env::current_dir() {
        let worktree = current_dir
            .join(".plugin-worktrees")
            .join("UEWorkflow")
            .join("KnowledgeBase");
        if worktree.is_dir() {
            return Some(worktree);
        }
        if let Some(parent) = current_dir.parent() {
            let sibling = parent.join(source_project_name());
            if sibling.is_dir() {
                return Some(sibling);
            }
        }
    }
    None
}

fn source_project_name() -> String {
    ["UE5", "KnowledgeBaseMaker"].join("_")
}

fn sanitize_segment(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    sanitized.trim_matches('-').to_string()
}

fn path_to_repo_relative(path: &Path) -> String {
    if let Ok(cwd) = env::current_dir() {
        if let Ok(relative) = path.strip_prefix(cwd) {
            return relative.to_string_lossy().replace('\\', "/");
        }
    }
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_kb_command_group() {
        let contract = contract();
        assert_eq!(contract.id.command_group(), "uwf kb");
        assert!(contract
            .capabilities
            .iter()
            .any(|capability| capability.id == "scope.registry"));
    }

    #[test]
    fn resolves_project_plugin_scope_by_default() {
        let request = CommandRequest {
            project: Some("Neon".to_string()),
            primary: Some("AesWorld".to_string()),
            ..CommandRequest::default()
        };
        let scope = resolve_scope(&request);
        assert_eq!(scope.scope_kind, KnowledgeScopeKind::ProjectPlugin);
        assert!(scope_relative_path(&scope).contains("projects/neon/plugins/aesworld"));
    }

    #[test]
    fn dry_run_blocks_heavy_pipeline() {
        let request = CommandRequest {
            action: Some("generate".to_string()),
            scope: Some("engine_plugin".to_string()),
            primary: Some("Chaos".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("dry-run", &request);
        assert!(json.contains("\"status\":\"planned\""));
        assert!(json.contains("\"scopeKind\":\"engine_plugin\""));
        assert!(json.contains("SQLite/Pickle"));
    }

    #[test]
    fn execute_requires_confirmation_boundary() {
        let request = CommandRequest {
            action: Some("record-task".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("execute", &request);
        assert!(json.contains("\"status\":\"blocked\""));
        assert!(
            json.contains("\"requiredConfirmation\":\"UEWorkflow.KnowledgeBase.record-task.v1\"")
        );
    }

    #[test]
    fn execute_blocks_heavy_export() {
        let request = CommandRequest {
            action: Some("export".to_string()),
            confirmation: Some("UEWorkflow.KnowledgeBase.export.v1".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("execute", &request);
        assert!(json.contains("\"status\":\"blocked\""));
        assert!(json.contains("重型"));
    }

    #[test]
    fn record_task_writes_light_artifacts_to_scope() {
        let test_root = temp_knowledge_root("record-task");
        env::set_var("UWF_KNOWLEDGE_ROOT", &test_root);
        let request = CommandRequest {
            action: Some("record-task".to_string()),
            confirmation: Some("UEWorkflow.KnowledgeBase.record-task.v1".to_string()),
            task_id: Some("Task 001".to_string()),
            goal: Some("沉淀插件规则".to_string()),
            project: Some("Neon".to_string()),
            primary: Some("AesWorld".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("execute", &request);
        assert!(json.contains("\"status\":\"complete\""));
        assert!(json.contains("scope-manifest.json"));
        let scope_root = test_root.join("scopes/projects/neon/plugins/aesworld");
        let manifest = scope_root.join("scope-manifest.json");
        let retrospective = scope_root.join("retrospectives/task-001.md");
        let solution = scope_root.join("solutions/task-001.md");
        let rule = scope_root.join("rules/task-001.md");
        let index = scope_root.join("index/light-index.md");
        for path in [&manifest, &retrospective, &solution, &rule, &index] {
            assert!(path.is_file(), "{} should exist", path.display());
        }
        assert!(fs::read_to_string(&retrospective)
            .expect("retrospective should be readable")
            .contains("任务复盘"));
        assert!(fs::read_to_string(&solution)
            .expect("solution should be readable")
            .contains("方案草案"));
        assert!(fs::read_to_string(&rule)
            .expect("rule should be readable")
            .contains("规则候选"));

        let query_request = CommandRequest {
            action: Some("query".to_string()),
            confirmation: Some("UEWorkflow.KnowledgeBase.query.v1".to_string()),
            goal: Some("沉淀插件规则".to_string()),
            project: Some("Neon".to_string()),
            primary: Some("AesWorld".to_string()),
            ..CommandRequest::default()
        };
        let query_json = command_json("execute", &query_request);
        assert!(query_json.contains("\"status\":\"complete\""));
        assert!(query_json.contains("retrospectives/task-001.md"));
        assert!(query_json.contains("solutions/task-001.md"));
        assert!(query_json.contains("rules/task-001.md"));
        assert!(query_json.contains("\"scopeId\":\"project/Neon/plugin/AesWorld\""));
        let _ = fs::remove_dir_all(&test_root);
        env::remove_var("UWF_KNOWLEDGE_ROOT");
    }

    fn temp_knowledge_root(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "uwf-kb-test-{}-{}-{}",
            std::process::id(),
            label,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ))
    }
}
