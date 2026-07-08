use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use uwf_core::{
    json_array, json_option, json_string, ArtifactRef, Capability, CommandOutcome, CommandRequest,
    CommandSpec, CommandStatus, DryRunPlan, DryRunStage, ModuleContract, ModuleId, ProviderRole,
    SafetyLevel,
};

pub fn contract() -> ModuleContract {
    ModuleContract {
        id: ModuleId::AgentHub,
        capabilities: vec![
            Capability {
                id: "provider.discovery",
                title: "Provider 发现",
                description: "发现 Copilot、Codex、Claude Code、OpenCode 等可用入口。",
            },
            Capability {
                id: "role.registry",
                title: "角色注册",
                description: "统一虚幻大师、执行、评审、诊断等角色语义，provider 文件只做适配。",
            },
            Capability {
                id: "command.routing",
                title: "命令路由",
                description: "把用户目标映射到受控 command，而不是一次性污染上下文。",
            },
        ],
        commands: vec![
            CommandSpec {
                name: "status",
                summary: "输出智能体模块状态。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "capabilities",
                summary: "输出智能体能力清单。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "commands",
                summary: "输出智能体命令清单。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "schema",
                summary: "输出智能体结果契约。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "dry-run",
                summary: "预演角色和 provider 路由，不安装、不写 provider 配置。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: true,
                enabled: true,
            },
            CommandSpec {
                name: "execute",
                summary: "执行安全智能体动作；安装 provider 或启动代理默认阻断。",
                safety: SafetyLevel::Dangerous,
                supports_dry_run: true,
                enabled: true,
            },
            CommandSpec {
                name: "history",
                summary: "读取智能体执行历史。本阶段返回空集合。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
            CommandSpec {
                name: "artifacts",
                summary: "读取智能体产物索引。本阶段返回空集合。",
                safety: SafetyLevel::ReadOnly,
                supports_dry_run: false,
                enabled: true,
            },
        ],
        schema: "uwf.module.agenthub.v1",
    }
}

pub fn command_json(command: &str, request: &CommandRequest) -> String {
    match command {
        "status" => status_json(command, request),
        "capabilities" => contract().result_json(
            command,
            "ready",
            "虚幻智能体能力清单已输出。",
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
            "未知智能体命令；请通过 commands 查看清单。",
            &format!("\"request\":{},\"blocked\":true", request.to_json()),
        ),
    }
}

fn status_json(command: &str, request: &CommandRequest) -> String {
    let registry = AgentRegistry::discover();
    let extra = format!(
        "\"request\":{},\"installed\":true,\"sideEffects\":[],\"agentHub\":{{\"masterAgent\":{},\"roles\":{},\"providers\":{},\"commandWrapping\":{},\"buildPolicy\":{}}}",
        request.to_json(),
        master_agent_json(),
        roles_json(),
        registry.providers_json(),
        command_wrapping_json(),
        build_policy_json()
    );
    contract().result_json(
        command,
        "ready",
        "虚幻智能体已加载；provider 状态只读检查完成。",
        &extra,
    )
}

fn commands_json(command: &str, request: &CommandRequest) -> String {
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"declaredActions\":{},\"providerCommands\":{}",
        request.to_json(),
        declared_actions_json(),
        provider_commands_json(request)
    );
    contract().result_json(command, "ready", "智能体命令清单已输出。", &extra)
}

fn schema_json(command: &str, request: &CommandRequest) -> String {
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"schemaDetail\":{{\"command\":\"uwf agents <status|capabilities|commands|schema|dry-run|execute|history|artifacts>\",\"request\":{},\"providerPackage\":{},\"roles\":{}}}",
        request.to_json(),
        json_string("CommandRequest"),
        json_string("AgentHubProviderCommandPackage"),
        roles_json()
    );
    contract().result_json(command, "ready", "智能体 schema 已输出。", &extra)
}

fn dry_run_json(command: &str, request: &CommandRequest) -> String {
    let action = normalize_action(request);
    let plan = dry_run_plan(action, request);
    let outcome = CommandOutcome {
        status: CommandStatus::Planned,
        message: "智能体 dry-run 已生成；不会安装 provider，不会启动外部代理。".to_string(),
        artifacts: vec![ArtifactRef {
            id: "agenthub-provider-package".to_string(),
            kind: "providerCommandPackage".to_string(),
            path: "%USERPROFILE%/.unrealworkflow/tasks/draft/agenthub-provider-package.json"
                .to_string(),
            description: "provider 可执行任务包预览".to_string(),
        }],
        dry_run: Some(plan.clone()),
    };
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"action\":{},\"confirmationBoundary\":{},\"commandWrapping\":{},\"providerCommands\":{},\"dryRun\":{},\"outcome\":{}",
        request.to_json(),
        json_string(action),
        json_string(&confirmation_boundary(action)),
        command_wrapping_json(),
        provider_commands_json(request),
        plan.to_json(),
        outcome.to_json()
    );
    contract().result_json(
        command,
        "planned",
        "智能体 dry-run 已完成；等待明确确认边界后才能执行允许动作。",
        &extra,
    )
}

