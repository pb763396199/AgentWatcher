use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;
use serde_json::Value;
use uwf_core::{
    json_array, json_option, json_string, ArtifactRef, Capability, CommandOutcome, CommandRequest,
    CommandSpec, CommandStatus, DryRunPlan, DryRunStage, ModuleContract, ModuleId, SafetyLevel,
};

pub fn contract() -> ModuleContract {
    ModuleContract {
        id: ModuleId::DevFlow,
        capabilities: vec![
            Capability {
                id: "workspace.discovery",
                title: "工作区发现",
                description: "发现 UE 项目、插件、Host 项目和任务工作区，但本阶段只读。",
            },
            Capability {
                id: "task.lifecycle",
                title: "任务生命周期",
                description: "声明需求草案、任务创建、worktree、分支、提交、合并和清理的标准流程。",
            },
            Capability {
                id: "build.routing",
                title: "构建路由",
                description:
                    "区分任务 worktree 构建、主项目构建和构建策略检查，避免误跑耗时 UE 编译。",
            },
        ],
        commands: vec![
            CommandSpec {
                name: "status",
                summary: "输出开发流模块状态。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "capabilities",
                summary: "输出开发流能力清单。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "commands",
                summary: "输出开发流命令清单。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "schema",
                summary: "输出开发流结果契约。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "dry-run",
                summary: "生成开发任务预演，不创建 worktree、不编译 UE。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: true,
                enabled: true,
            },
            CommandSpec {
                name: "execute",
                summary: "执行清单声明的安全开发流动作；创建、编译、删除等动作默认阻断。",
                safety: SafetyLevel::Dangerous,
                supports_dry_run: true,
                enabled: true,
            },
            CommandSpec {
                name: "history",
                summary: "读取开发流历史记录。本阶段返回空集合。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "artifacts",
                summary: "读取开发流产物索引。本阶段返回空集合。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
        ],
        schema: "uwf.module.devflow.v1",
    }
}

pub fn command_json(command: &str, request: &CommandRequest) -> String {
    match command {
        "status" => status_json(command, request),
        "capabilities" => contract().result_json(
            command,
            "ready",
            "虚幻开发流能力清单已输出。",
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
            "未知开发流命令；请通过 commands 查看清单。",
            &format!("\"request\":{},\"blocked\":true", request.to_json()),
        ),
    }
}

fn status_json(command: &str, request: &CommandRequest) -> String {
    let snapshot = AdapterSnapshot::collect();
    let extra = format!(
        "\"request\":{},\"installed\":true,\"sideEffects\":[],\"devFlow\":{{\"workspaceDiscovery\":{},\"taskDiscovery\":{},\"junctionStatus\":{},\"buildRouting\":{},\"adapter\":{}}}",
        request.to_json(),
        snapshot.workspaces.to_json("workspaces"),
        snapshot.tasks.to_json("tasks"),
        snapshot.status.to_json("junctions"),
        build_routing_json(),
        snapshot.adapter_json()
    );
    contract().result_json(
        command,
        "ready",
        "虚幻开发流已加载；只读状态来自受控适配器，危险动作保持阻断。",
        &extra,
    )
}

fn commands_json(command: &str, request: &CommandRequest) -> String {
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"declaredActions\":{}",
        request.to_json(),
        declared_actions_json()
    );
    contract().result_json(command, "ready", "开发流命令清单已输出。", &extra)
}

fn schema_json(command: &str, request: &CommandRequest) -> String {
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"schemaDetail\":{{\"command\":\"uwf dev <status|capabilities|commands|schema|dry-run|execute|history|artifacts>\",\"request\":{},\"resultKind\":\"UnrealWorkflowModuleResult\",\"dangerousActionsRequireDryRun\":true,\"rawUnrealBuildDefault\":\"disabled\"}}",
        request.to_json(),
        json_string("CommandRequest")
    );
    contract().result_json(command, "ready", "开发流 schema 已输出。", &extra)
}

fn dry_run_json(command: &str, request: &CommandRequest) -> String {
    let action = normalize_action(request.action.as_deref());
    let plan = dry_run_plan(action, request);
    let artifact_task = request.task_id.as_deref().unwrap_or("draft");
    let outcome = CommandOutcome {
        status: CommandStatus::Planned,
        message: "开发流 dry-run 已生成；不会创建 worktree、不会切换 Junction、不会启动 UE 编译。"
            .to_string(),
        artifacts: vec![ArtifactRef {
            id: "devflow-dry-run".to_string(),
            kind: "dryRunPlan".to_string(),
            path: format!(
                "%USERPROFILE%/.unrealworkflow/tasks/{artifact_task}/devflow-dry-run.json"
            ),
            description: "开发流预演计划".to_string(),
        }],
        dry_run: Some(plan.clone()),
    };
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"action\":{},\"confirmationBoundary\":{},\"targetContext\":{},\"buildRouting\":{},\"dryRun\":{},\"outcome\":{}",
        request.to_json(),
        json_string(action),
        json_string(&confirmation_boundary(action)),
        target_context_json(request),
        build_routing_for_action_json(action),
        plan.to_json(),
        outcome.to_json()
    );
    contract().result_json(
        command,
        "planned",
        "开发流 dry-run 已完成；等待明确确认边界后才能执行允许动作。",
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

    if action == "create-task" {
        return execute_create_task_json(command, request);
    }

    if action == "switch" {
        return execute_switch_json(command, request);
    }

    if !is_safe_execute_action(action) {
        return contract().result_json(
            command,
            "blocked",
            "该开发流动作会创建、切换、合并、清理或编译；当前阶段保持阻断。",
            &format!(
                "\"request\":{},\"sideEffects\":[],\"blocked\":true,\"action\":{},\"blockedActions\":{}",
                request.to_json(),
                json_string(action),
                json_array(
                    blocked_actions(action)
                        .into_iter()
                        .map(|action| json_string(&action)),
                )
            ),
        );
    }

    let extra = match action {
        "read-status" => {
            let snapshot = AdapterSnapshot::collect();
            format!(
                "\"request\":{},\"sideEffects\":[],\"executedAction\":{},\"devFlow\":{{\"junctionStatus\":{},\"taskDiscovery\":{}}}",
                request.to_json(),
                json_string(action),
                snapshot.status.to_json("junctions"),
                snapshot.tasks.to_json("tasks")
            )
        }
        "list-tasks" => {
            let snapshot = AdapterSnapshot::collect();
            format!(
                "\"request\":{},\"sideEffects\":[],\"executedAction\":{},\"devFlow\":{{\"taskDiscovery\":{}}}",
                request.to_json(),
                json_string(action),
                snapshot.tasks.to_json("tasks")
            )
        }
        "build-check" => format!(
            "\"request\":{},\"sideEffects\":[],\"executedAction\":{},\"buildRouting\":{}",
            request.to_json(),
            json_string(action),
            build_routing_for_action_json(action)
        ),
        _ => unreachable!("safe execute action checked above"),
    };

    contract().result_json(
        command,
        "complete",
        "安全开发流动作已完成；未启动 UE 编译，未创建或删除任何 worktree。",
        &extra,
    )
}

fn execute_switch_json(command: &str, request: &CommandRequest) -> String {
    match switch_task_via_adapter(request) {
        Ok(switched) => {
            let extra = format!(
                "\"request\":{},\"sideEffects\":[{}],\"executedAction\":{},\"switchContext\":{}",
                request.to_json(),
                json_string("devflow-switch-junction"),
                json_string("switch"),
                switched.to_json(request)
            );
            contract().result_json(
                command,
                "complete",
                "DevFlow has executed switch through the controlled adapter; no UE build was started.",
                &extra,
            )
        }
        Err(error) => {
            let side_effects = if error.attempted_adapter {
                json_array([json_string("attempted-devflow-switch")])
            } else {
                "[]".to_string()
            };
            let extra = format!(
                "\"request\":{},\"sideEffects\":{},\"blocked\":{},\"action\":{},\"errorEvidence\":{}",
                request.to_json(),
                side_effects,
                error.status == "blocked",
                json_string("switch"),
                error.evidence_json()
            );
            contract().result_json(command, error.status, &error.message, &extra)
        }
    }
}

fn execute_create_task_json(command: &str, request: &CommandRequest) -> String {
    match create_task_via_adapter(request) {
        Ok(created) => {
            let create_side_effect = if created.reused_existing {
                "devflow-existing-task-reused"
            } else {
                "unreal-workflow-devflow-create-task"
            };
            let mut side_effects = vec![json_string(create_side_effect)];
            if created.workspace_binding_updated {
                side_effects.insert(0, json_string("devflow-workspace-binding-updated"));
            }
            let extra = format!(
                "\"request\":{},\"sideEffects\":{},\"executedAction\":{},\"devFlow\":{},\"targetContext\":{},\"buildRouting\":{}",
                request.to_json(),
                json_array(side_effects),
                json_string("create-task"),
                created.dev_flow_json(),
                created.target_context_json(request),
                build_routing_for_action_json("build-check")
            );
            contract().result_json(
                command,
                "complete",
                if created.reused_existing {
                    "DevFlow 已确认隔离任务 Host/worktree 已存在；继续生成 Provider 执行包，未启动 UE 编译。"
                } else {
                    "DevFlow 已通过受控适配器创建隔离任务 Host/worktree；未启动 UE 编译。"
                },
                &extra,
            )
        }
        Err(error) => {
            let side_effects = if error.attempted_adapter {
                json_array([json_string("attempted-devflow-create-task")])
            } else {
                "[]".to_string()
            };
            let extra = format!(
                "\"request\":{},\"sideEffects\":{},\"blocked\":{},\"action\":{},\"errorEvidence\":{}",
                request.to_json(),
                side_effects,
                error.status == "blocked",
                json_string("create-task"),
                error.evidence_json()
            );
            contract().result_json(command, error.status, &error.message, &extra)
        }
    }
}

fn history_json(command: &str, request: &CommandRequest) -> String {
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"historyRoot\":{},\"history\":[]",
        request.to_json(),
        json_string("%USERPROFILE%/.unrealworkflow/history/devflow")
    );
    contract().result_json(command, "ready", "开发流历史索引已输出。", &extra)
}

