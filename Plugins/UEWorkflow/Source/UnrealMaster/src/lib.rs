use serde_json::Value;
use uwf_core::{
    json_array, json_option, json_string, CommandRequest, ModuleContract, ModuleId,
    CONTRACT_VERSION,
};

pub fn modules() -> Vec<ModuleContract> {
    vec![
        uwf_devflow::contract(),
        uwf_agenthub::contract(),
        uwf_knowledgebase::contract(),
    ]
}

pub fn doctor_json() -> String {
    let modules = modules();
    format!(
        "{{\"kind\":\"UnrealWorkflowDoctor\",\"contractVersion\":{},\"status\":\"ok\",\"plugin\":{},\"command\":{},\"modules\":{},\"safety\":{{\"allowArbitraryShell\":false,\"allowRawUnrealBuild\":false,\"allowDestructiveDelete\":false}},\"message\":{}}}",
        CONTRACT_VERSION,
        json_string("UnrealWorkflow"),
        json_string("uwf"),
        module_list_json(&modules),
        json_string("Unreal Workflow contract skeleton is ready.")
    )
}

pub fn modules_json() -> String {
    let modules = modules();
    format!(
        "{{\"kind\":\"UnrealWorkflowModules\",\"contractVersion\":{},\"status\":\"ok\",\"plugin\":{},\"modules\":{}}}",
        CONTRACT_VERSION,
        json_string("UnrealWorkflow"),
        module_list_json(&modules)
    )
}

pub fn master_command_json(command: &str, request: &CommandRequest) -> Result<String, String> {
    match command {
        "plan" => Ok(master_plan_json(request)),
        "dry-run" => Ok(master_dry_run_json(request)),
        "execute" => Ok(master_execute_json(request)),
        "history" => Ok(master_history_json(request)),
        "artifacts" => Ok(master_artifacts_json(request)),
        _ => Err(format!(
            "unknown master command '{command}', expected plan, dry-run, execute, history, artifacts"
        )),
    }
}

fn master_plan_json(request: &CommandRequest) -> String {
    format!(
        "{{\"kind\":\"UnrealWorkflowMasterResult\",\"contractVersion\":{},\"status\":\"planned\",\"command\":\"master plan\",\"request\":{},\"goal\":{},\"stages\":{},\"moduleCalls\":{},\"review\":{},\"diagnosis\":{},\"knowledgeClosure\":{},\"sideEffects\":[]}}",
        CONTRACT_VERSION,
        request.to_json(),
        json_string(request.goal.as_deref().unwrap_or("未提供目标")),
        master_stages_json(),
        master_module_calls_json(),
        json_string("任务结束前必须触发评审 Agent，结果进入执行记录。"),
        json_string("失败时触发诊断 Agent，保留错误证据。"),
        json_string("任务结束后触发 KnowledgeBase record-task 写入正确 scope。")
    )
}

fn master_dry_run_json(request: &CommandRequest) -> String {
    let dev_request = request_for_action(request, "create-task", None);
    let agent_request = request_for_action(
        request,
        &format!(
            "package-{}",
            provider_for_request(request).replace("claude-code", "claude")
        ),
        Some(provider_for_request(request)),
    );
    let kb_request = request_for_action(request, "record-task", None);
    let module_results = vec![
        uwf_devflow::command_json("dry-run", &dev_request),
        uwf_agenthub::command_json("dry-run", &agent_request),
        uwf_knowledgebase::command_json("dry-run", &kb_request),
    ];
    let context_confirmation = context_confirmation_from_module_result(&module_results[0], request);
    format!(
        "{{\"kind\":\"UnrealWorkflowMasterResult\",\"contractVersion\":{},\"status\":\"planned\",\"command\":\"master dry-run\",\"request\":{},\"confirmationBoundary\":{},\"willExecute\":false,\"contextConfirmation\":{},\"stages\":{},\"moduleResults\":{},\"providerPackage\":{},\"blockedActions\":{},\"sideEffects\":[]}}",
        CONTRACT_VERSION,
        request.to_json(),
        json_string("UEWorkflow.UnrealMaster.execute.v1"),
        context_confirmation,
        master_stages_json(),
        json_array(module_results),
        provider_package_json(request, &context_confirmation),
        json_array([
            "任意 shell",
            "raw UE build",
            "破坏性删除",
            "绕过模块直接操作原工程"
        ]
        .into_iter()
        .map(json_string))
    )
}