fn execute_json(command: &str, request: &CommandRequest) -> String {
    let action = normalize_action(request);
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
            "安装 provider、启动代理或执行构建都需要更高层确认；当前阶段保持阻断。",
            &format!(
                "\"request\":{},\"sideEffects\":[],\"blocked\":true,\"action\":{},\"blockedActions\":{}",
                request.to_json(),
                json_string(action),
                json_array(blocked_actions(action).into_iter().map(|action| json_string(&action)))
            ),
        );
    }

    let extra = match action {
        "install-provider" => match install_provider_assets(request) {
            Ok(report) => format!(
                "\"request\":{},\"sideEffects\":[{}],\"executedAction\":{},{}",
                request.to_json(),
                json_string("write-provider-unrealworkflow-assets"),
                json_string(action),
                report
            ),
            Err(error) => {
                return contract().result_json(
                    command,
                    "failed",
                    "Provider 安装失败；只写 Unreal Workflow 自己的文件，未安装 provider 本体。",
                    &format!(
                        "\"request\":{},\"sideEffects\":[],\"error\":{}",
                        request.to_json(),
                        json_string(&error)
                    ),
                );
            }
        },
        "provider-status" | "validate-contract" => {
            let registry = AgentRegistry::discover();
            format!(
                "\"request\":{},\"sideEffects\":[],\"executedAction\":{},\"agentHub\":{{\"providers\":{},\"roles\":{},\"buildPolicy\":{}}}",
                request.to_json(),
                json_string(action),
                registry.providers_json(),
                roles_json(),
                build_policy_json()
            )
        }
        _ => format!(
            "\"request\":{},\"sideEffects\":[],\"executedAction\":{},\"providerCommands\":{}",
            request.to_json(),
            json_string(action),
            provider_commands_json(request)
        ),
    };
    contract().result_json(
        command,
        "complete",
        "安全智能体动作已完成；只安装或生成 Unreal Workflow 自己的 provider 资产。",
        &extra,
    )
}

fn history_json(command: &str, request: &CommandRequest) -> String {
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"historyRoot\":{},\"history\":[]",
        request.to_json(),
        json_string("%USERPROFILE%/.unrealworkflow/history/agenthub")
    );
    contract().result_json(command, "ready", "智能体历史索引已输出。", &extra)
}

fn artifacts_json(command: &str, request: &CommandRequest) -> String {
    let extra = format!(
        "\"request\":{},\"sideEffects\":[],\"artifactRoot\":{},\"artifacts\":[]",
        request.to_json(),
        json_string("%USERPROFILE%/.unrealworkflow/artifacts/agenthub")
    );
    contract().result_json(command, "ready", "智能体产物索引已输出。", &extra)
}

fn dry_run_plan(action: &str, request: &CommandRequest) -> DryRunPlan {
    DryRunPlan {
        goal: request
            .goal
            .clone()
            .unwrap_or_else(|| format!("agenthub action: {action}")),
        stages: vec![
            DryRunStage {
                id: "select-master".to_string(),
                title: "选择虚幻大师".to_string(),
                summary: "主控 Agent 根据目标选择计划、执行、评审、诊断、知识沉淀角色。"
                    .to_string(),
                safety: SafetyLevel::ReadOnly,
                will_execute: false,
            },
            DryRunStage {
                id: "wrap-provider-command".to_string(),
                title: "生成 provider 命令包".to_string(),
                summary: "只生成目标 provider 可执行任务包，不把所有内容作为 skill 一次性加载。"
                    .to_string(),
                safety: SafetyLevel::ReadOnly,
                will_execute: false,
            },
            DryRunStage {
                id: "block-install".to_string(),
                title: "预览安装和阻断构建".to_string(),
                summary: "只预览 Unreal Workflow agent/command 文件；不安装 provider 本体、不启动外部代理、不运行 UE 构建。".to_string(),
                safety: SafetyLevel::Dangerous,
                will_execute: false,
            },
        ],
        blocked_actions: blocked_actions(action),
        confirmation_boundary: Some(confirmation_boundary(action)),
    }
}

fn normalize_action(request: &CommandRequest) -> &'static str {
    let default_action = match request.provider.as_deref() {
        Some("codex") => "package-codex",
        Some("claude") | Some("claude-code") => "package-claude",
        Some("opencode") => "package-opencode",
        _ => "package-copilot",
    };
    match request.action.as_deref().unwrap_or(default_action) {
        "status" | "provider-status" => "provider-status",
        "validate" | "validate-contract" => "validate-contract",
        "copilot" | "package-copilot" => "package-copilot",
        "codex" | "package-codex" => "package-codex",
        "claude" | "package-claude" => "package-claude",
        "opencode" | "package-opencode" => "package-opencode",
        "install" | "install-provider" => "install-provider",
        "run-master" | "master" => "run-master",
        "diagnose" => "diagnose",
        "review" => "review",
        _ => "package-copilot",
    }
}