fn artifacts_json(command: &str, request: &CommandRequest) -> String {
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"artifactRoot\":{},\"artifacts\":[]",
        request.to_json(),
        json_string("%USERPROFILE%/.unrealworkflow/artifacts/devflow")
    );
    contract().result_json(command, "ready", "开发流产物索引已输出。", &extra)
}

fn dry_run_plan(action: &str, request: &CommandRequest) -> DryRunPlan {
    DryRunPlan {
        goal: request
            .goal
            .clone()
            .unwrap_or_else(|| format!("devflow action: {action}")),
        stages: dry_run_stages(action),
        blocked_actions: blocked_actions(action),
        confirmation_boundary: Some(confirmation_boundary(action)),
    }
}

fn dry_run_stages(action: &str) -> Vec<DryRunStage> {
    match action {
        "build-task" => vec![
            stage(
                "freeze-context",
                "冻结任务上下文",
                "锁定 workspace、taskId、primary plugin，避免串项目。",
                SafetyLevel::ReadOnly,
            ),
            stage(
                "route-build-task",
                "选择任务构建路由",
                "只允许从任务 worktree/Host 隔离环境出发；本阶段不启动 UBT。",
                SafetyLevel::ReadOnly,
            ),
            stage(
                "wait-confirmation",
                "等待确认",
                "真实 UE 编译仍处于默认禁用。",
                SafetyLevel::Dangerous,
            ),
        ],
        "build-project" => vec![
            stage(
                "freeze-context",
                "冻结项目上下文",
                "锁定主项目或当前路径，避免误用任务 Host。",
                SafetyLevel::ReadOnly,
            ),
            stage(
                "route-build-project",
                "选择主项目构建路由",
                "整合虚幻大师构建 wrapper 策略；本阶段只输出策略。",
                SafetyLevel::ReadOnly,
            ),
            stage(
                "block-raw-build",
                "阻断 raw UE build",
                "未经单独确认不得直接调用 UBT 或 Editor build。",
                SafetyLevel::Dangerous,
            ),
        ],
        "build-check" => vec![
            stage(
                "read-context",
                "读取上下文",
                "检查 workspace、taskId、project、primary plugin 是否足够路由。",
                SafetyLevel::ReadOnly,
            ),
            stage(
                "emit-policy",
                "输出构建策略",
                "只输出 build-task/build-project 应走的路由，不启动 UE 编译。",
                SafetyLevel::ReadOnly,
            ),
        ],
        "switch" => vec![
            stage(
                "read-current-context",
                "读取当前上下文",
                "读取当前 workspace、taskId、project、primary plugin，不访问任意 shell。",
                SafetyLevel::ReadOnly,
            ),
            stage(
                "preview-target-context",
                "预览目标上下文",
                "展示即将切换到的工作区和任务上下文；不会切换 Junction。",
                SafetyLevel::ReadOnly,
            ),
            stage(
                "wait-confirmation",
                "等待确认",
                "改变工作区或任务上下文前必须由 AgentWatcher 记录确认边界。",
                SafetyLevel::Bounded,
            ),
        ],
        "merge" | "cleanup" | "delete" => vec![
            stage(
                "read-task",
                "读取任务状态",
                "确认任务和分支身份。",
                SafetyLevel::ReadOnly,
            ),
            stage(
                "block-destructive",
                "阻断破坏动作",
                "合并、清理、删除必须进入单独确认流程。",
                SafetyLevel::Dangerous,
            ),
        ],
        _ => vec![
            stage(
                "freeze-context",
                "确认 UE 项目上下文",
                "在派发前确认 workspace、主项目、主插件、Host 项目和插件依赖；未知项必须标记待确认。",
                SafetyLevel::ReadOnly,
            ),
            stage(
                "plan-worktree",
                "规划 worktree/Host",
                "只生成任务创建方案，不创建目录、不切换 Junction。",
                SafetyLevel::ReadOnly,
            ),
            stage(
                "wait-confirmation",
                "等待确认",
                "后续 bounded execute 必须带确认边界和执行记录。",
                SafetyLevel::Bounded,
            ),
        ],
    }
}