fn master_execute_json(request: &CommandRequest) -> String {
    if request.confirmation.as_deref() != Some("UEWorkflow.UnrealMaster.execute.v1") {
        return format!(
            "{{\"kind\":\"UnrealWorkflowMasterResult\",\"contractVersion\":{},\"status\":\"blocked\",\"command\":\"master execute\",\"request\":{},\"requiredConfirmation\":{},\"sideEffects\":[],\"message\":{}}}",
            CONTRACT_VERSION,
            request.to_json(),
            json_string("UEWorkflow.UnrealMaster.execute.v1"),
            json_string("master execute 缺少匹配的 dry-run 确认边界；未派发任何模块命令。")
        );
    }

    let dev_request = request_for_action_with_confirmation(
        request,
        "create-task",
        "UEWorkflow.DevFlow.create-task.v1",
        None,
    );
    let dev_result = uwf_devflow::command_json("execute", &dev_request);
    let context_confirmation = context_confirmation_from_module_result(&dev_result, request);
    let dev_status = result_status(&dev_result).unwrap_or_else(|| "failed".to_string());
    if dev_status != "complete" {
        return format!(
            "{{\"kind\":\"UnrealWorkflowMasterResult\",\"contractVersion\":{},\"status\":{},\"command\":\"master execute\",\"request\":{},\"message\":{},\"contextConfirmation\":{},\"stages\":{},\"moduleResults\":{},\"providerPackage\":null,\"diagnosis\":{},\"review\":{},\"knowledgeClosure\":{},\"sideEffects\":[]}}",
            CONTRACT_VERSION,
            json_string(if dev_status == "blocked" { "blocked" } else { "failed" }),
            request.to_json(),
            json_string("DevFlow 未能创建隔离任务 Host/worktree；Provider 不会被派发，禁止退化为手工修改主线。"),
            context_confirmation,
            master_stages_json(),
            json_array([dev_result]),
            json_string("先修复 DevFlow 返回的环境或输入问题，再重新执行。"),
            json_string("未进入 Provider 修改阶段，无评审结果。"),
            json_string("未进入任务结束阶段，知识沉淀不写入。")
        );
    }

    let agent_action = format!(
        "package-{}",
        provider_for_request(request).replace("claude-code", "claude")
    );
    let agent_boundary = format!("UEWorkflow.AgentHub.{agent_action}.v1");
    let agent_request = request_for_action_with_confirmation(
        request,
        &agent_action,
        &agent_boundary,
        Some(provider_for_request(request)),
    );
    let kb_request = request_for_action_with_confirmation(
        request,
        "record-task",
        "UEWorkflow.KnowledgeBase.record-task.v1",
        None,
    );
    let module_results = vec![
        dev_result,
        uwf_agenthub::command_json("execute", &agent_request),
        uwf_knowledgebase::command_json("execute", &kb_request),
    ];
    let status = if module_results.iter().any(|result| {
        matches!(
            result_status(result).as_deref(),
            Some("failed") | Some("error")
        )
    }) {
        "failed"
    } else if module_results
        .iter()
        .any(|result| result_status(result).as_deref() == Some("blocked"))
    {
        "blocked"
    } else {
        "complete"
    };
    format!(
        "{{\"kind\":\"UnrealWorkflowMasterResult\",\"contractVersion\":{},\"status\":{},\"command\":\"master execute\",\"request\":{},\"contextConfirmation\":{},\"stages\":{},\"moduleResults\":{},\"providerPackage\":{},\"diagnosis\":{},\"review\":{},\"knowledgeClosure\":{},\"sideEffects\":[{}]}}",
        CONTRACT_VERSION,
        json_string(status),
        request.to_json(),
        context_confirmation,
        master_stages_json(),
        json_array(module_results),
        provider_package_json(request, &context_confirmation),
        json_string("失败时由诊断 Agent 使用模块结果和执行记录定位原因。"),
        json_string("评审 Agent 必须在真实修改后只读审查；本次安全执行只生成包和轻量知识。"),
        json_string("KnowledgeBase record-task 已按 scope 写入轻量产物，或在失败时返回错误证据。"),
        json_string("knowledge-light-artifact")
    )
}

fn master_history_json(request: &CommandRequest) -> String {
    format!(
        "{{\"kind\":\"UnrealWorkflowMasterResult\",\"contractVersion\":{},\"status\":\"ready\",\"command\":\"master history\",\"request\":{},\"history\":[],\"sideEffects\":[]}}",
        CONTRACT_VERSION,
        request.to_json()
    )
}