fn confirmation_boundary(action: &str) -> String {
    format!("UEWorkflow.AgentHub.{action}.v1")
}

fn is_safe_execute_action(action: &str) -> bool {
    matches!(
        action,
        "provider-status"
            | "validate-contract"
            | "package-copilot"
            | "package-codex"
            | "package-claude"
            | "package-opencode"
            | "install-provider"
    )
}

fn blocked_actions(action: &str) -> Vec<String> {
    let mut actions = vec!["任意 shell".to_string(), "raw UE build".to_string()];
    if action != "install-provider" {
        actions.push("provider 配置写入".to_string());
    }
    if matches!(action, "run-master" | "diagnose" | "review") {
        actions.push("未受控启动外部 provider".to_string());
    }
    actions
}

fn declared_actions_json() -> String {
    let actions = [
        "provider-status",
        "validate-contract",
        "package-copilot",
        "package-codex",
        "package-claude",
        "package-opencode",
        "install-provider",
        "run-master",
        "diagnose",
        "review",
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

fn provider_commands_json(request: &CommandRequest) -> String {
    let provider = request.provider.as_deref().unwrap_or("all");
    let packages = match provider {
        "copilot" => vec![provider_command_json("copilot", request)],
        "codex" => vec![provider_command_json("codex", request)],
        "claude" | "claude-code" => vec![provider_command_json("claude", request)],
        "opencode" => vec![provider_command_json("opencode", request)],
        _ => vec![
            provider_command_json("copilot", request),
            provider_command_json("codex", request),
            provider_command_json("claude", request),
            provider_command_json("opencode", request),
        ],
    };
    json_array(packages)
}

fn provider_command_json(provider: &str, request: &CommandRequest) -> String {
    let aliases = master_agent_aliases_json();
    let command_name = match provider {
        "copilot" => "虚幻大师",
        "codex" => "unreal-master",
        "claude" => "unreal-master",
        "opencode" => "unreal-master",
        _ => "unreal-master",
    };
    let launch_hint = match provider {
        "copilot" => "选择“虚幻大师 / UnrealMaster”Custom Agent 后发送任务包",
        "codex" => "搜索“虚幻大师 / UnrealMaster / unreal-master”，选择 unreal-master agent",
        "claude" => {
            "搜索“虚幻大师 / UnrealMaster / unreal-master”，选择 unreal-master custom agent"
        }
        "opencode" => "搜索“虚幻大师 / UnrealMaster / unreal-master”，选择 unreal-master agent",
        _ => "选择虚幻大师",
    };
    format!(
        "{{\"provider\":{},\"commandName\":{},\"entryRole\":{},\"searchAliases\":{},\"launchHint\":{},\"taskPackage\":{{\"goal\":{},\"masterCommand\":{},\"roles\":{},\"buildPolicy\":{},\"knowledgeClosure\":true,\"oldNamesHidden\":true}}}}",
        json_string(provider),
        json_string(command_name),
        json_string("虚幻大师"),
        aliases,
        json_string(launch_hint),
        json_option(request.goal.as_deref()),
        json_string("uwf master dry-run --compact --json && nextCommands.masterExecute"),
        roles_json(),
        build_policy_json()
    )
}

fn master_agent_json() -> String {
    format!(
        "{{\"displayName\":{},\"englishName\":{},\"stableId\":{},\"aliases\":{},\"role\":{},\"providerNeutral\":true}}",
        json_string("虚幻大师"),
        json_string("UnrealMaster"),
        json_string("unreal-master"),
        master_agent_aliases_json(),
        json_string(ProviderRole::Master.as_str())
    )
}

fn master_agent_aliases_json() -> String {
    json_array(
        ["虚幻大师", "UnrealMaster", "unreal-master"]
            .into_iter()
            .map(json_string),
    )
}

fn roles_json() -> String {
    let roles = [
        ("master", "虚幻大师", "目标理解、阶段编排、收尾闭环"),
        ("planner", "计划 Agent", "需求澄清、证据收集、方案拆分"),
        ("executor", "执行 Agent", "按确认后的任务包做最小安全修改"),
        ("reviewer", "评审 Agent", "只读审查、findings first"),
        ("diagnostician", "诊断 Agent", "失败证据收集和构建策略诊断"),
        (
            "knowledgeCurator",
            "文档沉淀 Agent",
            "复盘、规则、轻量索引写入知识库",
        ),
    ];
    json_array(roles.into_iter().map(|(id, title, summary)| {
        format!(
            "{{\"id\":{},\"title\":{},\"summary\":{}}}",
            json_string(id),
            json_string(title),
            json_string(summary)
        )
    }))
}

fn command_wrapping_json() -> String {
    format!(
        "{{\"skillPollutionAvoided\":true,\"commandsAreProviderPackages\":true,\"manualCommandOverride\":true,\"defaultEntry\":{}}}",
        json_string("虚幻大师")
    )
}

fn build_policy_json() -> String {
    format!(
        "{{\"rawUnrealBuild\":\"disabled\",\"mustRouteThrough\":{},\"projectBuildWrapper\":\"checked-by-devflow-build-project\",\"taskBuildWrapper\":\"checked-by-devflow-build-task\"}}",
        json_string("DevFlow build-check")
    )
}

#[derive(Clone, Debug)]
struct AgentRegistry {
    source_available: bool,
    providers: Vec<ProviderStatus>,
}

impl AgentRegistry {
    fn discover() -> Self {
        let source_root = source_root();
        let providers = vec![
            provider_status("copilot", ".copilot/agents", &source_root),
            provider_status("codex", ".codex/agents", &source_root),
            provider_status("claude", ".claude/agents/custom/ue-master", &source_root),
            provider_status("opencode", ".config/opencode/agents", &source_root),
        ];
        Self {
            source_available: source_root.as_deref().is_some_and(Path::is_dir),
            providers,
        }
    }

    fn providers_json(&self) -> String {
        let source_available = self.source_available;
        json_array(
            self.providers
                .iter()
                .map(move |provider| provider.to_json(source_available)),
        )
    }
}

#[derive(Clone, Debug)]
struct ProviderStatus {
    id: &'static str,
    source_role_count: usize,
    installed: bool,
}

impl ProviderStatus {
    fn to_json(&self, source_available: bool) -> String {
        format!(
            "{{\"id\":{},\"displayName\":{},\"sourceAvailable\":{},\"sourceRoleCount\":{},\"installed\":{},\"status\":{}}}",
            json_string(self.id),
            json_string(provider_display_name(self.id)),
            source_available,
            self.source_role_count,
            self.installed,
            json_string(if self.source_role_count > 0 { "ready" } else { "missingSource" })
        )
    }
}

fn provider_status(
    id: &'static str,
    install_relative_path: &str,
    source_root: &Option<PathBuf>,
) -> ProviderStatus {
    ProviderStatus {
        id,
        source_role_count: source_root
            .as_ref()
            .map(|root| count_provider_roles(root, id))
            .unwrap_or(0),
        installed: provider_install_root()
            .map(|home| home.join(install_relative_path).exists())
            .unwrap_or(false),
    }
}

fn install_provider_assets(request: &CommandRequest) -> Result<String, String> {
    let root = provider_install_root().ok_or_else(|| {
        "USERPROFILE is not available; cannot install provider assets".to_string()
    })?;
    let cli_launcher = install_cli_launcher(&root)?;
    let providers = selected_providers(request.provider.as_deref())?;
    let mut installed = Vec::new();
    for provider in providers {
        installed.push(install_provider_asset(&root, provider)?);
    }
    Ok(format!(
        "\"providerInstall\":{},\"cliLauncher\":{}",
        json_array(installed),
        json_string(&cli_launcher)
    ))
}

fn selected_providers(provider: Option<&str>) -> Result<Vec<&'static str>, String> {
    match provider.unwrap_or("all") {
        "all" => Ok(vec!["copilot", "codex", "claude", "opencode"]),
        "copilot" => Ok(vec!["copilot"]),
        "codex" => Ok(vec!["codex"]),
        "claude" | "claude-code" => Ok(vec!["claude"]),
        "opencode" => Ok(vec!["opencode"]),
        value => Err(format!("unsupported provider: {value}")),
    }
}

fn install_provider_asset(root: &Path, provider: &str) -> Result<String, String> {
    let target = provider_target(provider)?;
    let directory = root.join(target.relative_dir);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("create provider directory failed: {error}"))?;
    let mut files = Vec::new();
    for asset in target.assets {
        let path = directory.join(asset.file_name);
        fs::write(&path, asset.content)
            .map_err(|error| format!("write provider asset failed: {error}"))?;
        files.push(path_to_json_path(&path));
    }
    Ok(format!(
        "{{\"provider\":{},\"displayName\":{},\"targetDir\":{},\"installedFiles\":{},\"providerBinaryInstalled\":false}}",
        json_string(provider),
        json_string(provider_display_name(provider)),
        json_string(&path_to_json_path(&directory)),
        json_array(files.into_iter().map(|path| json_string(&path)))
    ))
}