fn target_context_json(request: &CommandRequest) -> String {
    let primary_paths = primary_path_items(request.primary_path.as_deref());
    let primary_scans = scan_primary_plugins(request, &primary_paths);
    let primary_path_resolution = primary_path_resolution(request, &primary_scans);
    let deps = target_dependency_items(request, &primary_scans);
    let resolved_workspace = request
        .workspace
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .and_then(|workspace| resolve_udf_workspace_for_request(request, workspace, true).ok());
    let effective_host_root = request
        .host_root
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            resolved_workspace
                .as_ref()
                .and_then(|workspace| workspace.hosts_root.clone())
        });
    let effective_main_project_result = effective_project_path(
        request.main_project.as_deref(),
        resolved_workspace
            .as_ref()
            .and_then(|workspace| workspace.default_project.as_ref()),
    );
    let effective_main_project = effective_main_project_result
        .as_ref()
        .ok()
        .and_then(|value| value.clone());
    let effective_host_root_json = effective_host_root
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let effective_main_project_json = effective_main_project
        .as_ref()
        .map(|path| path.to_string_lossy().to_string());
    let devflow_workspace = resolved_workspace
        .as_ref()
        .map(|workspace| workspace.name.as_str())
        .or_else(|| request.workspace.as_deref());
    let host_ready = effective_host_root.is_some() && effective_main_project.is_some();
    let primary_ready = if primary_paths.is_empty() {
        request
            .primary
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    } else {
        primary_scans.iter().all(|scan| scan.status == "ready")
            && primary_path_resolution.status != "blocked"
            && primary_path_resolution.status != "pending"
    };
    let missing = [
        (!host_ready).then_some("主项目/Host 根"),
        (!primary_ready).then_some("主插件"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let required_before_provider = !missing.is_empty();
    let host_message = if let Err(error) = effective_main_project_result {
        error.message
    } else if host_ready {
        "已由任务上下文确认主项目和 Host 根。".to_string()
    } else {
        "缺少主项目路径或 Host 根，请在任务创建面板补齐。".to_string()
    };
    let dep_status = if deps.is_empty() { "auto" } else { "ready" };
    let dep_message = if deps.is_empty() {
        "未填写，DevFlow 将按 .uplugin 与工程/引擎插件扫描自动识别。"
    } else if plugin_dependency_items(request.plugin_dependencies.as_deref()).is_empty() {
        "已从主插件 .uplugin 扫描依赖。"
    } else {
        "已合并主插件 .uplugin 扫描结果和任务上下文依赖覆盖。"
    };
    format!(
        "{{\"workspacePath\":{},\"devFlowWorkspace\":{},\"project\":{},\"primaryPlugin\":{},\"primaryPluginPath\":{},\"primaryPathResolution\":{},\"hostRoot\":{},\"mainProject\":{},\"hostProject\":{{\"status\":{},\"path\":{},\"hostRoot\":{},\"message\":{}}},\"pluginDependencies\":{{\"status\":{},\"count\":{},\"message\":{},\"items\":{}}},\"confirmationStage\":{},\"requiredBeforeProviderExecution\":{},\"missingBinding\":{},\"primaryPluginScans\":{}}}",
        json_option(request.workspace.as_deref()),
        json_option(devflow_workspace),
        json_option(request.project.as_deref()),
        json_option(request.primary.as_deref()),
        json_option(request.primary_path.as_deref()),
        primary_path_resolution.to_json(),
        json_option(effective_host_root_json.as_deref()),
        json_option(effective_main_project_json.as_deref()),
        json_string(if host_ready { "ready" } else { "pending" }),
        json_option(effective_main_project_json.as_deref()),
        json_option(effective_host_root_json.as_deref()),
        json_string(&host_message),
        json_string(dep_status),
        deps.len(),
        json_string(dep_message),
        json_array(deps.iter().map(DependencyScanItem::to_json)),
        json_string("任务创建上下文 / DevFlow dry-run"),
        required_before_provider,
        json_array(missing.into_iter().map(json_string)),
        json_array(primary_scans.iter().map(PrimaryPluginScan::to_json))
    )
}

fn plugin_dependency_items(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or("")
        .split([',', ';', '\n', '\r', '，', '；', '、'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn primary_path_items(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or("")
        .split([',', ';', '\n', '\r', '，', '；', '、'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn primary_plugin_items(value: &str) -> Vec<String> {
    value
        .split([',', ';', '\n', '\r', '，', '、'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[derive(Clone, Debug)]
struct PrimaryPathResolution {
    status: &'static str,
    message: String,
    devflow_plugin_path: Option<String>,
}

impl PrimaryPathResolution {
    fn to_json(&self) -> String {
        format!(
            "{{\"status\":{},\"message\":{},\"devFlowPluginPath\":{}}}",
            json_string(self.status),
            json_string(&self.message),
            json_option(self.devflow_plugin_path.as_deref())
        )
    }
}

fn primary_path_resolution(
    request: &CommandRequest,
    scans: &[PrimaryPluginScan],
) -> PrimaryPathResolution {
    if primary_path_items(request.primary_path.as_deref()).is_empty() {
        return PrimaryPathResolution {
            status: "not_required",
            message: "未填写主插件路径，DevFlow 将按 workspace 和 --primary 名称解析。".to_string(),
            devflow_plugin_path: None,
        };
    }
    let Some(workspace) = request
        .workspace
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return PrimaryPathResolution {
            status: "pending",
            message: "已填写主插件路径，但尚未选择 DevFlow workspace。".to_string(),
            devflow_plugin_path: None,
        };
    };
    let resolved = match resolve_udf_workspace_for_request(request, workspace, true) {
        Ok(value) => value,
        Err(error) => {
            return PrimaryPathResolution {
                status: "blocked",
                message: error.message,
                devflow_plugin_path: None,
            };
        }
    };
    let devflow_plugin_path = resolved
        .plugin_path
        .as_ref()
        .map(|path| path.display().to_string());
    if resolved.plugin_path.is_none() {
        return match workspace_binding_request(request, &resolved, scans) {
            Ok(binding) => PrimaryPathResolution {
                status: "will_register",
                message: format!(
                    "DevFlow workspace 尚未绑定默认主插件；execute 时将调用 DevFlow workspace add 登记 plugin_path={}，再创建任务。",
                    binding.plugin_path.display()
                ),
                devflow_plugin_path: Some(binding.plugin_path.display().to_string()),
            },
            Err(error) => PrimaryPathResolution {
                status: "blocked",
                message: error.message,
                devflow_plugin_path: None,
            },
        };
    }
    let (status, message) = match validate_primary_paths_against_workspace(&resolved, scans) {
        Ok(true) => (
            "locked",
            "主插件路径已和 DevFlow workspace.plugin_path 精确一致；执行时将交给 DevFlow 默认主插件解析。"
                .to_string(),
        ),
        Ok(false) => (
            "partial",
            "至少一个主插件路径与 DevFlow workspace.plugin_path 一致；其余主插件仍由 DevFlow --primary 解析。"
                .to_string(),
        ),
        Err(error) => ("blocked", error.message),
    };
    PrimaryPathResolution {
        status,
        message,
        devflow_plugin_path,
    }
}

#[derive(Clone, Debug)]
struct PrimaryPluginScan {
    status: &'static str,
    name: String,
    path: String,
    descriptor_path: Option<String>,
    modules: Vec<String>,
    dependencies: Vec<DependencyScanItem>,
    message: String,
}

impl PrimaryPluginScan {
    fn to_json(&self) -> String {
        format!(
            "{{\"status\":{},\"name\":{},\"path\":{},\"descriptorPath\":{},\"modules\":{},\"dependencies\":{},\"message\":{}}}",
            json_string(self.status),
            json_string(&self.name),
            json_string(&self.path),
            json_option(self.descriptor_path.as_deref()),
            json_array(self.modules.iter().map(|module| json_string(module))),
            json_array(self.dependencies.iter().map(DependencyScanItem::to_json)),
            json_string(&self.message)
        )
    }
}

#[derive(Clone, Debug)]
struct DependencyScanItem {
    name: String,
    source: String,
    path: Option<String>,
    reason: String,
}

impl DependencyScanItem {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"source\":{},\"path\":{},\"reason\":{}}}",
            json_string(&self.name),
            json_string(&self.source),
            json_option(self.path.as_deref()),
            json_string(&self.reason)
        )
    }
}

fn scan_primary_plugins(request: &CommandRequest, paths: &[String]) -> Vec<PrimaryPluginScan> {
    paths
        .iter()
        .map(|path| scan_primary_plugin(request, path))
        .collect()
}

fn scan_primary_plugin(request: &CommandRequest, raw_path: &str) -> PrimaryPluginScan {
    let path = PathBuf::from(raw_path);
    let display_path = path_to_json_string(&path);
    let Ok((plugin_dir, descriptor_path)) = find_uplugin_descriptor(&path) else {
        return PrimaryPluginScan {
            status: "missing",
            name: plugin_name_from_path(&path),
            path: display_path,
            descriptor_path: None,
            modules: Vec::new(),
            dependencies: Vec::new(),
            message: "未找到 .uplugin 描述文件，预演不能确认插件工程身份。".to_string(),
        };
    };
    let descriptor_text = match fs::read_to_string(&descriptor_path) {
        Ok(value) => value,
        Err(error) => {
            return PrimaryPluginScan {
                status: "blocked",
                name: plugin_name_from_path(&plugin_dir),
                path: path_to_json_string(&plugin_dir),
                descriptor_path: Some(path_to_json_string(&descriptor_path)),
                modules: Vec::new(),
                dependencies: Vec::new(),
                message: format!(".uplugin 读取失败：{}", error.kind()),
            };
        }
    };
    let descriptor = match serde_json::from_str::<Value>(&descriptor_text) {
        Ok(value) => value,
        Err(error) => {
            return PrimaryPluginScan {
                status: "blocked",
                name: plugin_name_from_path(&plugin_dir),
                path: path_to_json_string(&plugin_dir),
                descriptor_path: Some(path_to_json_string(&descriptor_path)),
                modules: Vec::new(),
                dependencies: Vec::new(),
                message: format!(".uplugin 不是合法 JSON：{error}"),
            };
        }
    };
    let name = descriptor_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(ToString::to_string)
        .unwrap_or_else(|| plugin_name_from_path(&plugin_dir));
    let modules = descriptor
        .get("Modules")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("Name").and_then(Value::as_str))
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default();
    let dependencies = descriptor
        .get("Plugins")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| item.get("Enabled").and_then(Value::as_bool).unwrap_or(true))
                .filter_map(|item| item.get("Name").and_then(Value::as_str))
                .map(|name| resolve_dependency_scan_item(request, name, "uplugin"))
                .collect()
        })
        .unwrap_or_default();
    PrimaryPluginScan {
        status: "ready",
        name,
        path: path_to_json_string(&plugin_dir),
        descriptor_path: Some(path_to_json_string(&descriptor_path)),
        modules,
        dependencies,
        message: "已读取 .uplugin，完成主插件身份和声明依赖扫描。".to_string(),
    }
}

fn find_uplugin_descriptor(path: &Path) -> Result<(PathBuf, PathBuf), ()> {
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("uplugin"))
    {
        let Some(parent) = path.parent() else {
            return Err(());
        };
        return path
            .is_file()
            .then(|| (parent.to_path_buf(), path.to_path_buf()))
            .ok_or(());
    }
    if !path.is_dir() {
        return Err(());
    }
    let entries = fs::read_dir(path).map_err(|_| ())?;
    for entry in entries.flatten() {
        let candidate = entry.path();
        if candidate
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("uplugin"))
        {
            return Ok((path.to_path_buf(), candidate));
        }
    }
    Err(())
}

fn plugin_name_from_path(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|value| value.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn target_dependency_items(
    request: &CommandRequest,
    primary_scans: &[PrimaryPluginScan],
) -> Vec<DependencyScanItem> {
    let mut items = Vec::new();
    for scan in primary_scans {
        for dependency in &scan.dependencies {
            if !items
                .iter()
                .any(|item: &DependencyScanItem| item.name == dependency.name)
            {
                items.push(dependency.clone());
            }
        }
    }
    for override_item in plugin_dependency_items(request.plugin_dependencies.as_deref()) {
        let (name, source, path) = parse_dependency_override(&override_item);
        if let Some(existing) = items.iter_mut().find(|item| item.name == name) {
            existing.source = source;
            existing.path = path;
            existing.reason = "任务上下文依赖覆盖".to_string();
        } else {
            items.push(DependencyScanItem {
                name,
                source,
                path,
                reason: "任务上下文依赖覆盖".to_string(),
            });
        }
    }
    items
}

fn resolve_dependency_scan_item(
    request: &CommandRequest,
    name: &str,
    reason: &str,
) -> DependencyScanItem {
    let override_items = plugin_dependency_items(request.plugin_dependencies.as_deref());
    if let Some(override_item) = override_items.iter().find(|item| {
        item.split_once('=')
            .is_some_and(|(key, _)| key.trim() == name)
    }) {
        let (name, source, path) = parse_dependency_override(override_item);
        return DependencyScanItem {
            name,
            source,
            path,
            reason: "任务上下文依赖覆盖".to_string(),
        };
    }
    let project_plugin = request
        .main_project
        .as_deref()
        .map(PathBuf::from)
        .map(|root| root.join("Plugins").join(name))
        .filter(|path| path.is_dir());
    if let Some(path) = project_plugin {
        return DependencyScanItem {
            name: name.to_string(),
            source: "project_plugin".to_string(),
            path: Some(path_to_json_string(&path)),
            reason: reason.to_string(),
        };
    }
    DependencyScanItem {
        name: name.to_string(),
        source: "engine_or_external".to_string(),
        path: None,
        reason: reason.to_string(),
    }
}

fn parse_dependency_override(value: &str) -> (String, String, Option<String>) {
    let Some((name, target)) = value.split_once('=') else {
        return (value.trim().to_string(), "override".to_string(), None);
    };
    let target = target.trim();
    let source = match target {
        "project" => "project_plugin".to_string(),
        "engine" => "engine_plugin".to_string(),
        other if other.contains(':') || other.contains('\\') || other.contains('/') => {
            "absolute_path".to_string()
        }
        other => other.to_string(),
    };
    let path = (source == "absolute_path").then(|| target.to_string());
    (name.trim().to_string(), source, path)
}

fn stage(id: &str, title: &str, summary: &str, safety: SafetyLevel) -> DryRunStage {
    DryRunStage {
        id: id.to_string(),
        title: title.to_string(),
        summary: summary.to_string(),
        safety,
        will_execute: false,
    }
}

fn normalize_action(action: Option<&str>) -> &'static str {
    match action.unwrap_or("create-task") {
        "read-status" | "status" => "read-status",
        "list-tasks" | "tasks" => "list-tasks",
        "read-workspaces" | "workspaces" => "read-workspaces",
        "create-task" | "task-create" | "create" => "create-task",
        "switch" | "switch-task" => "switch",
        "commit" => "commit",
        "merge" => "merge",
        "cleanup" => "cleanup",
        "delete" => "delete",
        "build-task" => "build-task",
        "build-project" => "build-project",
        "build-check" | "check-build" => "build-check",
        _ => "create-task",
    }
}

fn confirmation_boundary(action: &str) -> String {
    format!("UEWorkflow.DevFlow.{action}.v1")
}

fn is_safe_execute_action(action: &str) -> bool {
    matches!(
        action,
        "read-status" | "list-tasks" | "build-check" | "switch"
    )
}

fn blocked_actions(action: &str) -> Vec<String> {
    let mut actions = vec![
        "任意 shell".to_string(),
        "raw UE build".to_string(),
        "破坏性删除".to_string(),
    ];
    match action {
        "build-task" | "build-project" => {
            actions.push("未经单独确认启动 UE 编译".to_string());
        }
        "create-task" => {
            actions.push("未经 bounded execute 创建 worktree/Host".to_string());
        }
        "switch" => {
            actions.push("直接切换 Junction".to_string());
        }
        "merge" => {
            actions.push("未经策略确认合并分支".to_string());
        }
        "cleanup" | "delete" => {
            actions.push("未经审计清理 worktree 或分支".to_string());
        }
        _ => {}
    }
    actions
}

fn declared_actions_json() -> String {
    let actions = [
        "read-status",
        "list-tasks",
        "read-workspaces",
        "create-task",
        "switch",
        "commit",
        "merge",
        "cleanup",
        "delete",
        "build-task",
        "build-project",
        "build-check",
    ];
    json_array(actions.into_iter().map(|action| {
        format!(
            "{{\"name\":{},\"safeExecute\":{},\"confirmationBoundary\":{}}}",
            json_string(action),
            is_safe_execute_action(action),
            json_string(&confirmation_boundary(action))
        )
    }))
}

fn build_routing_json() -> String {
    json_array(
        ["build-task", "build-project", "build-check"]
            .into_iter()
            .map(build_routing_for_action_json),
    )
}

fn build_routing_for_action_json(action: &str) -> String {
    match action {
        "build-task" => format!(
            "{{\"mode\":{},\"startsFrom\":{},\"buildPolicy\":{},\"rawUnrealBuild\":{},\"defaultExecute\":{}}}",
            json_string("build-task"),
            json_string("task worktree / Host isolated environment"),
            json_string("task host build route"),
            json_string("disabled"),
            json_string("blocked")
        ),
        "build-project" => format!(
            "{{\"mode\":{},\"startsFrom\":{},\"buildPolicy\":{},\"rawUnrealBuild\":{},\"defaultExecute\":{}}}",
            json_string("build-project"),
            json_string("main project or current project path"),
            json_string("UnrealMaster project wrapper route"),
            json_string("disabled"),
            json_string("blocked")
        ),
        _ => format!(
            "{{\"mode\":{},\"startsFrom\":{},\"buildPolicy\":{},\"rawUnrealBuild\":{},\"defaultExecute\":{}}}",
            json_string("build-check"),
            json_string("declared task or project context"),
            json_string("strategy check only"),
            json_string("disabled"),
            json_string("allowed")
        ),
    }
}

#[derive(Debug)]
struct CreateTaskFailure {
    status: &'static str,
    message: String,
    attempted_adapter: bool,
    stdout: Option<String>,
    stderr: Option<String>,
    exit_code: Option<i32>,
}

impl CreateTaskFailure {
    fn blocked(message: impl Into<String>) -> Self {
        Self {
            status: "blocked",
            message: message.into(),
            attempted_adapter: false,
            stdout: None,
            stderr: None,
            exit_code: None,
        }
    }

    fn failed(message: impl Into<String>, output: Option<&std::process::Output>) -> Self {
        Self {
            status: "failed",
            message: message.into(),
            attempted_adapter: output.is_some(),
            stdout: output.map(|value| short_evidence(&String::from_utf8_lossy(&value.stdout))),
            stderr: output.map(|value| short_evidence(&String::from_utf8_lossy(&value.stderr))),
            exit_code: output.and_then(|value| value.status.code()),
        }
    }

    fn evidence_json(&self) -> String {
        format!(
            "{{\"adapter\":{},\"stdout\":{},\"stderr\":{},\"exitCode\":{}}}",
            json_string("DevFlowAdapter"),
            json_option(self.stdout.as_deref()),
            json_option(self.stderr.as_deref()),
            self.exit_code
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string())
        )
    }
}