fn master_artifacts_json(request: &CommandRequest) -> String {
    format!(
        "{{\"kind\":\"UnrealWorkflowMasterResult\",\"contractVersion\":{},\"status\":\"ready\",\"command\":\"master artifacts\",\"request\":{},\"artifacts\":[],\"sideEffects\":[]}}",
        CONTRACT_VERSION,
        request.to_json()
    )
}

fn master_stages_json() -> String {
    let stages = [
        (
            "understand-goal",
            "确认任务和 UE 上下文",
            "UnrealMaster",
            "在派发前确认任务说明、workspace、主项目、主插件、Host 项目和依赖状态。",
        ),
        (
            "devflow-plan",
            "生成开发流预演",
            "DevFlow",
            "规划任务上下文、worktree/Host、插件依赖和构建路由；未知项保持待确认。",
        ),
        (
            "agenthub-package",
            "生成智能体任务包",
            "AgentHub",
            "选择 provider 命令包和角色分工。",
        ),
        (
            "review-diagnose",
            "评审和失败诊断",
            "AgentHub",
            "真实修改后触发评审，失败时触发诊断。",
        ),
        (
            "knowledge-closure",
            "知识沉淀",
            "KnowledgeBase",
            "复盘、方案、规则候选写入正确 scope。",
        ),
    ];
    json_array(stages.into_iter().map(|(id, title, module, summary)| {
        format!(
            "{{\"id\":{},\"title\":{},\"module\":{},\"summary\":{},\"willExecute\":false}}",
            json_string(id),
            json_string(title),
            json_string(module),
            json_string(summary)
        )
    }))
}

fn master_module_calls_json() -> String {
    let calls = [
        (
            "DevFlow",
            "uwf dev dry-run",
            "任务上下文、worktree/Host、构建路由",
        ),
        (
            "AgentHub",
            "uwf agents dry-run",
            "provider 命令包、角色分工",
        ),
        (
            "KnowledgeBase",
            "uwf kb dry-run",
            "scope、沉淀路径、轻量索引",
        ),
    ];
    json_array(calls.into_iter().map(|(module, command, purpose)| {
        format!(
            "{{\"module\":{},\"command\":{},\"purpose\":{}}}",
            json_string(module),
            json_string(command),
            json_string(purpose)
        )
    }))
}

fn provider_package_json(request: &CommandRequest, context_confirmation: &str) -> String {
    format!(
        "{{\"provider\":{},\"entryRole\":{},\"masterCommand\":{},\"goal\":{},\"contextConfirmation\":{},\"moduleCommands\":{}}}",
        json_string(provider_for_request(request)),
        json_string("虚幻大师"),
        json_string("uwf master plan -> uwf master dry-run -> uwf master execute"),
        json_string(request.goal.as_deref().unwrap_or("未提供目标")),
        context_confirmation,
        master_module_calls_json()
    )
}

fn context_confirmation_json(request: &CommandRequest) -> String {
    let host_ready = request
        .host_root
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        && request
            .main_project
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty());
    let primary_ready = request
        .primary
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    let deps_ready = request
        .plugin_dependencies
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty());
    format!(
        "{{\"confirmationStage\":{},\"workspacePath\":{},\"project\":{},\"primaryPlugin\":{},\"primaryPluginPath\":{},\"hostRoot\":{},\"mainProject\":{},\"hostProject\":{{\"status\":{},\"path\":{},\"hostRoot\":{},\"message\":{}}},\"pluginDependencies\":{{\"status\":{},\"message\":{}}},\"mustConfirmBeforeProviderExecution\":{}}}",
        json_string("任务创建上下文 / master dry-run"),
        json_option(request.workspace.as_deref()),
        json_option(request.project.as_deref()),
        json_option(request.primary.as_deref()),
        json_option(request.primary_path.as_deref()),
        json_option(request.host_root.as_deref()),
        json_option(request.main_project.as_deref()),
        json_string(if host_ready { "ready" } else { "pending" }),
        json_option(request.main_project.as_deref()),
        json_option(request.host_root.as_deref()),
        json_string(if host_ready {
            "已由任务上下文确认主项目和 Host 根。"
        } else {
            "缺少主项目路径或 Host 根，请在任务创建面板补齐。"
        }),
        json_string(if deps_ready { "ready" } else { "auto" }),
        json_string(if deps_ready {
            "已由任务上下文提供依赖覆盖。"
        } else {
            "未填写，DevFlow 将按 .uplugin 与工程/引擎插件扫描自动识别。"
        }),
        !host_ready || !primary_ready
    )
}