fn install_cli_launcher(root: &Path) -> Result<String, String> {
    let bin_dir = root.join(".unrealworkflow").join("bin");
    fs::create_dir_all(&bin_dir)
        .map_err(|error| format!("create uwf launcher directory failed: {error}"))?;
    let current_exe = env::current_exe()
        .map_err(|error| format!("resolve current uwf executable failed: {error}"))?;
    let launcher = bin_dir.join("uwf.cmd");
    let content = format!("@echo off\r\n\"{}\" %*\r\n", current_exe.display());
    fs::write(&launcher, content).map_err(|error| format!("write uwf launcher failed: {error}"))?;
    Ok(path_to_json_path(&launcher))
}

struct ProviderTarget {
    relative_dir: &'static str,
    assets: &'static [ProviderAsset],
}

struct ProviderAsset {
    file_name: &'static str,
    content: &'static str,
}

fn provider_target(provider: &str) -> Result<ProviderTarget, String> {
    match provider {
        "copilot" => Ok(ProviderTarget {
            relative_dir: ".copilot/agents",
            assets: &[ProviderAsset {
                file_name: "UnrealMaster.agent.md",
                content: COPILOT_UNREAL_MASTER_AGENT,
            }],
        }),
        "codex" => Ok(ProviderTarget {
            relative_dir: ".codex/agents",
            assets: &[ProviderAsset {
                file_name: "unreal-master.toml",
                content: CODEX_UNREAL_MASTER_AGENT,
            }],
        }),
        "claude" => Ok(ProviderTarget {
            relative_dir: ".claude/agents/custom",
            assets: &[ProviderAsset {
                file_name: "unreal-master.md",
                content: CLAUDE_UNREAL_MASTER_AGENT,
            }],
        }),
        "opencode" => Ok(ProviderTarget {
            relative_dir: ".config/opencode/agents",
            assets: &[ProviderAsset {
                file_name: "unreal-master.md",
                content: OPENCODE_UNREAL_MASTER_AGENT,
            }],
        }),
        value => Err(format!("unsupported provider: {value}")),
    }
}