#[derive(Clone, Debug)]
struct CreatedTaskContext {
    workspace: String,
    task_id: String,
    task_uid: Option<String>,
    branch: Option<String>,
    based_on: Option<String>,
    host_path: String,
    host_project_path: String,
    primary_plugin: String,
    primary_plugins: Vec<String>,
    primary_worktree_path: String,
    source_repo: Option<String>,
    dependency_plugins: Vec<DependencyPluginSummary>,
    workspace_binding_updated: bool,
    reused_existing: bool,
}

impl CreatedTaskContext {
    fn dev_flow_json(&self) -> String {
        format!(
            "{{\"adapter\":{{\"status\":\"integrated\",\"oldNamesHidden\":true}},\"workspaceBindingUpdated\":{},\"reusedExistingTask\":{},\"createdTask\":{},\"dependencyPlugins\":{}}}",
            self.workspace_binding_updated,
            self.reused_existing,
            self.created_task_json(),
            json_array(self.dependency_plugins.iter().map(DependencyPluginSummary::to_json))
        )
    }

    fn created_task_json(&self) -> String {
        format!(
            "{{\"workspace\":{},\"taskId\":{},\"taskUid\":{},\"branch\":{},\"basedOn\":{},\"hostPath\":{},\"hostProjectPath\":{},\"primaryPlugin\":{},\"primaryPlugins\":{},\"primaryWorktreePath\":{},\"sourceRepo\":{}}}",
            json_string(&self.workspace),
            json_string(&self.task_id),
            json_option(self.task_uid.as_deref()),
            json_option(self.branch.as_deref()),
            json_option(self.based_on.as_deref()),
            json_string(&self.host_path),
            json_string(&self.host_project_path),
            json_string(&self.primary_plugin),
            json_array(self.primary_plugins.iter().map(|plugin| json_string(plugin))),
            json_string(&self.primary_worktree_path),
            json_option(self.source_repo.as_deref())
        )
    }

    fn target_context_json(&self, request: &CommandRequest) -> String {
        let dependency_message = format!(
            "已从 DevFlow 任务元数据确认 {} 个依赖插件。",
            self.dependency_plugins.len()
        );
        let workspace_path = if self.primary_plugins.len() > 1 {
            &self.host_path
        } else {
            &self.primary_worktree_path
        };
        format!(
            "{{\"workspacePath\":{},\"workspace\":{},\"taskId\":{},\"project\":{},\"primaryPlugin\":{},\"primaryPlugins\":{},\"hostProject\":{{\"status\":\"ready\",\"path\":{},\"message\":{}}},\"pluginDependencies\":{{\"status\":\"ready\",\"count\":{},\"message\":{},\"items\":{}}},\"confirmationStage\":{},\"requiredBeforeProviderExecution\":false,\"mustConfirmBeforeProviderExecution\":false}}",
            json_string(workspace_path),
            json_string(&self.workspace),
            json_string(&self.task_id),
            json_option(request.project.as_deref()),
            json_string(&self.primary_plugin),
            json_array(self.primary_plugins.iter().map(|plugin| json_string(plugin))),
            json_string(&self.host_project_path),
            json_string("DevFlow 已创建隔离 Host 项目，Provider 必须从该任务 worktree 工作。"),
            self.dependency_plugins.len(),
            json_string(&dependency_message),
            json_array(self.dependency_plugins.iter().map(DependencyPluginSummary::to_json)),
            json_string("DevFlow create-task 已确认")
        )
    }
}

#[derive(Clone, Debug)]
struct SwitchedTaskContext {
    workspace: String,
    requested_workspace: String,
    task_id: String,
    task_ref: String,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
}

impl SwitchedTaskContext {
    fn to_json(&self, request: &CommandRequest) -> String {
        format!(
            "{{\"workspace\":{},\"requestedWorkspace\":{},\"taskId\":{},\"taskRef\":{},\"project\":{},\"primary\":{},\"junctionSwitch\":{},\"gitMutation\":false,\"rawUnrealBuild\":{},\"adapter\":{{\"status\":\"integrated\",\"oldNamesHidden\":true,\"stdout\":{},\"stderr\":{},\"exitCode\":{}}}}}",
            json_string(&self.workspace),
            json_string(&self.requested_workspace),
            json_string(&self.task_id),
            json_string(&self.task_ref),
            json_option(request.project.as_deref()),
            json_option(request.primary.as_deref()),
            json_string("performed"),
            json_string("disabled"),
            json_option((!self.stdout.is_empty()).then_some(self.stdout.as_str())),
            json_option((!self.stderr.is_empty()).then_some(self.stderr.as_str())),
            self.exit_code
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".to_string())
        )
    }
}

#[derive(Clone, Debug)]
struct DependencyPluginSummary {
    name: String,
    source: String,
    source_path: Option<String>,
}