fn context_confirmation_from_module_result(result: &str, request: &CommandRequest) -> String {
    let Ok(value) = serde_json::from_str::<Value>(result) else {
        return context_confirmation_json(request);
    };
    value
        .get("targetContext")
        .map(Value::to_string)
        .unwrap_or_else(|| context_confirmation_json(request))
}

fn result_status(result: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(result).ok()?;
    value.get("status")?.as_str().map(ToString::to_string)
}

fn provider_for_request(request: &CommandRequest) -> &str {
    request.provider.as_deref().unwrap_or("copilot")
}

fn request_for_action(
    request: &CommandRequest,
    action: &str,
    provider: Option<&str>,
) -> CommandRequest {
    let mut next = request.clone();
    next.action = Some(action.to_string());
    if let Some(provider) = provider {
        next.provider = Some(provider.to_string());
    }
    next.confirmation = None;
    next
}

fn request_for_action_with_confirmation(
    request: &CommandRequest,
    action: &str,
    confirmation: &str,
    provider: Option<&str>,
) -> CommandRequest {
    let mut next = request_for_action(request, action, provider);
    next.confirmation = Some(confirmation.to_string());
    next
}

pub fn help_json() -> String {
    let modules = modules();
    let commands = std::iter::once(json_string("doctor"))
        .chain(std::iter::once(json_string("modules")))
        .chain(
            ["master plan", "master dry-run", "master execute"]
                .into_iter()
                .map(json_string),
        )
        .chain(modules.iter().flat_map(|module| {
            module.commands.iter().map(move |command| {
                json_string(&format!(
                    "{} {}",
                    module.id.command_group().trim_start_matches("uwf "),
                    command.name
                ))
            })
        }));
    format!(
        "{{\"kind\":\"UnrealWorkflowHelp\",\"contractVersion\":{},\"status\":\"ok\",\"commands\":{}}}",
        CONTRACT_VERSION,
        json_array(commands)
    )
}

pub fn unknown_top_level_error(command: &str) -> String {
    format!(
        "unknown command '{}', expected doctor, modules, master, or one of: {}",
        command,
        command_group_names(&modules())
    )
}

pub fn module_command_json(
    group: &str,
    command: &str,
    request: &CommandRequest,
) -> Result<String, String> {
    let module = find_module(group).ok_or_else(|| {
        format!("unknown module group '{group}', expected one of: dev, agents, kb")
    })?;

    if !module.commands.iter().any(|spec| spec.name == command) {
        return Err(format!(
            "unknown command '{}' for {}, expected one of: {}",
            command,
            module.id.command_group(),
            command_names(&module)
        ));
    }

    if module.id == ModuleId::DevFlow {
        return Ok(uwf_devflow::command_json(command, request));
    }
    if module.id == ModuleId::AgentHub {
        return Ok(uwf_agenthub::command_json(command, request));
    }
    if module.id == ModuleId::KnowledgeBase {
        return Ok(uwf_knowledgebase::command_json(command, request));
    }

    let result = match command {
        "status" => module.result_json(
            command,
            "ready",
            "模块契约已加载；本阶段只提供安全只读骨架。",
            "\"installed\":true,\"sideEffects\":[]",
        ),
        "capabilities" => {
            module.result_json(command, "ready", "能力清单已输出。", "\"sideEffects\":[]")
        }
        "commands" => {
            module.result_json(command, "ready", "命令清单已输出。", "\"sideEffects\":[]")
        }
        "schema" => module.result_json(command, "ready", "schema 已输出。", "\"sideEffects\":[]"),
        "dry-run" => {
            let goal = request.goal.as_deref().unwrap_or("未提供目标");
            let plan = format!(
                "\"dryRun\":{{\"goal\":{},\"willExecute\":false,\"stages\":[{}, {}, {}],\"blockedActions\":[{}, {}, {}]}}",
                json_string(goal),
                json_string("读取模块契约"),
                json_string("生成受控计划"),
                json_string("等待显式确认"),
                json_string("任意 shell"),
                json_string("raw UE build"),
                json_string("破坏性删除")
            );
            module.result_json(
                command,
                "planned",
                "dry-run 已生成；未执行任何外部副作用。",
                &plan,
            )
        }
        "execute" => module.result_json(
            command,
            "blocked",
            "execute 在当前阶段禁用；请先使用 dry-run 并补齐安全命令清单。",
            "\"sideEffects\":[],\"blocked\":true",
        ),
        "history" => module.result_json(command, "ready", "本阶段暂无历史记录。", "\"history\":[]"),
        "artifacts" => module.result_json(command, "ready", "本阶段暂无产物。", "\"artifacts\":[]"),
        _ => unreachable!("command existence checked above"),
    };

    Ok(result)
}