const COPILOT_UNREAL_MASTER_AGENT: &str = r#"---
name: 虚幻大师
description: 虚幻大师 / UnrealMaster / unreal-master - Use Unreal Workflow through uwf for Unreal project tasks.
---

workflow=unrealworkflow
contractVersion=uwf.provider-agent.v1
provider=copilot
role=UnrealMaster
aliases=虚幻大师,UnrealMaster,unreal-master

You are 虚幻大师, the Unreal Workflow master agent.
Hard gate: before reading or editing repository files, resolve the Unreal Workflow CLI.
PowerShell preflight:
`$uwfBin=Join-Path $env:USERPROFILE ".unrealworkflow\bin"; $udfBin=Join-Path $env:USERPROFILE ".unrealdevflow\bin"; $env:PATH=@($uwfBin,$udfBin,$env:PATH) -join ";"; if (-not $env:UNREALDEVFLOW_CONFIG_DIR -or -not (Test-Path (Join-Path $env:UNREALDEVFLOW_CONFIG_DIR "config.toml"))) { $env:UNREALDEVFLOW_CONFIG_DIR=Join-Path $env:USERPROFILE ".unrealdevflow" }; $uwf=(Get-Command uwf -ErrorAction SilentlyContinue).Source; if (-not $uwf -and (Test-Path (Join-Path $uwfBin "uwf.cmd"))) { $uwf=Join-Path $uwfBin "uwf.cmd" }; if (-not $uwf) { Write-Error "UWF_NOT_AVAILABLE: stop and report to AgentWatcher"; exit 10 }; & $uwf doctor --json`
If the preflight fails, stop. Do not continue manually.
Do not run arbitrary shell commands.
Do not run raw UE builds or Build.bat directly.
Start by producing or requesting `uwf master dry-run --compact --json` through the resolved CLI.
When the user provides task text, build the dry-run command with explicit flags instead of running an empty dry-run:
`& $uwf master dry-run --goal "<user goal>" --workspace "<workspace>" --main-project "<uproject dir>" --host-root "<Hosts dir>" --primary-path "<plugin path[,plugin path]>" --id "<task-id>" --provider "<provider>" --compact --json`
Use `--id` for the DevFlow task id; it is the same business identifier as `--task-id`.
After dry-run succeeds, run `nextCommands.masterExecute` exactly. Do not run `uwf dev schema`, do not inspect UWF source files, and do not read workflow memory to decide whether to bypass UWF.
If required fields are missing, ask for those fields before executing. Never fall back to direct `unrealdevflow` CLI when uwf blocks; report the exact uwf JSON error to AgentWatcher.
Use `uwf dev build-check --json` for build strategy checks.
For real validation, prefer full `unrealdevflow build <task-ref>` from the DevFlow Host. Do not use `--primary-only` as the first or final validation shortcut unless the user explicitly asks for a quick module-only probe.
Never replace a missing uwf/DevFlow path with direct source edits, direct Host creation, or raw Unreal build commands.
Use provider commands as wrappers around uwf, not as broad always-loaded skills.
Keep AgentWatcher taskId/runId markers in every handoff when provided.
"#;