impl DependencyPluginSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"name\":{},\"source\":{},\"sourcePath\":{}}}",
            json_string(&self.name),
            json_string(&self.source),
            json_option(self.source_path.as_deref())
        )
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct UdfConfig {
    hosts_root: Option<PathBuf>,
    default_project: Option<PathBuf>,
    engine_path: Option<PathBuf>,
    plugin_path: Option<PathBuf>,
    plugins_root: Option<PathBuf>,
    #[serde(default)]
    workspaces: HashMap<String, UdfWorkspaceConfig>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct UdfWorkspaceConfig {
    hosts_root: Option<PathBuf>,
    plugin_path: Option<PathBuf>,
    default_project: Option<PathBuf>,
    engine_path: Option<PathBuf>,
    plugins_root: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
struct ResolvedUdfWorkspace {
    name: String,
    hosts_root: Option<PathBuf>,
    default_project: Option<PathBuf>,
    engine_path: Option<PathBuf>,
    plugins_root: Option<PathBuf>,
    plugin_path: Option<PathBuf>,
}

#[derive(Clone, Debug)]
struct PrimaryAdapterSelection {
    names: Vec<String>,
    pass_primary_arg: bool,
}

#[derive(Clone, Debug)]
struct WorkspaceBindingRequest {
    project: PathBuf,
    hosts_root: PathBuf,
    plugins_root: PathBuf,
    plugin_path: PathBuf,
    engine_path: Option<PathBuf>,
}

fn create_task_via_adapter(
    request: &CommandRequest,
) -> Result<CreatedTaskContext, CreateTaskFailure> {
    let goal = required_request_value(request.goal.as_deref(), "任务说明 goal")?;
    let task_id = required_request_value(request.task_id.as_deref(), "任务 ID taskId")?;
    let workspace_input = required_request_value(request.workspace.as_deref(), "工作区 workspace")?;
    let (resolved, workspace_binding_updated) =
        ensure_udf_workspace_binding(request, workspace_input)?;
    let primary_selection = primary_plugins_for_adapter(request, &resolved)?;
    let primary_plugins = primary_selection.names;
    if !valid_udf_task_id(task_id) {
        return Err(CreateTaskFailure::blocked(format!(
            "任务 ID '{task_id}' 不能交给 DevFlow 创建任务；请使用小写英文、数字和连字符。"
        )));
    }
    if primary_plugins.iter().any(|plugin| {
        plugin
            .chars()
            .any(|ch| ch.is_control() || ch == '/' || ch == '\\')
    }) {
        return Err(CreateTaskFailure::blocked(
            "主插件 primary 支持逗号分隔多个插件名，但不能包含路径或控制字符。",
        ));
    }

    let mut create_args = vec![
        "create".to_string(),
        clipped_goal(goal).to_string(),
        "--id".to_string(),
        task_id.to_string(),
        "--workspace".to_string(),
        resolved.name.clone(),
        "--prompt".to_string(),
        goal.to_string(),
        "--yes".to_string(),
    ];
    if primary_selection.pass_primary_arg {
        create_args.push("--primary".to_string());
        create_args.push(primary_plugins.join(","));
    }
    for dep_override in plugin_dependency_items(request.plugin_dependencies.as_deref()) {
        create_args.push("--override-dep".to_string());
        create_args.push(dep_override);
    }
    let create_output = run_fixed_adapter_command(&create_args).map_err(|error| {
        CreateTaskFailure::failed(
            format!(
                "DevFlowAdapter 不可用，无法创建隔离任务环境：{}",
                error.kind()
            ),
            None,
        )
    })?;

    let reused_existing = if !create_output.status.success() {
        let body = joined_output(&create_output);
        if create_failure_means_task_exists(&body) {
            true
        } else {
            return Err(CreateTaskFailure::failed(
                format!(
                    "DevFlowAdapter 拒绝创建任务；不会派发 Provider。{}",
                    short_evidence(&body)
                ),
                Some(&create_output),
            ));
        }
    } else {
        false
    };

    let list_output = read_task_list_from_adapter(&resolved.name)?;
    created_task_from_list_json(
        task_id,
        &primary_plugins.join(","),
        &resolved,
        &String::from_utf8_lossy(&list_output.stdout),
        workspace_binding_updated,
        reused_existing,
    )
}

fn switch_task_via_adapter(
    request: &CommandRequest,
) -> Result<SwitchedTaskContext, CreateTaskFailure> {
    let workspace_input =
        required_request_value_for_action(request.workspace.as_deref(), "workspace", "switch")?;
    let task_id =
        required_request_value_for_action(request.task_id.as_deref(), "taskId", "switch")?;
    if !valid_udf_task_id(task_id) {
        return Err(CreateTaskFailure::blocked(format!(
            "switch taskId must be a DevFlow task id made of lowercase letters, digits, and '-': {task_id}"
        )));
    }
    let resolved = resolve_udf_workspace_for_request(request, workspace_input, false)?;
    let task_ref = format!("{}/{}", resolved.name, task_id);
    let args = vec!["switch".to_string(), task_ref.clone()];
    let output = run_fixed_adapter_command(&args).map_err(|error| {
        CreateTaskFailure::failed(
            format!(
                "DevFlowAdapter is not available; cannot execute switch: {}",
                error.kind()
            ),
            None,
        )
    })?;
    if !output.status.success() {
        return Err(CreateTaskFailure::failed(
            format!(
                "DevFlowAdapter rejected switch; no Provider fallback will run. {}",
                short_evidence(&joined_output(&output))
            ),
            Some(&output),
        ));
    }
    Ok(SwitchedTaskContext {
        workspace: resolved.name,
        requested_workspace: workspace_input.to_string(),
        task_id: task_id.to_string(),
        task_ref,
        stdout: short_evidence(&String::from_utf8_lossy(&output.stdout)),
        stderr: short_evidence(&String::from_utf8_lossy(&output.stderr)),
        exit_code: output.status.code(),
    })
}

fn create_failure_means_task_exists(body: &str) -> bool {
    let body = body.to_lowercase();
    body.contains("任务已存在")
        || body.contains("task already exists")
        || body.contains("already exists")
}

fn read_task_list_from_adapter(workspace: &str) -> Result<std::process::Output, CreateTaskFailure> {
    let list_output =
        run_fixed_adapter_command(&["list", "--workspace", workspace, "--format", "json"])
            .map_err(|error| {
                CreateTaskFailure::failed(
                    format!(
                        "DevFlow 任务可能已创建，但无法读取任务元数据：{}",
                        error.kind()
                    ),
                    None,
                )
            })?;
    if !list_output.status.success() {
        return Err(CreateTaskFailure::failed(
            "DevFlow 任务可能已创建，但 list --format json 返回失败；为避免串项目，Provider 不会启动。",
            Some(&list_output),
        ));
    }
    Ok(list_output)
}

fn ensure_udf_workspace_binding(
    request: &CommandRequest,
    workspace_input: &str,
) -> Result<(ResolvedUdfWorkspace, bool), CreateTaskFailure> {
    let primary_paths = primary_path_items(request.primary_path.as_deref());
    let mut resolved =
        resolve_udf_workspace_for_request(request, workspace_input, !primary_paths.is_empty())?;
    if primary_paths.is_empty() {
        return Ok((resolved, false));
    }
    let scans = scan_primary_plugins(request, &primary_paths);
    if resolved.plugin_path.is_some() {
        validate_primary_paths_against_workspace(&resolved, &scans)?;
    }
    let binding = workspace_binding_request(request, &resolved, &scans)?;
    if workspace_binding_matches(&resolved, &binding) {
        return Ok((resolved, false));
    }
    let mut args = vec![
        "workspace".to_string(),
        "add".to_string(),
        resolved.name.clone(),
        "--project".to_string(),
        binding.project.to_string_lossy().to_string(),
        "--hosts-root".to_string(),
        binding.hosts_root.to_string_lossy().to_string(),
        "--plugins-root".to_string(),
        binding.plugins_root.to_string_lossy().to_string(),
        "--plugin-path".to_string(),
        binding.plugin_path.to_string_lossy().to_string(),
    ];
    if let Some(engine_path) = binding.engine_path.as_ref() {
        args.push("--engine-path".to_string());
        args.push(engine_path.to_string_lossy().to_string());
    }
    args.push("--yes".to_string());
    let output = run_fixed_adapter_command(&args).map_err(|error| {
        CreateTaskFailure::failed(
            format!(
                "DevFlowAdapter 不可用，无法登记 workspace 主插件路径：{}",
                error.kind()
            ),
            None,
        )
    })?;
    if !output.status.success() {
        return Err(CreateTaskFailure::failed(
            format!(
                "DevFlowAdapter 拒绝登记 workspace 主插件路径；不会创建任务。{}",
                short_evidence(&joined_output(&output))
            ),
            Some(&output),
        ));
    }
    resolved.hosts_root = Some(binding.hosts_root);
    resolved.default_project = Some(binding.project);
    resolved.plugins_root = Some(binding.plugins_root);
    resolved.plugin_path = Some(binding.plugin_path);
    resolved.engine_path = binding.engine_path;
    Ok((resolved, true))
}

fn primary_plugins_for_adapter(
    request: &CommandRequest,
    resolved: &ResolvedUdfWorkspace,
) -> Result<PrimaryAdapterSelection, CreateTaskFailure> {
    let primary_paths = primary_path_items(request.primary_path.as_deref());
    if !primary_paths.is_empty() {
        let scans = scan_primary_plugins(request, &primary_paths);
        if let Some(blocked) = scans.iter().find(|scan| scan.status != "ready") {
            return Err(CreateTaskFailure::blocked(format!(
                "主插件集路径无法确认：{}。{}",
                blocked.path, blocked.message
            )));
        }
        let omit_primary_arg = validate_primary_paths_against_workspace(resolved, &scans)?;
        let names = scans
            .iter()
            .map(|scan| scan.name.clone())
            .filter(|name| !name.trim().is_empty())
            .collect::<Vec<_>>();
        if !names.is_empty() {
            return Ok(PrimaryAdapterSelection {
                names,
                pass_primary_arg: !omit_primary_arg,
            });
        }
    }
    let primary = required_request_value(request.primary.as_deref(), "主插件 primary")?;
    let primary_plugins = primary_plugin_items(primary);
    if primary_plugins.is_empty() {
        return Err(CreateTaskFailure::blocked(
            "create-task 缺少主插件集；未执行任何外部动作。",
        ));
    }
    Ok(PrimaryAdapterSelection {
        names: primary_plugins,
        pass_primary_arg: true,
    })
}

fn validate_primary_paths_against_workspace(
    resolved: &ResolvedUdfWorkspace,
    scans: &[PrimaryPluginScan],
) -> Result<bool, CreateTaskFailure> {
    if scans.is_empty() {
        return Ok(false);
    }
    let Some(plugin_path) = resolved.plugin_path.as_ref() else {
        return Err(CreateTaskFailure::blocked(
            "已填写主插件路径，但 DevFlow workspace 没有配置精确 plugin_path。UWF 不会把路径降级成插件名去全局扫描；请先用 unrealdevflow workspace add/configure 把该路径登记为 workspace 的 plugin_path。",
        ));
    };
    let plugin_path_key = normalized_path_key(plugin_path);
    let exact_matches = scans
        .iter()
        .filter(|scan| normalized_path_key(Path::new(&scan.path)) == plugin_path_key)
        .count();
    if scans.len() == 1 && exact_matches == 1 {
        return Ok(true);
    }
    if exact_matches == 0 {
        return Err(CreateTaskFailure::blocked(format!(
            "主插件路径与 DevFlow workspace.plugin_path 不一致；为避免同名 .uplugin 误识别，已阻断。DevFlow plugin_path={}，任务主插件路径={}",
            plugin_path.display(),
            scans.iter().map(|scan| scan.path.as_str()).collect::<Vec<_>>().join(", ")
        )));
    }
    Ok(false)
}

fn workspace_binding_request(
    request: &CommandRequest,
    resolved: &ResolvedUdfWorkspace,
    scans: &[PrimaryPluginScan],
) -> Result<WorkspaceBindingRequest, CreateTaskFailure> {
    if scans.len() != 1 {
        return Err(CreateTaskFailure::blocked(
            "DevFlow workspace 只能自动绑定一个默认主插件路径；多主插件任务请先在 DevFlow 中配置 workspace.plugin_path，再用 --primary 名称声明其余主插件。",
        ));
    }
    let scan = &scans[0];
    if scan.status != "ready" {
        return Err(CreateTaskFailure::blocked(format!(
            "主插件路径无法确认：{}。{}",
            scan.path, scan.message
        )));
    }
    let project = effective_project_path(
        request.main_project.as_deref(),
        resolved.default_project.as_ref(),
    )?
    .ok_or_else(|| {
        CreateTaskFailure::blocked(
            "create-task 缺少有效 UE 主项目目录；请填写包含 .uproject 的目录，或先在 DevFlow workspace 中登记 default_project。",
        )
    })?;
    let hosts_root = request
        .host_root
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| resolved.hosts_root.clone())
        .ok_or_else(|| {
            CreateTaskFailure::blocked(
                "create-task 缺少 Host 根目录；请填写 hostRoot，或先在 DevFlow workspace 中登记 hosts_root。",
            )
        })?;
    let plugin_path = PathBuf::from(&scan.path);
    let plugins_root = resolved
        .plugins_root
        .clone()
        .or_else(|| plugin_path.parent().map(Path::to_path_buf))
        .ok_or_else(|| {
            CreateTaskFailure::blocked("主插件路径没有父目录，无法推导 DevFlow plugins_root。")
        })?;
    Ok(WorkspaceBindingRequest {
        project,
        hosts_root,
        plugins_root,
        plugin_path,
        engine_path: resolved.engine_path.clone(),
    })
}

fn effective_project_path(
    requested: Option<&str>,
    workspace_default: Option<&PathBuf>,
) -> Result<Option<PathBuf>, CreateTaskFailure> {
    let requested_path = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);
    match (requested_path, workspace_default) {
        (Some(path), _) if !contains_uproject(&path) => Err(CreateTaskFailure::blocked(format!(
            "填写的主项目路径不是 UE 项目目录，未找到 .uproject：{}。UWF 不会回退到其他 DevFlow workspace，避免把任务串到错误项目。",
            path.display()
        ))),
        (Some(path), _) => Ok(Some(path)),
        (None, Some(default_project)) => Ok(Some(default_project.clone())),
        (None, None) => Ok(None),
    }
}