pub fn command_surface_snapshot() -> String {
    modules()
        .iter()
        .map(|module| {
            format!(
                "{}|{}|{}",
                module.id.code_name(),
                module.id.command_group(),
                command_names(module)
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn error_json(message: &str) -> String {
    format!(
        "{{\"kind\":\"UnrealWorkflowError\",\"contractVersion\":{},\"status\":\"error\",\"message\":{}}}",
        CONTRACT_VERSION,
        json_string(message)
    )
}

fn find_module(group: &str) -> Option<ModuleContract> {
    modules().into_iter().find(|module| {
        matches!(
            (group, module.id.short_name()),
            ("dev", "devflow")
                | ("devflow", "devflow")
                | ("agents", "agents")
                | ("agenthub", "agents")
                | ("kb", "kb")
                | ("knowledgebase", "kb")
        )
    })
}

fn module_list_json(modules: &[ModuleContract]) -> String {
    json_array(modules.iter().map(ModuleContract::to_json))
}

fn command_group_names(modules: &[ModuleContract]) -> String {
    modules
        .iter()
        .map(|module| module.id.command_group().trim_start_matches("uwf "))
        .collect::<Vec<_>>()
        .join(", ")
}

fn command_names(module: &ModuleContract) -> String {
    module
        .commands
        .iter()
        .map(|command| command.name)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doctor_lists_three_business_modules() {
        let json = doctor_json();
        assert!(json.contains("\"kind\":\"UnrealWorkflowDoctor\""));
        assert!(json.contains("\"codeName\":\"DevFlow\""));
        assert!(json.contains("\"codeName\":\"AgentHub\""));
        assert!(json.contains("\"codeName\":\"KnowledgeBase\""));
    }

    #[test]
    fn module_status_contract_is_stable() {
        let json = module_command_json("dev", "status", &CommandRequest::default()).unwrap();
        assert!(json.contains("\"kind\":\"UnrealWorkflowModuleResult\""));
        assert!(json.contains("\"command\":\"status\""));
        assert!(json.contains("\"schema\":\"uwf.module.devflow.v1\""));
    }

    #[test]
    fn execute_is_blocked_in_skeleton_phase() {
        let json = module_command_json("agents", "execute", &CommandRequest::default()).unwrap();
        assert!(json.contains("\"status\":\"blocked\""));
        assert!(json.contains("\"blocked\":true"));
    }

    #[test]
    fn master_plan_lists_three_module_calls() {
        let request = CommandRequest {
            goal: Some("修复插件任务".to_string()),
            ..CommandRequest::default()
        };
        let json = master_command_json("plan", &request).unwrap();
        assert!(json.contains("\"kind\":\"UnrealWorkflowMasterResult\""));
        assert!(json.contains("\"module\":\"DevFlow\""));
        assert!(json.contains("\"module\":\"AgentHub\""));
        assert!(json.contains("\"module\":\"KnowledgeBase\""));
    }

    #[test]
    fn master_dry_run_combines_module_results() {
        let request = CommandRequest {
            goal: Some("修复插件任务".to_string()),
            provider: Some("copilot".to_string()),
            ..CommandRequest::default()
        };
        let json = master_command_json("dry-run", &request).unwrap();
        assert!(json.contains("\"status\":\"planned\""));
        assert!(json.contains("\"confirmationBoundary\":\"UEWorkflow.UnrealMaster.execute.v1\""));
        assert!(json.contains("\"codeName\":\"DevFlow\""));
        assert!(json.contains("\"codeName\":\"AgentHub\""));
        assert!(json.contains("\"codeName\":\"KnowledgeBase\""));
    }

    #[test]
    fn master_execute_requires_confirmation() {
        let request = CommandRequest::default();
        let json = master_command_json("execute", &request).unwrap();
        assert!(json.contains("\"status\":\"blocked\""));
        assert!(json.contains("\"requiredConfirmation\":\"UEWorkflow.UnrealMaster.execute.v1\""));
    }

    #[test]
    fn command_surface_matches_golden_contract() {
        let expected = include_str!("../../../Tests/contracts/uwf-command-surface.golden.txt");
        assert_eq!(command_surface_snapshot(), expected.trim_end());
    }
}