const CODEX_UNREAL_MASTER_AGENT: &str = r#"name = "unreal-master"
description = "虚幻大师 / UnrealMaster / unreal-master - Use Unreal Workflow through uwf for Unreal project tasks."
prompt = """
workflow=unrealworkflow
contractVersion=uwf.provider-agent.v1
provider=codex
role=UnrealMaster
aliases=虚幻大师,UnrealMaster,unreal-master

You are 虚幻大师, the Unreal Workflow master agent.
Hard gate: before reading or editing repository files, resolve the Unreal Workflow CLI.
PowerShell preflight:
`$uwfBin=Join-Path $env:USERPROFILE ".unrealworkflow\bin"; $udfBin=Join-Path $env:USERPROFILE ".unrealdevflow\bin"; $env:PATH=@($uwfBin,$udfBin,$env:PATH) -join ";"; if (-not $env:UNREALDEVFLOW_CONFIG_DIR -or -not (Test-Path (Join-Path $env:UNREALDEVFLOW_CONFIG_DIR "config.toml"))) { $env:UNREALDEVFLOW_CONFIG_DIR=Join-Path $env:USERPROFILE ".unrealdevflow" }; $uwf=(Get-Command uwf -ErrorAction SilentlyContinue).Source; if (-not $uwf -and (Test-Path (Join-Path $uwfBin "uwf.cmd"))) { $uwf=Join-Path $uwfBin "uwf.cmd" }; if (-not $uwf) { Write-Error "UWF_NOT_AVAILABLE: stop and report to AgentWatcher"; exit 10 }; & $uwf doctor --json`
If the preflight fails, stop. Do not continue manually.
Do not run arbitrary shell commands.
Do not run raw UE builds or Build.bat directly.
Start by producing or requesting `uwf master dry-run --compact --json` through the resolved CLI.
When the user provides task text, build the dry-run command with explicit flags instead of running an empty dry-run:
`& $uwf master dry-run --goal "<user goal>" --workspace "<workspace>" --main-project "<uproject dir>" --host-root "<Hosts dir>" --primary-path "<plugin path[,plugin path]>" --id "<task-id>" --provider "<provider>" --compact --json`
Use `--id` for the DevFlow task id; it is the same business identifier as `--task-id`.
After dry-run succeeds, run `nextCommands.masterExecute` exactly. Do not run `uwf dev schema`, do not inspect UWF source files, and do not read workflow memory to decide whether to bypass UWF.
If required fields are missing, ask for those fields before executing. Never fall back to direct `unrealdevflow` CLI when uwf blocks; report the exact uwf JSON error to AgentWatcher.
Use `uwf dev build-check --json` for build strategy checks.
For real validation, prefer full `unrealdevflow build <task-ref>` from the DevFlow Host. Do not use `--primary-only` as the first or final validation shortcut unless the user explicitly asks for a quick module-only probe.
Never replace a missing uwf/DevFlow path with direct source edits, direct Host creation, or raw Unreal build commands.
Use provider commands as wrappers around uwf, not as broad always-loaded skills.
Keep AgentWatcher taskId/runId markers in every handoff when provided.
"""
"#;

const CLAUDE_UNREAL_MASTER_AGENT: &str = r#"---
name: unreal-master
description: 虚幻大师 / UnrealMaster / unreal-master - Use Unreal Workflow through uwf for Unreal project tasks.
---

workflow=unrealworkflow
contractVersion=uwf.provider-agent.v1
provider=claude
role=UnrealMaster
aliases=虚幻大师,UnrealMaster,unreal-master

You are 虚幻大师, the Unreal Workflow master agent.
Hard gate: before reading or editing repository files, resolve the Unreal Workflow CLI.
PowerShell preflight:
`$uwfBin=Join-Path $env:USERPROFILE ".unrealworkflow\bin"; $udfBin=Join-Path $env:USERPROFILE ".unrealdevflow\bin"; $env:PATH=@($uwfBin,$udfBin,$env:PATH) -join ";"; if (-not $env:UNREALDEVFLOW_CONFIG_DIR -or -not (Test-Path (Join-Path $env:UNREALDEVFLOW_CONFIG_DIR "config.toml"))) { $env:UNREALDEVFLOW_CONFIG_DIR=Join-Path $env:USERPROFILE ".unrealdevflow" }; $uwf=(Get-Command uwf -ErrorAction SilentlyContinue).Source; if (-not $uwf -and (Test-Path (Join-Path $uwfBin "uwf.cmd"))) { $uwf=Join-Path $uwfBin "uwf.cmd" }; if (-not $uwf) { Write-Error "UWF_NOT_AVAILABLE: stop and report to AgentWatcher"; exit 10 }; & $uwf doctor --json`
If the preflight fails, stop. Do not continue manually.
Do not run arbitrary shell commands.
Do not run raw UE builds or Build.bat directly.
Start by producing or requesting `uwf master dry-run --compact --json` through the resolved CLI.
When the user provides task text, build the dry-run command with explicit flags instead of running an empty dry-run:
`& $uwf master dry-run --goal "<user goal>" --workspace "<workspace>" --main-project "<uproject dir>" --host-root "<Hosts dir>" --primary-path "<plugin path[,plugin path]>" --id "<task-id>" --provider "<provider>" --compact --json`
Use `--id` for the DevFlow task id; it is the same business identifier as `--task-id`.
After dry-run succeeds, run `nextCommands.masterExecute` exactly. Do not run `uwf dev schema`, do not inspect UWF source files, and do not read workflow memory to decide whether to bypass UWF.
If required fields are missing, ask for those fields before executing. Never fall back to direct `unrealdevflow` CLI when uwf blocks; report the exact uwf JSON error to AgentWatcher.
Use `uwf dev build-check --json` for build strategy checks.
For real validation, prefer full `unrealdevflow build <task-ref>` from the DevFlow Host. Do not use `--primary-only` as the first or final validation shortcut unless the user explicitly asks for a quick module-only probe.
Never replace a missing uwf/DevFlow path with direct source edits, direct Host creation, or raw Unreal build commands.
Use provider commands as wrappers around uwf, not as broad always-loaded skills.
Keep AgentWatcher taskId/runId markers in every handoff when provided.
"#;