fn contains_uproject(path: &Path) -> bool {
    path.read_dir()
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .any(|entry_path| {
            entry_path.is_file()
                && entry_path
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("uproject"))
        })
}

fn required_request_value<'a>(
    value: Option<&'a str>,
    label: &str,
) -> Result<&'a str, CreateTaskFailure> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(CreateTaskFailure::blocked(format!(
            "create-task 缺少 {label}；未执行任何外部动作。"
        )));
    };
    Ok(value)
}

fn required_request_value_for_action<'a>(
    value: Option<&'a str>,
    label: &str,
    action: &str,
) -> Result<&'a str, CreateTaskFailure> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Err(CreateTaskFailure::blocked(format!(
            "{action} missing {label}; no external action was executed."
        )));
    };
    Ok(value)
}

fn valid_udf_task_id(task_id: &str) -> bool {
    let mut chars = task_id.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}

fn clipped_goal(goal: &str) -> &str {
    goal.char_indices()
        .nth(240)
        .map(|(index, _)| goal[..index].trim())
        .filter(|value| !value.is_empty())
        .unwrap_or(goal)
}

fn resolve_udf_workspace(workspace_input: &str) -> Result<ResolvedUdfWorkspace, CreateTaskFailure> {
    let config = read_udf_config();
    if let Some(config) = config.as_ref() {
        if let Some(workspace) = config.workspaces.get(workspace_input) {
            return Ok(ResolvedUdfWorkspace {
                name: workspace_input.to_string(),
                hosts_root: workspace
                    .hosts_root
                    .clone()
                    .or_else(|| config.hosts_root.clone()),
                default_project: workspace
                    .default_project
                    .clone()
                    .or_else(|| config.default_project.clone()),
                engine_path: workspace
                    .engine_path
                    .clone()
                    .or_else(|| config.engine_path.clone()),
                plugins_root: workspace
                    .plugins_root
                    .clone()
                    .or_else(|| config.plugins_root.clone()),
                plugin_path: workspace
                    .plugin_path
                    .clone()
                    .or_else(|| config.plugin_path.clone()),
            });
        }
        if is_path_like(workspace_input) {
            let input_path = PathBuf::from(workspace_input);
            for (name, workspace) in &config.workspaces {
                if workspace_matches_path(workspace, config, &input_path) {
                    return Ok(ResolvedUdfWorkspace {
                        name: name.clone(),
                        hosts_root: workspace
                            .hosts_root
                            .clone()
                            .or_else(|| config.hosts_root.clone()),
                        default_project: workspace
                            .default_project
                            .clone()
                            .or_else(|| config.default_project.clone()),
                        engine_path: workspace
                            .engine_path
                            .clone()
                            .or_else(|| config.engine_path.clone()),
                        plugins_root: workspace
                            .plugins_root
                            .clone()
                            .or_else(|| config.plugins_root.clone()),
                        plugin_path: workspace
                            .plugin_path
                            .clone()
                            .or_else(|| config.plugin_path.clone()),
                    });
                }
            }
            return Err(CreateTaskFailure::blocked(format!(
                "工作区路径未在 DevFlow 配置中找到匹配项：{workspace_input}"
            )));
        }
    } else if is_path_like(workspace_input) {
        return Err(CreateTaskFailure::blocked(
            "workspace 是路径，但未找到 DevFlow 配置，无法确定 DevFlow 工作区名。",
        ));
    }

    Ok(ResolvedUdfWorkspace {
        name: workspace_input.to_string(),
        hosts_root: config.as_ref().and_then(|value| value.hosts_root.clone()),
        default_project: config
            .as_ref()
            .and_then(|value| value.default_project.clone()),
        engine_path: config.as_ref().and_then(|value| value.engine_path.clone()),
        plugins_root: config.as_ref().and_then(|value| value.plugins_root.clone()),
        plugin_path: config.as_ref().and_then(|value| value.plugin_path.clone()),
    })
}

fn resolve_udf_workspace_for_request(
    request: &CommandRequest,
    workspace_input: &str,
    can_register_binding: bool,
) -> Result<ResolvedUdfWorkspace, CreateTaskFailure> {
    let mut resolved = resolve_udf_workspace(workspace_input)?;
    retarget_workspace_for_explicit_project(request, &mut resolved, can_register_binding)?;
    Ok(resolved)
}

fn retarget_workspace_for_explicit_project(
    request: &CommandRequest,
    resolved: &mut ResolvedUdfWorkspace,
    can_register_binding: bool,
) -> Result<(), CreateTaskFailure> {
    let Some(requested_project) = explicit_project_path(request)? else {
        return Ok(());
    };
    let Some(current_project) = resolved.default_project.as_ref() else {
        return Ok(());
    };
    if paths_equal(current_project, &requested_project) {
        return Ok(());
    }
    if !can_register_binding {
        return Err(CreateTaskFailure::blocked(format!(
            "任务填写的主项目是 {}，但匹配到的 DevFlow workspace '{}' 绑定的是 {}。为避免串项目，请填写主插件路径，让 UWF 为该主项目登记独立 workspace。",
            requested_project.display(),
            resolved.name,
            current_project.display()
        )));
    }
    let config = read_udf_config();
    let new_name = select_workspace_name_for_project(config.as_ref(), &requested_project);
    if let Some(config) = config.as_ref() {
        if let Some(workspace) = config.workspaces.get(&new_name) {
            *resolved = resolved_workspace_from_config(&new_name, workspace, config);
            return Ok(());
        }
    }
    resolved.name = new_name;
    resolved.default_project = Some(requested_project);
    resolved.plugin_path = None;
    Ok(())
}

fn explicit_project_path(request: &CommandRequest) -> Result<Option<PathBuf>, CreateTaskFailure> {
    effective_project_path(request.main_project.as_deref(), None)
}

fn select_workspace_name_for_project(config: Option<&UdfConfig>, project: &Path) -> String {
    let preferred = suggest_devflow_workspace_name(project);
    let Some(config) = config else {
        return preferred;
    };
    match config.workspaces.get(&preferred) {
        Some(workspace) if !workspace_project_matches(workspace, config, project) => {}
        _ => return preferred,
    }
    if let Some((name, _)) = config
        .workspaces
        .iter()
        .find(|(_, workspace)| workspace_project_matches(workspace, config, project))
    {
        return name.clone();
    }
    (2..)
        .map(|index| format!("{preferred}-{index}"))
        .find(|candidate| !config.workspaces.contains_key(candidate))
        .unwrap_or(preferred)
}

fn suggest_devflow_workspace_name(project: &Path) -> String {
    let leaf = project
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("dev");
    let root = project
        .parent()
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|value| value.to_str())
        .unwrap_or(leaf);
    sanitize_workspace_name(&format!("{root}-{leaf}"))
}

fn sanitize_workspace_name(raw: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "default".to_string()
    } else {
        out
    }
}

fn workspace_project_matches(
    workspace: &UdfWorkspaceConfig,
    root: &UdfConfig,
    project: &Path,
) -> bool {
    workspace
        .default_project
        .as_ref()
        .or(root.default_project.as_ref())
        .is_some_and(|candidate| paths_equal(candidate, project))
}

fn resolved_workspace_from_config(
    name: &str,
    workspace: &UdfWorkspaceConfig,
    root: &UdfConfig,
) -> ResolvedUdfWorkspace {
    ResolvedUdfWorkspace {
        name: name.to_string(),
        hosts_root: workspace
            .hosts_root
            .clone()
            .or_else(|| root.hosts_root.clone()),
        default_project: workspace
            .default_project
            .clone()
            .or_else(|| root.default_project.clone()),
        engine_path: workspace
            .engine_path
            .clone()
            .or_else(|| root.engine_path.clone()),
        plugins_root: workspace
            .plugins_root
            .clone()
            .or_else(|| root.plugins_root.clone()),
        plugin_path: workspace
            .plugin_path
            .clone()
            .or_else(|| root.plugin_path.clone()),
    }
}

fn workspace_binding_matches(
    resolved: &ResolvedUdfWorkspace,
    binding: &WorkspaceBindingRequest,
) -> bool {
    path_option_matches(resolved.default_project.as_ref(), &binding.project)
        && path_option_matches(resolved.hosts_root.as_ref(), &binding.hosts_root)
        && path_option_matches(resolved.plugins_root.as_ref(), &binding.plugins_root)
        && path_option_matches(resolved.plugin_path.as_ref(), &binding.plugin_path)
        && match (binding.engine_path.as_ref(), resolved.engine_path.as_ref()) {
            (Some(expected), Some(actual)) => paths_equal(expected, actual),
            (Some(_), None) => false,
            (None, _) => true,
        }
}

fn path_option_matches(actual: Option<&PathBuf>, expected: &Path) -> bool {
    actual.is_some_and(|actual| paths_equal(actual, expected))
}