const OPENCODE_UNREAL_MASTER_AGENT: &str = r#"---
name: unreal-master
description: 虚幻大师 / UnrealMaster / unreal-master - Use Unreal Workflow through uwf for Unreal project tasks.
---

workflow=unrealworkflow
contractVersion=uwf.provider-agent.v1
provider=opencode
role=UnrealMaster
aliases=虚幻大师,UnrealMaster,unreal-master

You are 虚幻大师, the Unreal Workflow master agent.
Hard gate: before reading or editing repository files, resolve the Unreal Workflow CLI.
PowerShell preflight:
`$uwfBin=Join-Path $env:USERPROFILE ".unrealworkflow\bin"; $udfBin=Join-Path $env:USERPROFILE ".unrealdevflow\bin"; $env:PATH=@($uwfBin,$udfBin,$env:PATH) -join ";"; if (-not $env:UNREALDEVFLOW_CONFIG_DIR -or -not (Test-Path (Join-Path $env:UNREALDEVFLOW_CONFIG_DIR "config.toml"))) { $env:UNREALDEVFLOW_CONFIG_DIR=Join-Path $env:USERPROFILE ".unrealdevflow" }; $uwf=(Get-Command uwf -ErrorAction SilentlyContinue).Source; if (-not $uwf -and (Test-Path (Join-Path $uwfBin "uwf.cmd"))) { $uwf=Join-Path $uwfBin "uwf.cmd" }; if (-not $uwf) { Write-Error "UWF_NOT_AVAILABLE: stop and report to AgentWatcher"; exit 10 }; & $uwf doctor --json`
If the preflight fails, stop. Do not continue manually.
Do not run arbitrary shell commands.
Do not run raw UE builds or Build.bat directly.
Start by producing or requesting `uwf master dry-run --compact --json` through the resolved CLI.
When the user provides task text, build the dry-run command with explicit flags instead of running an empty dry-run:
`& $uwf master dry-run --goal "<user goal>" --workspace "<workspace>" --main-project "<uproject dir>" --host-root "<Hosts dir>" --primary-path "<plugin path[,plugin path]>" --id "<task-id>" --provider "<provider>" --compact --json`
Use `--id` for the DevFlow task id; it is the same business identifier as `--task-id`.
After dry-run succeeds, run `nextCommands.masterExecute` exactly. Do not run `uwf dev schema`, do not inspect UWF source files, and do not read workflow memory to decide whether to bypass UWF.
If required fields are missing, ask for those fields before executing. Never fall back to direct `unrealdevflow` CLI when uwf blocks; report the exact uwf JSON error to AgentWatcher.
Use `uwf dev build-check --json` for build strategy checks.
For real validation, prefer full `unrealdevflow build <task-ref>` from the DevFlow Host. Do not use `--primary-only` as the first or final validation shortcut unless the user explicitly asks for a quick module-only probe.
Never replace a missing uwf/DevFlow path with direct source edits, direct Host creation, or raw Unreal build commands.
Use provider commands as wrappers around uwf, not as broad always-loaded skills.
Keep AgentWatcher taskId/runId markers in every handoff when provided.
"#;

fn count_provider_roles(root: &Path, id: &str) -> usize {
    let path = match id {
        "copilot" => root.join("profiles").join("copilot-vscode").join("agents"),
        "codex" => root.join("profiles").join("codex").join("agents"),
        "claude" => root
            .join("profiles")
            .join("claude-code")
            .join("agents")
            .join("custom"),
        "opencode" => root.join("profiles").join("opencode").join("agents"),
        _ => root.to_path_buf(),
    };
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| entry.path().is_file())
                .count()
        })
        .unwrap_or(0)
}

fn provider_display_name(id: &str) -> &str {
    match id {
        "copilot" => "VSCode Copilot",
        "codex" => "Codex",
        "claude" => "Claude Code",
        "opencode" => "OpenCode",
        _ => id,
    }
}

fn source_root() -> Option<PathBuf> {
    if let Ok(path) = env::var("UWF_AGENTHUB_SOURCE_ROOT") {
        let path = PathBuf::from(path);
        if path.is_dir() {
            return Some(path);
        }
    }
    if let Ok(current_dir) = env::current_dir() {
        let worktree = current_dir
            .join(".plugin-worktrees")
            .join("UEWorkflow")
            .join("AgentHub");
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
    ["UE", "Master", "Agent"].join("_")
}

fn provider_install_root() -> Option<PathBuf> {
    env::var("UWF_PROVIDER_INSTALL_ROOT")
        .or_else(|_| env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

fn path_to_json_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_agents_command_group() {
        let contract = contract();
        assert_eq!(contract.id.command_group(), "uwf agents");
        assert!(contract
            .capabilities
            .iter()
            .any(|capability| capability.id == "role.registry"));
    }

    #[test]
    fn dry_run_generates_provider_command_package() {
        let request = CommandRequest {
            goal: Some("修复插件构建失败".to_string()),
            provider: Some("copilot".to_string()),
            action: Some("package-copilot".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("dry-run", &request);
        assert!(json.contains("\"status\":\"planned\""));
        assert!(json.contains("\"provider\":\"copilot\""));
        assert!(json.contains("\"entryRole\":\"虚幻大师\""));
        assert!(
            json.contains("\"searchAliases\":[\"虚幻大师\",\"UnrealMaster\",\"unreal-master\"]")
        );
        assert!(json.contains("\"commandsAreProviderPackages\":true"));
    }

    #[test]
    fn dry_run_default_action_tracks_requested_provider() {
        let request = CommandRequest {
            provider: Some("codex".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("dry-run", &request);
        assert!(json.contains("\"action\":\"package-codex\""));
        assert!(json.contains("\"confirmationBoundary\":\"UEWorkflow.AgentHub.package-codex.v1\""));
        assert!(json.contains("\"provider\":\"codex\""));
        assert!(json.contains("\"commandName\":\"unreal-master\""));
    }

    #[test]
    fn execute_requires_confirmation_boundary() {
        let request = CommandRequest {
            action: Some("package-codex".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("execute", &request);
        assert!(json.contains("\"status\":\"blocked\""));
        assert!(json.contains("\"requiredConfirmation\":\"UEWorkflow.AgentHub.package-codex.v1\""));
    }

    #[test]
    fn provider_install_requires_confirmation() {
        let request = CommandRequest {
            action: Some("install-provider".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("execute", &request);
        assert!(json.contains("\"status\":\"blocked\""));
        assert!(
            json.contains("\"requiredConfirmation\":\"UEWorkflow.AgentHub.install-provider.v1\"")
        );
    }

    #[test]
    fn provider_install_writes_only_unrealworkflow_assets() {
        let root = temp_provider_root("install-all");
        env::set_var("UWF_PROVIDER_INSTALL_ROOT", &root);
        let request = CommandRequest {
            action: Some("install-provider".to_string()),
            confirmation: Some("UEWorkflow.AgentHub.install-provider.v1".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("execute", &request);

        assert!(json.contains("\"status\":\"complete\""));
        assert!(root.join(".unrealworkflow/bin/uwf.cmd").is_file());
        assert!(json.contains("\"cliLauncher\""));
        assert!(json.contains("\"providerBinaryInstalled\":false"));
        for path in [
            ".copilot/agents/UnrealMaster.agent.md",
            ".codex/agents/unreal-master.toml",
            ".claude/agents/custom/unreal-master.md",
            ".config/opencode/agents/unreal-master.md",
        ] {
            let file = root.join(path);
            assert!(file.is_file(), "{} should exist", file.display());
            let content = fs::read_to_string(file).unwrap();
            assert!(content.contains("workflow=unrealworkflow"));
            assert!(content.contains("虚幻大师"));
            assert!(content.contains("UnrealMaster"));
            assert!(content.contains("unreal-master"));
            assert!(content.contains("uwf master dry-run --compact --json"));
            assert!(content.contains("nextCommands.masterExecute"));
            assert!(content.contains("UWF_NOT_AVAILABLE"));
            assert!(content.contains("Do not continue manually"));
        }

        let _ = fs::remove_dir_all(&root);
        env::remove_var("UWF_PROVIDER_INSTALL_ROOT");
    }

    #[test]
    fn execute_can_generate_codex_package_without_writes() {
        let request = CommandRequest {
            action: Some("package-codex".to_string()),
            provider: Some("codex".to_string()),
            confirmation: Some("UEWorkflow.AgentHub.package-codex.v1".to_string()),
            ..CommandRequest::default()
        };
        let json = command_json("execute", &request);
        assert!(json.contains("\"status\":\"complete\""));
        assert!(json.contains("\"sideEffects\":[]"));
        assert!(json.contains("\"provider\":\"codex\""));
    }

    #[test]
    fn ai_task_actions_remain_provider_agent_only() {
        for (action, confirmation) in [
            ("run-master", "UEWorkflow.AgentHub.run-master.v1"),
            ("diagnose", "UEWorkflow.AgentHub.diagnose.v1"),
            ("review", "UEWorkflow.AgentHub.review.v1"),
        ] {
            let request = CommandRequest {
                action: Some(action.to_string()),
                confirmation: Some(confirmation.to_string()),
                ..CommandRequest::default()
            };
            let json = command_json("execute", &request);
            assert!(json.contains("\"status\":\"blocked\""));
            assert!(json.contains("外部 provider"));
            assert!(json.contains("\"blocked\":true"));
        }
    }

    fn temp_provider_root(label: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "uwf-agenthub-provider-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system time should be after unix epoch")
                .as_nanos()
        ))
    }
}