fn read_udf_config() -> Option<UdfConfig> {
    read_udf_config_from_candidates(&udf_config_path_candidates())
}

fn read_udf_config_from_candidates(candidates: &[PathBuf]) -> Option<UdfConfig> {
    for path in candidates {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        if let Ok(config) = toml::from_str(&content) {
            return Some(config);
        }
    }
    None
}

fn effective_udf_config_dir() -> Option<PathBuf> {
    effective_udf_config_dir_from_candidates(&udf_config_path_candidates())
}

fn effective_udf_config_dir_from_candidates(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| path.is_file())
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

fn udf_config_path_candidates() -> Vec<PathBuf> {
    udf_config_path_candidates_from(
        env::var(devflow_config_env_name()).ok().map(PathBuf::from),
        env::var("USERPROFILE").ok().map(PathBuf::from),
        env::var("HOME").ok().map(PathBuf::from),
    )
}

fn udf_config_path_candidates_from(
    configured_dir: Option<PathBuf>,
    user_profile: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(dir) = configured_dir {
        push_unique_path(&mut candidates, dir.join("config.toml"));
    }
    if let Some(profile) = user_profile {
        push_unique_path(
            &mut candidates,
            profile.join(devflow_home_dir_name()).join("config.toml"),
        );
    }
    if let Some(home) = home {
        push_unique_path(
            &mut candidates,
            home.join(devflow_home_dir_name()).join("config.toml"),
        );
    }
    push_unique_path(
        &mut candidates,
        PathBuf::from(devflow_home_dir_name()).join("config.toml"),
    );
    candidates
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|candidate| candidate == &path) {
        paths.push(path);
    }
}

fn devflow_config_env_name() -> String {
    ["UNREAL", "DEVFLOW", "_CONFIG_DIR"].concat()
}

fn devflow_home_dir_name() -> String {
    [".unreal", "devflow"].concat()
}

fn workspace_matches_path(
    workspace: &UdfWorkspaceConfig,
    root: &UdfConfig,
    input_path: &Path,
) -> bool {
    [
        workspace.plugin_path.as_ref(),
        workspace
            .plugins_root
            .as_ref()
            .or(root.plugins_root.as_ref()),
        workspace
            .default_project
            .as_ref()
            .or(root.default_project.as_ref()),
        workspace.hosts_root.as_ref().or(root.hosts_root.as_ref()),
        workspace.engine_path.as_ref().or(root.engine_path.as_ref()),
    ]
    .into_iter()
    .flatten()
    .any(|candidate| path_is_same_or_child(input_path, candidate))
}

fn path_is_same_or_child(path: &Path, candidate: &Path) -> bool {
    let path = normalized_path_key(path);
    let candidate = normalized_path_key(candidate);
    path == candidate || path.starts_with(&format!("{candidate}\\"))
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    normalized_path_key(left) == normalized_path_key(right)
}

fn normalized_path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn is_path_like(value: &str) -> bool {
    value.contains(':') || value.contains('\\') || value.contains('/')
}

fn created_task_from_list_json(
    task_id: &str,
    primary: &str,
    resolved: &ResolvedUdfWorkspace,
    stdout: &str,
    workspace_binding_updated: bool,
    reused_existing: bool,
) -> Result<CreatedTaskContext, CreateTaskFailure> {
    let requested_primary_plugins = primary_plugin_items(primary);
    let primary_display = requested_primary_plugins.join(",");
    let value: Value = serde_json::from_str(stdout).map_err(|error| {
        CreateTaskFailure::failed(
            format!("DevFlow list 输出不是稳定 JSON，无法确认任务上下文：{error}"),
            None,
        )
    })?;
    let tasks = value
        .get("tasks")
        .and_then(Value::as_array)
        .ok_or_else(|| CreateTaskFailure::failed("DevFlow list JSON 缺少 tasks 数组。", None))?;
    let task = tasks
        .iter()
        .find(|task| task.get("id").and_then(Value::as_str) == Some(task_id))
        .ok_or_else(|| {
            CreateTaskFailure::failed(
                format!("DevFlow 创建后没有在任务清单中找到 taskId={task_id}；为避免串项目，Provider 不会启动。"),
                None,
            )
        })?;

    let workspace = task
        .get("workspace")
        .and_then(Value::as_str)
        .unwrap_or(&resolved.name)
        .to_string();
    let hosts_root = task
        .pointer("/context/hosts_root")
        .and_then(Value::as_str)
        .map(PathBuf::from)
        .or_else(|| resolved.hosts_root.clone())
        .ok_or_else(|| {
            CreateTaskFailure::failed(
                "DevFlow 任务元数据缺少 hosts_root，无法确定隔离 Host 路径。",
                None,
            )
        })?;
    let host_path = hosts_root
        .join(format!("W-{workspace}"))
        .join(format!("T-{task_id}_Host"));
    let host_project_path = host_path.join(format!("T-{task_id}_Host.uproject"));

    let primary_entry = task
        .get("primary_plugins")
        .and_then(Value::as_array)
        .and_then(|plugins| {
            requested_primary_plugins.iter().find_map(|requested| {
                plugins.iter().find(|plugin| {
                    plugin.get("name").and_then(Value::as_str) == Some(requested.as_str())
                })
            })
        })
        .ok_or_else(|| {
            CreateTaskFailure::failed(
                format!("DevFlow 任务元数据缺少主插件 {primary_display}；Provider 不会启动。"),
                None,
            )
        })?;
    let worktree_rel = primary_entry
        .get("worktree")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CreateTaskFailure::failed(
                format!("DevFlow 主插件 {primary_display} 缺少 worktree 字段。"),
                None,
            )
        })?;
    let dependency_plugins = task
        .get("dependency_plugins")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(DependencyPluginSummary {
                        name: item.get("name")?.as_str()?.to_string(),
                        source: item
                            .get("source")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                            .to_string(),
                        source_path: item
                            .get("source_path")
                            .and_then(Value::as_str)
                            .map(ToString::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(CreatedTaskContext {
        workspace,
        task_id: task_id.to_string(),
        task_uid: task
            .get("task_uid")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        branch: task
            .get("branch")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        based_on: task
            .get("based_on")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        host_path: path_to_json_string(&host_path),
        host_project_path: path_to_json_string(&host_project_path),
        primary_plugin: primary_display,
        primary_plugins: requested_primary_plugins,
        primary_worktree_path: path_to_json_string(&host_path.join(worktree_rel)),
        source_repo: primary_entry
            .get("source_repo")
            .and_then(Value::as_str)
            .map(ToString::to_string),
        dependency_plugins,
        workspace_binding_updated,
        reused_existing,
    })
}

fn path_to_json_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn joined_output(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.trim().is_empty() {
        stdout.trim().to_string()
    } else if stdout.trim().is_empty() {
        stderr.trim().to_string()
    } else {
        format!("{}\n{}", stdout.trim(), stderr.trim())
    }
}

fn short_evidence(value: &str) -> String {
    let redacted = redact_legacy_text(value.trim());
    redacted
        .chars()
        .take(1400)
        .collect::<String>()
        .trim()
        .to_string()
}

#[derive(Clone, Debug)]
struct AdapterSnapshot {
    version: SafeProbe,
    status: SafeProbe,
    tasks: SafeProbe,
    workspaces: SafeProbe,
}

impl AdapterSnapshot {
    fn collect() -> Self {
        Self {
            version: run_fixed_probe(&["--version"], "version"),
            status: run_fixed_probe(&["status", "--format", "json"], "junction status"),
            tasks: run_fixed_probe(&["list", "--format", "json"], "task list"),
            workspaces: run_fixed_probe(
                &["workspace", "list", "--format", "json"],
                "workspace list",
            ),
        }
    }

    fn adapter_json(&self) -> String {
        format!(
            "{{\"available\":{},\"version\":{},\"status\":\"integrated\",\"oldNamesHidden\":true}}",
            self.version.ok,
            json_option(self.version.summary.as_deref())
        )
    }
}

#[derive(Clone, Debug)]
struct SafeProbe {
    ok: bool,
    summary: Option<String>,
    raw_format: &'static str,
    item_count_hint: usize,
    error: Option<String>,
}

impl SafeProbe {
    fn to_json(&self, label: &str) -> String {
        format!(
            "{{\"label\":{},\"available\":{},\"rawFormat\":{},\"itemCountHint\":{},\"summary\":{},\"error\":{}}}",
            json_string(label),
            self.ok,
            json_string(self.raw_format),
            self.item_count_hint,
            json_option(self.summary.as_deref()),
            json_option(self.error.as_deref())
        )
    }
}

fn run_fixed_probe(args: &[&str], label: &'static str) -> SafeProbe {
    let output = run_fixed_adapter_command(args);
    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let body = if stdout.trim().is_empty() {
                stderr.trim()
            } else {
                stdout.trim()
            };
            let raw_format = if body.starts_with('{') || body.starts_with('[') {
                "json"
            } else {
                "legacyText"
            };
            SafeProbe {
                ok: output.status.success(),
                summary: summarize_probe(label, body),
                raw_format,
                item_count_hint: count_probe_items(label, body),
                error: (!output.status.success()).then(|| redact_legacy_text(body)),
            }
        }
        Err(error) => SafeProbe {
            ok: false,
            summary: None,
            raw_format: "unavailable",
            item_count_hint: 0,
            error: Some(format!("adapter unavailable: {}", error.kind())),
        },
    }
}

fn run_fixed_adapter_command<S>(args: &[S]) -> Result<std::process::Output, std::io::Error>
where
    S: AsRef<OsStr>,
{
    let mut last_error = None;
    let config_dir = effective_udf_config_dir();
    for candidate in adapter_executable_candidates() {
        let mut command = Command::new(&candidate);
        if let Some(config_dir) = config_dir.as_ref() {
            command.env(devflow_config_env_name(), config_dir);
        }
        for arg in args {
            command.arg(arg.as_ref());
        }
        match command.output() {
            Ok(output) => return Ok(output),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::NotFound, "devflow adapter not found")
    }))
}

fn adapter_executable_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(path) = env::var("UWF_DEVFLOW_ADAPTER_EXE") {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(path) = env::var("PATH") {
        for dir in env::split_paths(&path) {
            push_unique_path(&mut candidates, dir.join(adapter_executable_name()));
        }
    }
    if let Ok(profile) = env::var("USERPROFILE") {
        push_unique_path(
            &mut candidates,
            PathBuf::from(profile)
                .join(devflow_home_dir_name())
                .join("bin")
                .join(adapter_executable_name()),
        );
    }
    if let Ok(home) = env::var("HOME") {
        push_unique_path(
            &mut candidates,
            PathBuf::from(home)
                .join(devflow_home_dir_name())
                .join("bin")
                .join(adapter_executable_name()),
        );
    }
    if let Ok(current_dir) = env::current_dir() {
        push_unique_path(
            &mut candidates,
            current_dir
                .join(".plugin-worktrees")
                .join("UEWorkflow")
                .join("DevFlow")
                .join("target")
                .join("release")
                .join(adapter_executable_name()),
        );
        if let Some(parent) = current_dir.parent() {
            push_unique_path(
                &mut candidates,
                parent
                    .join(source_project_name())
                    .join("target")
                    .join("release")
                    .join(adapter_executable_name()),
            );
        }
    }
    candidates
        .into_iter()
        .filter(|candidate| candidate.is_file())
        .collect()
}

fn summarize_probe(label: &str, body: &str) -> Option<String> {
    if body.trim().is_empty() {
        return None;
    }
    Some(match label {
        "version" => redact_legacy_text(body.lines().next().unwrap_or("available")),
        "junction status" => format!(
            "activeTaskCount={}, validJunctionCount={}",
            body.matches("\"active_task\"").count(),
            body.matches("\"junction_valid\": true").count()
        ),
        "task list" => format!("taskCount={}", body.matches("\"schema_version\"").count()),
        "workspace list" => format!("workspaceCount={}", count_workspace_rows(body)),
        _ => "available".to_string(),
    })
}

fn count_probe_items(label: &str, body: &str) -> usize {
    match label {
        "junction status" => body.matches("\"active_task\"").count(),
        "task list" => body.matches("\"schema_version\"").count(),
        "workspace list" => count_workspace_rows(body),
        "version" if !body.trim().is_empty() => 1,
        _ => 0,
    }
}

fn count_workspace_rows(body: &str) -> usize {
    body.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty()
                && !trimmed.starts_with("Workspaces")
                && !trimmed.starts_with("Name")
                && !trimmed.starts_with("---")
        })
        .count()
}

fn redact_legacy_text(value: &str) -> String {
    value
        .replace(&source_project_name(), "DevFlowAdapter")
        .replace(&adapter_executable_stem(), "devflow-adapter")
}

fn source_project_name() -> String {
    ["Unreal", "DevFlow"].concat()
}

fn adapter_executable_stem() -> String {
    ["unreal", "devflow"].concat()
}

fn adapter_executable_name() -> String {
    format!("{}.exe", adapter_executable_stem())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_dev_command_group() {
        let contract = contract();
        assert_eq!(contract.id.command_group(), "uwf dev");
        assert!(contract
            .commands
            .iter()
            .any(|command| command.name == "dry-run"));
    }

    #[test]
    fn dry_run_freezes_context_and_blocks_raw_build() {
        let request = CommandRequest {
            goal: Some("修复地形锚点".to_string()),
            workspace: Some("neon-dev1".to_string()),
            task_id: Some("anchor-fix".to_string()),
            action: Some("build-task".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("dry-run", &request);
        assert!(json.contains("\"status\":\"planned\""));
        assert!(json.contains("\"confirmationBoundary\":\"UEWorkflow.DevFlow.build-task.v1\""));
        assert!(json.contains("task worktree / Host isolated environment"));
        assert!(json.contains("raw UE build"));
        assert!(json.contains("\"willExecute\":false"));
    }

    #[test]
    fn execute_requires_confirmation_boundary() {
        let request = CommandRequest {
            action: Some("build-check".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("execute", &request);
        assert!(json.contains("\"status\":\"blocked\""));
        assert!(json.contains("\"requiredConfirmation\":\"UEWorkflow.DevFlow.build-check.v1\""));
    }

    #[test]
    fn execute_blocks_task_creation_without_required_context() {
        let request = CommandRequest {
            action: Some("create-task".to_string()),
            confirmation: Some("UEWorkflow.DevFlow.create-task.v1".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("execute", &request);
        assert!(json.contains("\"status\":\"blocked\""));
        assert!(json.contains("worktree"));
    }

    #[test]
    fn execute_allows_build_check_without_compiling() {
        let request = CommandRequest {
            action: Some("build-check".to_string()),
            confirmation: Some("UEWorkflow.DevFlow.build-check.v1".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("execute", &request);
        assert!(json.contains("\"status\":\"complete\""));
        assert!(json.contains("\"rawUnrealBuild\":\"disabled\""));
        assert!(json.contains("\"sideEffects\":[]"));
    }

    #[test]
    fn switch_dry_run_declares_confirmation_boundary() {
        let dry_run_request = CommandRequest {
            action: Some("switch".to_string()),
            workspace: Some("neon-dev1".to_string()),
            task_id: Some("task-001".to_string()),
            project: Some("Neon".to_string()),
            primary: Some("AesWorld".to_string()),
            ..CommandRequest::default()
        };
        let dry_run = command_json("dry-run", &dry_run_request);
        assert!(dry_run.contains("\"confirmationBoundary\":\"UEWorkflow.DevFlow.switch.v1\""));
        assert!(dry_run.contains("预览目标上下文"));

        let execute_request = CommandRequest { ..dry_run_request };
        let execute = command_json("execute", &execute_request);
        assert!(execute.contains("\"status\":\"blocked\""));
        assert!(execute.contains("\"requiredConfirmation\":\"UEWorkflow.DevFlow.switch.v1\""));
    }

    /// 回归：用户填主项目 DEV，但匹配到的 DevFlow workspace 绑定 DEV_1 时，
    /// 绝不能沿用 DEV_1 绑定（历史 bug：Host 项目/主项目被串到 DEV_1）。
    /// 有主插件路径时必须改为为 DEV 登记独立 workspace；无主插件路径时必须阻断。
    #[test]
    fn config_discovery_falls_back_when_env_points_at_unrealworkflow() {
        let temp_root = std::env::temp_dir().join(format!(
            "uwf-devflow-config-fallback-{}",
            std::process::id()
        ));
        let bad_unrealworkflow = temp_root.join(".unrealworkflow");
        let good_unrealdevflow = temp_root.join(".unrealdevflow");
        fs::create_dir_all(&bad_unrealworkflow).expect("create bad config dir");
        fs::create_dir_all(&good_unrealdevflow).expect("create good config dir");
        fs::write(
            good_unrealdevflow.join("config.toml"),
            "hosts_root = 'F:\\ShanghaiP4\\neon\\Hosts'\n\
             default_project = 'F:\\ShanghaiP4\\neon\\UGA\\DEV_1'\n\
             [workspaces.neon-dev1]\n\
             plugin_path = 'F:\\ShanghaiP4\\neon\\Plugins\\AesWorld'\n",
        )
        .expect("write good config");

        let candidates = udf_config_path_candidates_from(
            Some(bad_unrealworkflow),
            Some(temp_root.clone()),
            None,
        );
        let config = read_udf_config_from_candidates(&candidates)
            .expect("good .unrealdevflow config must win after bad env path is skipped");
        assert!(config.workspaces.contains_key("neon-dev1"));
        assert_eq!(
            effective_udf_config_dir_from_candidates(&candidates),
            Some(good_unrealdevflow)
        );

        let _ = fs::remove_dir_all(&temp_root);
    }

    #[test]
    fn explicit_main_project_never_falls_back_to_workspace_default_project() {
        let temp_root = std::env::temp_dir().join(format!(
            "uwf-devflow-retarget-regression-{}",
            std::process::id()
        ));
        let dev = temp_root.join("UGA").join("DEV");
        let dev_1 = temp_root.join("UGA").join("DEV_1");
        let plugin_path = temp_root.join("Plugins").join("AesWorld");
        for dir in [&dev, &dev_1, &plugin_path] {
            fs::create_dir_all(dir).expect("create temp dirs");
        }
        fs::write(dev.join("DEV.uproject"), "{}").expect("write DEV uproject");
        fs::write(dev_1.join("DEV_1.uproject"), "{}").expect("write DEV_1 uproject");
        let config_dir = temp_root.join("config");
        fs::create_dir_all(&config_dir).expect("create config dir");
        fs::write(
            config_dir.join("config.toml"),
            format!(
                "hosts_root = '{hosts}'\ndefault_project = '{dev1}'\n\n[workspaces.neon-dev1]\nhosts_root = '{hosts}'\nplugin_path = '{plugin}'\ndefault_project = '{dev1}'\n",
                hosts = temp_root.join("Hosts").display(),
                dev1 = dev_1.display(),
                plugin = plugin_path.display(),
            ),
        )
        .expect("write config");
        env::set_var(devflow_config_env_name(), &config_dir);

        let request = CommandRequest {
            workspace: Some(plugin_path.to_string_lossy().to_string()),
            main_project: Some(dev.to_string_lossy().to_string()),
            primary_path: Some(plugin_path.to_string_lossy().to_string()),
            ..CommandRequest::default()
        };

        // 有主插件路径：必须重定向到新 workspace，主项目必须是 DEV，不能是 DEV_1。
        let resolved = resolve_udf_workspace_for_request(
            &request,
            request.workspace.as_deref().unwrap(),
            true,
        )
        .expect("retarget must succeed when binding can be registered");
        assert_ne!(resolved.name, "neon-dev1", "不能沿用 DEV_1 的 workspace");
        let resolved_project = resolved
            .default_project
            .as_ref()
            .expect("retargeted workspace must carry the requested project");
        assert!(
            paths_equal(resolved_project, &dev),
            "主项目必须是用户填写的 DEV，而不是 workspace 默认的 DEV_1：{}",
            resolved_project.display()
        );

        // 无主插件路径（不可登记绑定）：必须阻断，不能静默串到 DEV_1。
        let blocked = resolve_udf_workspace_for_request(
            &request,
            request.workspace.as_deref().unwrap(),
            false,
        );
        let error = blocked.expect_err("不可登记绑定时必须阻断而不是串项目");
        assert_eq!(error.status, "blocked");
        assert!(error.message.contains("避免串项目"));

        env::remove_var(devflow_config_env_name());
        let _ = fs::remove_dir_all(&temp_root);
    }
}
