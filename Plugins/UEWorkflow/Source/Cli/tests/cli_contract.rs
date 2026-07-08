use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn run_uwf(args: &[&str]) -> String {
    let output = uwf_command(args).output().expect("uwf binary should run");
    assert!(
        output.status.success(),
        "uwf failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout should be utf-8")
}

fn run_uwf_json(args: &[&str]) -> Value {
    serde_json::from_str(&run_uwf(args)).expect("uwf output should be valid JSON")
}

fn run_uwf_with_env(args: &[&str], envs: &[(&str, &Path)]) -> String {
    let mut command = uwf_command(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command.output().expect("uwf binary should run");
    assert!(
        output.status.success(),
        "uwf failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout should be utf-8")
}

fn run_uwf_json_with_env(args: &[&str], envs: &[(&str, &Path)]) -> Value {
    serde_json::from_str(&run_uwf_with_env(args, envs)).expect("uwf output should be valid JSON")
}

fn run_uwf_failure(args: &[&str]) -> String {
    let output = uwf_command(args).output().expect("uwf binary should run");
    assert!(
        !output.status.success(),
        "uwf unexpectedly succeeded: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout should be utf-8")
}

fn uwf_command(args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_uwf"));
    command.args(args);
    command
}

struct TempKnowledgeRoot(PathBuf);

impl TempKnowledgeRoot {
    fn new(label: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        Self(std::env::temp_dir().join(format!("uwf-cli-kb-{label}-{}-{now}", std::process::id())))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempKnowledgeRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct TempProviderRoot(PathBuf);

impl TempProviderRoot {
    fn new(label: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "uwf-cli-provider-{label}-{}-{now}",
            std::process::id()
        )))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempProviderRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct TempDevFlowAdapter {
    root: PathBuf,
    exe: PathBuf,
    config_dir: PathBuf,
    host_root: PathBuf,
    project_root: PathBuf,
    primary_source: PathBuf,
    primary_worktree: PathBuf,
    args_log: PathBuf,
}

impl TempDevFlowAdapter {
    fn new(label: &str, workspace: &str, task_id: &str, primary: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "uwf-cli-devflow-{label}-{}-{now}",
            std::process::id()
        ));
        let config_dir = root.join("config");
        let hosts_root = root.join("Hosts");
        let project_root = root.join("Project");
        let plugins_root = root.join("Plugins");
        let primary_source = plugins_root.join(primary);
        let host_path = hosts_root
            .join(format!("W-{workspace}"))
            .join(format!("T-{task_id}_Host"));
        let primary_worktree = host_path.join("Plugins").join(primary);
        fs::create_dir_all(&config_dir).expect("config dir should be created");
        fs::create_dir_all(&project_root).expect("project root should be created");
        fs::create_dir_all(&primary_source).expect("primary source dir should be created");
        fs::create_dir_all(root.join("bin")).expect("bin dir should be created");
        fs::write(
            project_root.join("Project.uproject"),
            serde_json::json!({
                "FileVersion": 3,
                "EngineAssociation": "5.5"
            })
            .to_string(),
        )
        .expect("project uproject should be written");
        fs::write(
            primary_source.join(format!("{primary}.uplugin")),
            serde_json::json!({
                "FileVersion": 3,
                "FriendlyName": primary,
                "Modules": [{ "Name": primary, "Type": "Runtime" }]
            })
            .to_string(),
        )
        .expect("primary uplugin should be written");
        fs::write(
            config_dir.join("config.toml"),
            format!(
                "[workspaces.{workspace}]\n\
                 hosts_root = '{}'\n\
                 plugin_path = '{}'\n\
                 default_project = '{}'\n\
                 engine_path = '{}'\n\
                 plugins_root = '{}'\n",
                toml_path(&hosts_root),
                toml_path(&primary_source),
                toml_path(&project_root),
                toml_path(&root.join("Engine")),
                toml_path(&plugins_root)
            ),
        )
        .expect("config should be written");
        let list_json = serde_json::json!({
            "tasks": [{
                "schema_version": 3,
                "id": task_id,
                "name": "contract smoke",
                "branch": format!("task/{workspace}/{task_id}"),
                "based_on": "abc123",
                "status": "active",
                "prompt": "contract smoke",
                "primary_plugins": [{
                    "name": primary,
                    "source_repo": primary_source,
                    "worktree": format!("Plugins\\{primary}"),
                    "branch": format!("task/{workspace}/{task_id}"),
                    "based_on": "abc123"
                }],
                "dependency_plugins": [{
                    "name": "GeometryProcessing",
                    "source": "engine",
                    "source_path": root.join("Engine/Plugins/Runtime/GeometryProcessing")
                }],
                "workspace": workspace,
                "task_uid": format!("{workspace}/{task_id}"),
                "context": {
                    "workspace": workspace,
                    "hosts_root": hosts_root,
                    "plugin_path": primary_source,
                    "default_project": root.join("Project"),
                    "engine_path": root.join("Engine"),
                    "plugins_root": plugins_root
                }
            }]
        });
        fs::write(root.join("list.json"), list_json.to_string()).expect("list json should exist");
        let exe = root.join("bin").join("unrealdevflow.cmd");
        let args_log = root.join("args.log");
        fs::write(
            &exe,
            "@echo off\r\n\
             if \"%1\"==\"workspace\" (\r\n\
             echo %*>>\"%~dp0..\\args.log\"\r\n\
             echo workspace updated\r\n\
             exit /b 0\r\n\
             )\r\n\
             if \"%1\"==\"create\" (\r\n\
             echo %*>>\"%~dp0..\\args.log\"\r\n\
             if exist \"%~dp0fail_create_existing.flag\" goto create_exists\r\n\
             echo created\r\n\
             exit /b 0\r\n\
             )\r\n\
             if \"%1\"==\"switch\" (\r\n\
             echo %*>>\"%~dp0..\\args.log\"\r\n\
             echo switched %2\r\n\
             exit /b 0\r\n\
             )\r\n\
             if \"%1\"==\"build\" (\r\n\
             echo %*>>\"%~dp0..\\args.log\"\r\n\
             echo built %2\r\n\
             exit /b 0\r\n\
             )\r\n\
             if \"%1\"==\"list\" (\r\n\
             type \"%~dp0..\\list.json\"\r\n\
             exit /b 0\r\n\
             )\r\n\
             if \"%1\"==\"--version\" (\r\n\
             echo DevFlowAdapter fake\r\n\
             exit /b 0\r\n\
             )\r\n\
             goto fallback\r\n\
             :create_exists\r\n\
             echo Task already exists 1>&2\r\n\
             exit /b 1\r\n\
             :fallback\r\n\
             echo {}\r\n\
             exit /b 0\r\n",
        )
        .expect("fake adapter should be written");
        Self {
            root,
            exe,
            config_dir,
            host_root: hosts_root,
            project_root,
            primary_source,
            primary_worktree,
            args_log,
        }
    }

    fn exe(&self) -> &Path {
        &self.exe
    }

    fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    fn host_root(&self) -> &Path {
        &self.host_root
    }

    fn project_root(&self) -> &Path {
        &self.project_root
    }

    fn primary_worktree(&self) -> &Path {
        &self.primary_worktree
    }

    fn primary_source(&self) -> &Path {
        &self.primary_source
    }

    fn args_log(&self) -> &Path {
        &self.args_log
    }

    /// cmd 批处理在中文系统上按 OEM 码页（GBK）写 args.log，
    /// 严格 UTF-8 读取会失败；断言只看 ASCII 子串，故用容错读取。
    fn read_args_log(&self) -> String {
        String::from_utf8_lossy(&fs::read(&self.args_log).expect("adapter args should be logged"))
            .into_owned()
    }

    fn duplicate_primary_source(&self, primary: &str) -> PathBuf {
        let path = self.root.join("Plugins").join(format!("{primary}_AI"));
        fs::create_dir_all(&path).expect("duplicate primary dir should be created");
        fs::write(
            path.join(format!("{primary}.uplugin")),
            serde_json::json!({
                "FileVersion": 3,
                "FriendlyName": primary,
                "Modules": [{ "Name": primary, "Type": "Runtime" }]
            })
            .to_string(),
        )
        .expect("duplicate primary uplugin should be written");
        path
    }

    fn clear_workspace_plugin_path(&self) {
        let config_path = self.config_dir.join("config.toml");
        let content = fs::read_to_string(&config_path).expect("config should be readable");
        let filtered = content
            .lines()
            .filter(|line| !line.trim_start().starts_with("plugin_path = "))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(config_path, format!("{filtered}\n")).expect("config should be updated");
    }

    fn write_single_workspace_config(
        &self,
        workspace: &str,
        project: &Path,
        plugin_path: &Path,
        plugins_root: &Path,
    ) {
        fs::write(
            self.config_dir.join("config.toml"),
            format!(
                "[workspaces.{workspace}]\n\
                 hosts_root = '{}'\n\
                 plugin_path = '{}'\n\
                 default_project = '{}'\n\
                 engine_path = '{}'\n\
                 plugins_root = '{}'\n",
                toml_path(&self.host_root),
                toml_path(plugin_path),
                toml_path(project),
                toml_path(&self.root.join("Engine")),
                toml_path(plugins_root)
            ),
        )
        .expect("config should be rewritten");
    }

    fn fail_create_as_existing_task(&self) {
        fs::write(self.root.join("bin").join("fail_create_existing.flag"), "1")
            .expect("existing-task flag should be written");
    }
}

impl Drop for TempDevFlowAdapter {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn toml_path(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

#[test]
fn doctor_json_contract_runs() {
    let output = run_uwf(&["doctor", "--json"]);
    assert!(output.contains("\"kind\":\"UnrealWorkflowDoctor\""));
    assert!(output.contains("\"allowRawUnrealBuild\":false"));
}

#[test]
fn modules_json_contract_lists_business_modules() {
    let output = run_uwf(&["modules", "--json"]);
    assert!(output.contains("\"kind\":\"UnrealWorkflowModules\""));
    assert!(output.contains("\"codeName\":\"DevFlow\""));
    assert!(output.contains("\"codeName\":\"AgentHub\""));
    assert!(output.contains("\"codeName\":\"KnowledgeBase\""));
}

#[test]
fn module_status_json_contract_runs() {
    for group in ["dev", "agents", "kb"] {
        let output = run_uwf(&[group, "status", "--json"]);
        assert!(output.contains("\"kind\":\"UnrealWorkflowModuleResult\""));
        assert!(output.contains("\"command\":\"status\""));
        assert!(output.contains("\"status\":\"ready\""));
    }
}

#[test]
fn dry_run_does_not_execute_external_actions() {
    let output = run_uwf(&["dev", "dry-run", "--goal", "smoke", "--json"]);
    assert!(output.contains("\"willExecute\":false"));
    assert!(output.contains("raw UE build"));
    assert!(output.contains("破坏性删除"));
}

#[test]
fn format_json_alias_uses_machine_contract() {
    let output = run_uwf(&["doctor", "--format", "json"]);
    assert!(output.contains("\"kind\":\"UnrealWorkflowDoctor\""));
    assert!(output.contains("\"status\":\"ok\""));
}

#[test]
fn human_output_is_not_raw_json_for_success() {
    let output = run_uwf(&["doctor"]);
    assert_eq!(
        output.trim(),
        "Unreal Workflow ready. Use --json for the stable machine contract."
    );
}

#[test]
fn all_module_core_commands_return_json_results() {
    let commands = [
        "status",
        "capabilities",
        "commands",
        "schema",
        "dry-run",
        "execute",
        "history",
        "artifacts",
    ];
    for group in ["dev", "agents", "kb"] {
        for command in commands {
            let output = run_uwf(&[group, command, "--json"]);
            assert!(
                output.contains("\"kind\":\"UnrealWorkflowModuleResult\""),
                "{group} {command} did not return module result: {output}"
            );
            assert!(
                output.contains(&format!("\"command\":\"{command}\"")),
                "{group} {command} did not echo command: {output}"
            );
        }
    }
}

#[test]
fn master_commands_return_json_results() {
    let plan = run_uwf(&["master", "plan", "--goal", "smoke", "--json"]);
    assert!(plan.contains("\"kind\":\"UnrealWorkflowMasterResult\""));
    assert!(plan.contains("\"command\":\"master plan\""));

    let dry_run = run_uwf(&["master", "dry-run", "--goal", "smoke", "--json"]);
    assert!(dry_run.contains("\"status\":\"planned\""));
    assert!(dry_run.contains("\"confirmationBoundary\":\"UEWorkflow.UnrealMaster.execute.v1\""));

    let execute = run_uwf(&["master", "execute", "--goal", "smoke", "--json"]);
    assert!(execute.contains("\"status\":\"blocked\""));
    assert!(execute.contains("\"requiredConfirmation\":\"UEWorkflow.UnrealMaster.execute.v1\""));
}

#[test]
fn cli_accepts_unrealdevflow_id_alias_for_task_id() {
    let output = run_uwf_json(&[
        "master",
        "dry-run",
        "--goal",
        "smoke",
        "--workspace",
        "neon-dev1",
        "--id",
        "vegetation-editor",
        "--branch",
        "feature/vegetation_editor",
        "--json",
    ]);

    assert_eq!(output["command"], "master dry-run");
    assert_eq!(output["request"]["taskId"], "vegetation-editor");
}

#[test]
fn compact_master_dry_run_gives_next_command_without_full_module_results() {
    let output_text = run_uwf(&[
        "master",
        "dry-run",
        "--goal",
        "smoke",
        "--workspace",
        "neon-dev1",
        "--id",
        "token-smoke",
        "--provider",
        "copilot",
        "--compact",
        "--json",
    ]);
    let output: Value = serde_json::from_str(&output_text).expect("compact output should be json");

    assert_eq!(output["status"], "planned");
    assert!(output.get("moduleResults").is_none());
    assert!(output["moduleSummary"].is_array());
    assert!(output["nextCommands"]["masterExecute"]
        .as_str()
        .expect("masterExecute command should exist")
        .contains("--compact --json"));
    assert!(output["nextCommands"]["devBuildTask"]
        .as_str()
        .expect("devBuildTask command should exist")
        .contains("--action 'build-task'"));
    assert!(
        output_text.len() < 8_000,
        "compact dry-run should stay small enough for provider prompts: {} bytes",
        output_text.len()
    );
}

#[test]
fn provider_packages_share_master_contract_between_copilot_and_codex() {
    let copilot = run_uwf_json(&[
        "master",
        "dry-run",
        "--goal",
        "验证 provider 契约",
        "--workspace",
        "neon-dev1",
        "--task-id",
        "provider-copilot-check",
        "--provider",
        "copilot",
        "--scope",
        "project_plugin",
        "--json",
    ]);
    let codex = run_uwf_json(&[
        "master",
        "dry-run",
        "--goal",
        "验证 provider 契约",
        "--workspace",
        "neon-dev1",
        "--task-id",
        "provider-codex-check",
        "--provider",
        "codex",
        "--scope",
        "project_plugin",
        "--json",
    ]);

    assert_eq!(copilot["providerPackage"]["provider"], "copilot");
    assert_eq!(codex["providerPackage"]["provider"], "codex");
    assert_eq!(copilot["stages"], codex["stages"]);
    assert_eq!(
        copilot["providerPackage"]["masterCommand"],
        codex["providerPackage"]["masterCommand"]
    );
    assert_eq!(
        copilot["providerPackage"]["moduleCommands"],
        codex["providerPackage"]["moduleCommands"]
    );
    assert_eq!(copilot["blockedActions"], codex["blockedActions"]);

    let copilot_agent = agenthub_result(&copilot);
    let codex_agent = agenthub_result(&codex);
    assert_eq!(
        copilot_agent["commandWrapping"],
        codex_agent["commandWrapping"]
    );
    assert_eq!(copilot_agent["providerCommands"][0]["provider"], "copilot");
    assert_eq!(codex_agent["providerCommands"][0]["provider"], "codex");
    assert_eq!(
        copilot_agent["providerCommands"][0]["taskPackage"]["roles"],
        codex_agent["providerCommands"][0]["taskPackage"]["roles"]
    );
    assert_eq!(
        copilot_agent["providerCommands"][0]["taskPackage"]["buildPolicy"],
        codex_agent["providerCommands"][0]["taskPackage"]["buildPolicy"]
    );
    assert_eq!(
        copilot_agent["providerCommands"][0]["taskPackage"]["knowledgeClosure"],
        codex_agent["providerCommands"][0]["taskPackage"]["knowledgeClosure"]
    );
    assert_eq!(
        copilot_agent["providerCommands"][0]["taskPackage"]["oldNamesHidden"],
        codex_agent["providerCommands"][0]["taskPackage"]["oldNamesHidden"]
    );
}

#[test]
fn safety_contract_blocks_unconfirmed_and_dangerous_actions() {
    let doctor = run_uwf_json(&["doctor", "--json"]);
    assert_eq!(doctor["safety"]["allowArbitraryShell"], false);
    assert_eq!(doctor["safety"]["allowRawUnrealBuild"], false);
    assert_eq!(doctor["safety"]["allowDestructiveDelete"], false);

    let dry_run = run_uwf_json(&[
        "master",
        "dry-run",
        "--goal",
        "安全边界验证",
        "--workspace",
        "neon-dev1",
        "--task-id",
        "safety-contract",
        "--provider",
        "copilot",
        "--json",
    ]);
    assert_eq!(dry_run["willExecute"], false);
    for expected in [
        "任意 shell",
        "raw UE build",
        "破坏性删除",
        "绕过模块直接操作原工程",
    ] {
        assert_json_array_contains(&dry_run["blockedActions"], expected);
    }

    for (args, required_boundary) in [
        (
            vec!["master", "execute", "--goal", "smoke", "--json"],
            "UEWorkflow.UnrealMaster.execute.v1",
        ),
        (
            vec!["dev", "execute", "--action", "build-check", "--json"],
            "UEWorkflow.DevFlow.build-check.v1",
        ),
        (
            vec!["agents", "execute", "--action", "package-codex", "--json"],
            "UEWorkflow.AgentHub.package-codex.v1",
        ),
        (
            vec!["kb", "execute", "--action", "record-task", "--json"],
            "UEWorkflow.KnowledgeBase.record-task.v1",
        ),
        (
            vec![
                "agents",
                "execute",
                "--action",
                "install-provider",
                "--json",
            ],
            "UEWorkflow.AgentHub.install-provider.v1",
        ),
    ] {
        let output = run_uwf_json(&args);
        assert_blocked_with_no_side_effects(&output);
        assert_eq!(output["requiredConfirmation"], required_boundary);
    }

    let provider_root = TempProviderRoot::new("safety-contract");
    let provider_install = run_uwf_json_with_env(
        &[
            "agents",
            "execute",
            "--action",
            "install-provider",
            "--confirm",
            "UEWorkflow.AgentHub.install-provider.v1",
            "--json",
        ],
        &[("UWF_PROVIDER_INSTALL_ROOT", provider_root.path())],
    );
    assert_eq!(provider_install["status"], "complete");
    assert_json_array_contains(
        &provider_install["sideEffects"],
        "write-provider-unrealworkflow-assets",
    );
    let installs = provider_install["providerInstall"]
        .as_array()
        .expect("providerInstall should be an array");
    assert_eq!(installs.len(), 4);
    assert!(installs
        .iter()
        .all(|install| install["providerBinaryInstalled"] == false));
    for expected_file in [
        ".copilot/agents/UnrealMaster.agent.md",
        ".codex/agents/unreal-master.toml",
        ".claude/agents/custom/unreal-master.md",
        ".config/opencode/agents/unreal-master.md",
    ] {
        assert!(
            provider_root.path().join(expected_file).is_file(),
            "{expected_file} should be installed under the isolated provider root"
        );
    }

    for (args, expected_block) in [
        (
            vec![
                "dev",
                "execute",
                "--action",
                "build-task",
                "--confirm",
                "UEWorkflow.DevFlow.build-task.v1",
                "--json",
            ],
            "missing workspace",
        ),
        (
            vec![
                "dev",
                "execute",
                "--action",
                "build-project",
                "--confirm",
                "UEWorkflow.DevFlow.build-project.v1",
                "--json",
            ],
            "UE 编译",
        ),
        (
            vec![
                "dev",
                "execute",
                "--action",
                "delete",
                "--confirm",
                "UEWorkflow.DevFlow.delete.v1",
                "--json",
            ],
            "破坏性删除",
        ),
        (
            vec![
                "agents",
                "execute",
                "--action",
                "run-master",
                "--confirm",
                "UEWorkflow.AgentHub.run-master.v1",
                "--json",
            ],
            "未受控启动外部 provider",
        ),
        (
            vec![
                "kb",
                "execute",
                "--action",
                "generate",
                "--confirm",
                "UEWorkflow.KnowledgeBase.generate.v1",
                "--json",
            ],
            "SQLite/Pickle",
        ),
        (
            vec![
                "kb",
                "execute",
                "--action",
                "export",
                "--confirm",
                "UEWorkflow.KnowledgeBase.export.v1",
                "--json",
            ],
            "重型知识库",
        ),
    ] {
        let output = run_uwf_json(&args);
        assert_blocked_with_no_side_effects(&output);
        assert!(
            output.to_string().contains(expected_block),
            "{expected_block} should be present in blocked output: {output}"
        );
    }
}

#[test]
fn master_execute_only_dispatches_safe_module_actions() {
    let root = TempKnowledgeRoot::new("master-safety");
    let devflow_adapter = TempDevFlowAdapter::new(
        "master-safety",
        "neon-dev1",
        "master-safety-smoke",
        "AesWorld",
    );
    let envs = [
        ("UWF_KNOWLEDGE_ROOT", root.path()),
        ("UWF_DEVFLOW_ADAPTER_EXE", devflow_adapter.exe()),
        ("UNREALDEVFLOW_CONFIG_DIR", devflow_adapter.config_dir()),
    ];
    let output = run_uwf_json_with_env(
        &[
            "master",
            "execute",
            "--goal",
            "验证受控执行边界",
            "--workspace",
            "neon-dev1",
            "--id",
            "master-safety-smoke",
            "--provider",
            "codex",
            "--scope",
            "project_plugin",
            "--project",
            "Neon",
            "--primary",
            "AesWorld",
            "--confirm",
            "UEWorkflow.UnrealMaster.execute.v1",
            "--json",
        ],
        &envs,
    );

    assert_eq!(output["status"], "complete");
    let text = output.to_string();
    for forbidden in [
        "RunUAT",
        "BuildCookRun",
        "UnrealBuildTool",
        "\"rawUnrealBuild\":\"enabled\"",
        "\"executedAction\":\"build-task\"",
        "\"executedAction\":\"build-project\"",
        "\"executedAction\":\"delete\"",
    ] {
        assert!(
            !text.contains(forbidden),
            "{forbidden} must not appear in master execute output: {text}"
        );
    }

    let devflow_result = module_result(&output, "DevFlow");
    assert_eq!(devflow_result["status"], "complete");
    assert_eq!(devflow_result["executedAction"], "create-task");
    assert_json_array_contains(
        &devflow_result["sideEffects"],
        "unreal-workflow-devflow-create-task",
    );
    assert_eq!(
        devflow_result["targetContext"]["workspacePath"],
        devflow_adapter
            .primary_worktree()
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(
        devflow_result["targetContext"]["requiredBeforeProviderExecution"],
        false
    );
    assert_eq!(
        output["providerPackage"]["contextConfirmation"]["workspacePath"],
        devflow_adapter
            .primary_worktree()
            .to_string_lossy()
            .as_ref()
    );
    assert_eq!(devflow_result["buildRouting"]["rawUnrealBuild"], "disabled");

    let agenthub = module_result(&output, "AgentHub");
    assert_eq!(agenthub["status"], "complete");
    assert_eq!(agenthub["executedAction"], "package-codex");
    assert_json_array_is_empty(&agenthub["sideEffects"]);

    let knowledge_base = module_result(&output, "KnowledgeBase");
    assert_eq!(knowledge_base["status"], "complete");
    assert_eq!(knowledge_base["executedAction"], "record-task");
    assert_eq!(knowledge_base["sideEffects"][0], "write-light-knowledge");
}

#[test]
fn devflow_execute_delegates_exact_primary_path_to_workspace_plugin_path() {
    let devflow_adapter = TempDevFlowAdapter::new(
        "exact-primary",
        "neon-dev1",
        "exact-primary-smoke",
        "AesWorld",
    );
    let envs = [
        ("UWF_DEVFLOW_ADAPTER_EXE", devflow_adapter.exe()),
        ("UNREALDEVFLOW_CONFIG_DIR", devflow_adapter.config_dir()),
    ];
    let output = run_uwf_json_with_env(
        &[
            "dev",
            "execute",
            "--action",
            "create-task",
            "--goal",
            "验证精确主插件路径不会被降级成名字",
            "--workspace",
            "neon-dev1",
            "--task-id",
            "exact-primary-smoke",
            "--primary-path",
            devflow_adapter
                .primary_source()
                .to_str()
                .expect("primary source path should be utf-8"),
            "--confirm",
            "UEWorkflow.DevFlow.create-task.v1",
            "--json",
        ],
        &envs,
    );

    assert_eq!(output["status"], "complete");
    assert_eq!(
        output["devFlow"]["createdTask"]["primaryPlugin"],
        "AesWorld"
    );
    assert_eq!(
        output["targetContext"]["workspacePath"],
        devflow_adapter
            .primary_worktree()
            .to_string_lossy()
            .as_ref()
    );
    let args_log = devflow_adapter.read_args_log();
    assert!(
        !args_log.contains("--primary"),
        "exact workspace plugin_path should be delegated to DevFlow instead of downgraded to --primary: {args_log}"
    );
    assert!(args_log.contains("create"));
    assert!(args_log.contains("--workspace neon-dev1"));
}

#[test]
fn devflow_switch_execute_delegates_to_adapter_after_confirmation() {
    let devflow_adapter =
        TempDevFlowAdapter::new("switch", "neon-dev1", "switch-smoke", "AesWorld");
    let envs = [
        ("UWF_DEVFLOW_ADAPTER_EXE", devflow_adapter.exe()),
        ("UNREALDEVFLOW_CONFIG_DIR", devflow_adapter.config_dir()),
    ];
    let output = run_uwf_json_with_env(
        &[
            "dev",
            "execute",
            "--action",
            "switch",
            "--workspace",
            "neon-dev1",
            "--task-id",
            "switch-smoke",
            "--confirm",
            "UEWorkflow.DevFlow.switch.v1",
            "--json",
        ],
        &envs,
    );

    assert_eq!(output["status"], "complete");
    assert_eq!(output["executedAction"], "switch");
    assert_eq!(output["switchContext"]["junctionSwitch"], "performed");
    assert_eq!(output["switchContext"]["taskRef"], "neon-dev1/switch-smoke");
    assert_json_array_contains(&output["sideEffects"], "devflow-switch-junction");
    let args_log = devflow_adapter.read_args_log();
    assert!(
        args_log.contains("switch neon-dev1/switch-smoke"),
        "UWF must delegate real switch to the DevFlow adapter: {args_log}"
    );
    assert!(
        !output.to_string().contains("not-performed"),
        "switch execute must not report fake completion: {output}"
    );
}

#[test]
fn devflow_build_task_execute_delegates_to_adapter_after_confirmation() {
    let devflow_adapter =
        TempDevFlowAdapter::new("build-task", "neon-dev1", "build-smoke", "AesWorld");
    let envs = [
        ("UWF_DEVFLOW_ADAPTER_EXE", devflow_adapter.exe()),
        ("UNREALDEVFLOW_CONFIG_DIR", devflow_adapter.config_dir()),
    ];
    let output = run_uwf_json_with_env(
        &[
            "dev",
            "execute",
            "--action",
            "build-task",
            "--workspace",
            "neon-dev1",
            "--task-id",
            "build-smoke",
            "--confirm",
            "UEWorkflow.DevFlow.build-task.v1",
            "--json",
        ],
        &envs,
    );

    assert_eq!(output["status"], "complete");
    assert_eq!(output["executedAction"], "build-task");
    assert_eq!(
        output["buildResult"]["ueBuild"],
        "performed-through-devflow"
    );
    assert_eq!(output["buildResult"]["rawUnrealBuild"], "disabled");
    assert_eq!(output["buildResult"]["taskRef"], "neon-dev1/build-smoke");
    assert_json_array_contains(&output["sideEffects"], "devflow-build-task");
    let args_log = devflow_adapter.read_args_log();
    assert!(
        args_log.contains("build neon-dev1/build-smoke"),
        "UWF must delegate task build to the DevFlow adapter: {args_log}"
    );
}

#[test]
fn devflow_execute_registers_missing_workspace_plugin_path_via_devflow() {
    let devflow_adapter = TempDevFlowAdapter::new(
        "register-primary",
        "neon-dev1",
        "register-primary-smoke",
        "AesWorld",
    );
    devflow_adapter.clear_workspace_plugin_path();
    let envs = [
        ("UWF_DEVFLOW_ADAPTER_EXE", devflow_adapter.exe()),
        ("UNREALDEVFLOW_CONFIG_DIR", devflow_adapter.config_dir()),
    ];
    let output = run_uwf_json_with_env(
        &[
            "dev",
            "execute",
            "--action",
            "create-task",
            "--goal",
            "验证缺失 workspace plugin_path 时委托 DevFlow 登记",
            "--workspace",
            "neon-dev1",
            "--task-id",
            "register-primary-smoke",
            "--main-project",
            devflow_adapter
                .project_root()
                .to_str()
                .expect("project path should be utf-8"),
            "--host-root",
            devflow_adapter
                .host_root()
                .to_str()
                .expect("host root path should be utf-8"),
            "--primary-path",
            devflow_adapter
                .primary_source()
                .to_str()
                .expect("primary source path should be utf-8"),
            "--confirm",
            "UEWorkflow.DevFlow.create-task.v1",
            "--json",
        ],
        &envs,
    );

    assert_eq!(output["status"], "complete");
    assert_eq!(output["devFlow"]["workspaceBindingUpdated"], true);
    assert_json_array_contains(&output["sideEffects"], "devflow-workspace-binding-updated");
    let args_log = devflow_adapter.read_args_log();
    assert!(
        args_log.contains("workspace add neon-dev1"),
        "UWF should use DevFlow workspace add instead of editing config itself: {args_log}"
    );
    assert!(
        args_log.contains("--plugin-path"),
        "workspace add should bind the precise primary plugin path: {args_log}"
    );
    assert!(
        args_log.contains("--engine-path"),
        "workspace add should pass the registered engine path instead of forcing DevFlow to auto-detect EngineAssociation: {args_log}"
    );
    let create_line = args_log
        .lines()
        .find(|line| line.starts_with("create "))
        .expect("create should be called after workspace add");
    assert!(
        !create_line.contains("--primary"),
        "create should rely on DevFlow workspace plugin_path after registration: {create_line}"
    );
}

#[test]
fn devflow_execute_blocks_invalid_explicit_main_project_instead_of_fallback() {
    let devflow_adapter = TempDevFlowAdapter::new(
        "invalid-main-project",
        "neon-dev1",
        "invalid-main-project-smoke",
        "AesWorld",
    );
    devflow_adapter.clear_workspace_plugin_path();
    let repo_root = devflow_adapter.root.join("neon");
    fs::create_dir_all(&repo_root).expect("repo root should exist without uproject");
    let envs = [
        ("UWF_DEVFLOW_ADAPTER_EXE", devflow_adapter.exe()),
        ("UNREALDEVFLOW_CONFIG_DIR", devflow_adapter.config_dir()),
    ];
    let output = run_uwf_json_with_env(
        &[
            "dev",
            "execute",
            "--action",
            "create-task",
            "--goal",
            "验证 UI 传仓库根时使用 DevFlow workspace 主项目",
            "--workspace",
            devflow_adapter
                .primary_source()
                .to_str()
                .expect("primary source path should be utf-8"),
            "--task-id",
            "invalid-main-project-smoke",
            "--main-project",
            repo_root.to_str().expect("repo root path should be utf-8"),
            "--host-root",
            devflow_adapter
                .host_root()
                .to_str()
                .expect("host root path should be utf-8"),
            "--primary-path",
            devflow_adapter
                .primary_source()
                .to_str()
                .expect("primary source path should be utf-8"),
            "--confirm",
            "UEWorkflow.DevFlow.create-task.v1",
            "--json",
        ],
        &envs,
    );

    assert_eq!(output["status"], "blocked");
    assert!(
        output.to_string().contains("不是 UE 项目目录"),
        "blocked output should explain the invalid explicit main project: {output}"
    );
    assert!(
        !devflow_adapter.args_log().exists(),
        "adapter must not run after an invalid explicit main project"
    );
}

#[test]
fn devflow_execute_retargets_path_workspace_when_explicit_main_project_differs() {
    let devflow_adapter = TempDevFlowAdapter::new(
        "explicit-main-project",
        "neon-dev",
        "explicit-main-project-smoke",
        "AesWorld",
    );
    let repo_root = devflow_adapter.root.join("neon");
    let requested_project = repo_root.join("UGA").join("DEV");
    let stale_project = repo_root.join("UGA").join("DEV_1");
    fs::create_dir_all(&requested_project).expect("requested project should exist");
    fs::create_dir_all(&stale_project).expect("stale project should exist");
    fs::write(
        requested_project.join("UGA.uproject"),
        serde_json::json!({ "FileVersion": 3, "EngineAssociation": "5.5" }).to_string(),
    )
    .expect("requested project should contain uproject");
    fs::write(
        stale_project.join("UGA.uproject"),
        serde_json::json!({ "FileVersion": 3, "EngineAssociation": "5.5" }).to_string(),
    )
    .expect("stale project should contain uproject");
    devflow_adapter.write_single_workspace_config(
        "neon-dev1",
        &stale_project,
        devflow_adapter.primary_source(),
        devflow_adapter
            .primary_source()
            .parent()
            .expect("primary source should have parent"),
    );
    let envs = [
        ("UWF_DEVFLOW_ADAPTER_EXE", devflow_adapter.exe()),
        ("UNREALDEVFLOW_CONFIG_DIR", devflow_adapter.config_dir()),
    ];
    let output = run_uwf_json_with_env(
        &[
            "dev",
            "execute",
            "--action",
            "create-task",
            "--goal",
            "验证显式 DEV 不会被旧 DEV_1 workspace 带偏",
            "--workspace",
            devflow_adapter
                .primary_source()
                .to_str()
                .expect("primary source path should be utf-8"),
            "--task-id",
            "explicit-main-project-smoke",
            "--main-project",
            requested_project
                .to_str()
                .expect("requested project path should be utf-8"),
            "--host-root",
            devflow_adapter
                .host_root()
                .to_str()
                .expect("host root path should be utf-8"),
            "--primary-path",
            devflow_adapter
                .primary_source()
                .to_str()
                .expect("primary source path should be utf-8"),
            "--confirm",
            "UEWorkflow.DevFlow.create-task.v1",
            "--json",
        ],
        &envs,
    );

    assert_eq!(output["status"], "complete");
    assert_eq!(output["devFlow"]["workspaceBindingUpdated"], true);
    assert_eq!(output["devFlow"]["createdTask"]["workspace"], "neon-dev");
    let args_log = devflow_adapter.read_args_log();
    assert!(
        args_log.contains("workspace add neon-dev"),
        "UWF should register the workspace that matches explicit DEV, not reuse neon-dev1: {args_log}"
    );
    assert!(
        args_log.contains(&format!(
            "--project {}",
            requested_project.to_string_lossy()
        )),
        "workspace add should use the explicit DEV project: {args_log}"
    );
    assert!(
        !args_log.contains(&format!("--project {}", stale_project.to_string_lossy())),
        "workspace add must not use the stale DEV_1 project: {args_log}"
    );
    assert!(
        args_log.contains("create "),
        "create should run after workspace add: {args_log}"
    );
    assert!(
        args_log.contains("--workspace neon-dev"),
        "create should target the retargeted neon-dev workspace: {args_log}"
    );
}

#[test]
fn compact_master_execute_returns_created_task_without_full_module_results() {
    let root = TempKnowledgeRoot::new("master-compact");
    let devflow_adapter = TempDevFlowAdapter::new(
        "master-compact",
        "neon-dev1",
        "master-compact-smoke",
        "AesWorld",
    );
    let envs = [
        ("UWF_KNOWLEDGE_ROOT", root.path()),
        ("UWF_DEVFLOW_ADAPTER_EXE", devflow_adapter.exe()),
        ("UNREALDEVFLOW_CONFIG_DIR", devflow_adapter.config_dir()),
    ];
    let output_text = run_uwf_with_env(
        &[
            "master",
            "execute",
            "--goal",
            "验证紧凑执行输出",
            "--workspace",
            "neon-dev1",
            "--id",
            "master-compact-smoke",
            "--provider",
            "copilot",
            "--scope",
            "project_plugin",
            "--project",
            "Neon",
            "--primary",
            "AesWorld",
            "--confirm",
            "UEWorkflow.UnrealMaster.execute.v1",
            "--compact",
            "--json",
        ],
        &envs,
    );
    let output: Value = serde_json::from_str(&output_text).expect("compact output should be json");

    assert_eq!(output["status"], "complete");
    assert!(output.get("moduleResults").is_none());
    assert_eq!(output["createdTask"]["taskId"], "master-compact-smoke");
    assert!(output["moduleSummary"].is_array());
    assert!(
        output_text.len() < 8_000,
        "compact execute should stay small enough for provider prompts: {} bytes",
        output_text.len()
    );
}

#[test]
fn devflow_execute_reuses_existing_task_instead_of_failing_provider_dispatch() {
    let devflow_adapter = TempDevFlowAdapter::new(
        "existing-task",
        "neon-dev1",
        "existing-task-smoke",
        "AesWorld",
    );
    devflow_adapter.fail_create_as_existing_task();
    let envs = [
        ("UWF_DEVFLOW_ADAPTER_EXE", devflow_adapter.exe()),
        ("UNREALDEVFLOW_CONFIG_DIR", devflow_adapter.config_dir()),
    ];
    let output = run_uwf_json_with_env(
        &[
            "master",
            "execute",
            "--goal",
            "验证已存在任务可以幂等继续派发",
            "--workspace",
            "neon-dev1",
            "--task-id",
            "existing-task-smoke",
            "--provider",
            "copilot",
            "--primary-path",
            devflow_adapter
                .primary_source()
                .to_str()
                .expect("primary source path should be utf-8"),
            "--confirm",
            "UEWorkflow.UnrealMaster.execute.v1",
            "--json",
        ],
        &envs,
    );

    assert_eq!(output["status"], "complete");
    let devflow_result = module_result(&output, "DevFlow");
    assert_eq!(devflow_result["status"], "complete");
    assert_eq!(devflow_result["devFlow"]["reusedExistingTask"], true);
    assert_json_array_contains(
        &devflow_result["sideEffects"],
        "devflow-existing-task-reused",
    );
    assert!(
        output["providerPackage"].is_object(),
        "provider package should still be generated when DevFlow reports an existing task: {output}"
    );
}

#[test]
fn devflow_execute_blocks_mismatched_primary_path_before_adapter() {
    let devflow_adapter = TempDevFlowAdapter::new(
        "mismatched-primary",
        "neon-dev1",
        "mismatched-primary-smoke",
        "AesWorld",
    );
    let duplicate = devflow_adapter.duplicate_primary_source("AesWorld");
    let envs = [
        ("UWF_DEVFLOW_ADAPTER_EXE", devflow_adapter.exe()),
        ("UNREALDEVFLOW_CONFIG_DIR", devflow_adapter.config_dir()),
    ];
    let output = run_uwf_json_with_env(
        &[
            "dev",
            "execute",
            "--action",
            "create-task",
            "--goal",
            "验证同名副本不会被误识别",
            "--workspace",
            "neon-dev1",
            "--task-id",
            "mismatched-primary-smoke",
            "--primary-path",
            duplicate
                .to_str()
                .expect("duplicate primary path should be utf-8"),
            "--confirm",
            "UEWorkflow.DevFlow.create-task.v1",
            "--json",
        ],
        &envs,
    );

    assert_eq!(output["status"], "blocked");
    assert_eq!(output["blocked"], true);
    assert!(
        output.to_string().contains("workspace.plugin_path"),
        "blocked output should explain exact plugin_path mismatch: {output}"
    );
    assert!(
        !devflow_adapter.args_log().exists(),
        "adapter create must not be called when primary path mismatches workspace plugin_path"
    );
}

#[test]
fn devflow_dry_run_marks_mismatched_primary_path_as_not_ready() {
    let devflow_adapter = TempDevFlowAdapter::new(
        "mismatched-dry-run",
        "neon-dev1",
        "mismatched-dry-run-smoke",
        "AesWorld",
    );
    let duplicate = devflow_adapter.duplicate_primary_source("AesWorld");
    let envs = [
        ("UWF_DEVFLOW_ADAPTER_EXE", devflow_adapter.exe()),
        ("UNREALDEVFLOW_CONFIG_DIR", devflow_adapter.config_dir()),
    ];
    let output = run_uwf_json_with_env(
        &[
            "dev",
            "dry-run",
            "--action",
            "create-task",
            "--goal",
            "验证预演会阻断错误主插件路径",
            "--workspace",
            "neon-dev1",
            "--task-id",
            "mismatched-dry-run-smoke",
            "--primary-path",
            duplicate
                .to_str()
                .expect("duplicate primary path should be utf-8"),
            "--json",
        ],
        &envs,
    );

    assert_eq!(output["status"], "planned");
    assert_eq!(
        output["targetContext"]["primaryPathResolution"]["status"],
        "blocked"
    );
    assert_eq!(
        output["targetContext"]["requiredBeforeProviderExecution"],
        true
    );
    assert_json_array_contains(&output["targetContext"]["missingBinding"], "主插件");
}

#[test]
fn knowledge_base_record_and_query_scope_closure() {
    let root = TempKnowledgeRoot::new("closure");
    let envs = [("UWF_KNOWLEDGE_ROOT", root.path())];
    let record = run_uwf_json_with_env(
        &[
            "kb",
            "execute",
            "--action",
            "record-task",
            "--goal",
            "沉淀插件规则",
            "--task-id",
            "kb closure smoke",
            "--scope",
            "project_plugin",
            "--project",
            "Neon",
            "--primary",
            "AesWorld",
            "--confirm",
            "UEWorkflow.KnowledgeBase.record-task.v1",
            "--json",
        ],
        &envs,
    );

    assert_eq!(record["status"], "complete");
    assert_eq!(record["executedAction"], "record-task");
    assert_eq!(record["scope"]["scopeKind"], "project_plugin");
    assert_eq!(record["sideEffects"][0], "write-light-knowledge");

    let scope_root = root.path().join("scopes/projects/neon/plugins/aesworld");
    for relative in [
        "scope-manifest.json",
        "retrospectives/kb-closure-smoke.md",
        "solutions/kb-closure-smoke.md",
        "rules/kb-closure-smoke.md",
        "index/light-index.md",
    ] {
        assert!(
            scope_root.join(relative).is_file(),
            "{relative} should be written under the project plugin scope"
        );
    }
    assert!(
        fs::read_to_string(scope_root.join("retrospectives/kb-closure-smoke.md"))
            .expect("retrospective should be readable")
            .contains("任务复盘")
    );
    assert!(
        fs::read_to_string(scope_root.join("solutions/kb-closure-smoke.md"))
            .expect("solution should be readable")
            .contains("方案草案")
    );
    assert!(
        fs::read_to_string(scope_root.join("rules/kb-closure-smoke.md"))
            .expect("rule should be readable")
            .contains("规则候选")
    );

    let query = run_uwf_json_with_env(
        &[
            "kb",
            "execute",
            "--action",
            "query",
            "--goal",
            "沉淀插件规则",
            "--scope",
            "project_plugin",
            "--project",
            "Neon",
            "--primary",
            "AesWorld",
            "--confirm",
            "UEWorkflow.KnowledgeBase.query.v1",
            "--json",
        ],
        &envs,
    );
    let paths: Vec<&str> = query["matches"]
        .as_array()
        .expect("matches should be an array")
        .iter()
        .map(|item| {
            item["path"]
                .as_str()
                .expect("match path should be a string")
        })
        .collect();
    for expected in [
        "retrospectives/kb-closure-smoke.md",
        "solutions/kb-closure-smoke.md",
        "rules/kb-closure-smoke.md",
    ] {
        assert!(
            paths.iter().any(|path| path.contains(expected)),
            "{expected} should be found by scope-bound query: {paths:?}"
        );
    }

    let other_scope_query = run_uwf_json_with_env(
        &[
            "kb",
            "execute",
            "--action",
            "query",
            "--goal",
            "沉淀插件规则",
            "--scope",
            "engine_plugin",
            "--primary",
            "AesWorld",
            "--confirm",
            "UEWorkflow.KnowledgeBase.query.v1",
            "--json",
        ],
        &envs,
    );
    assert_eq!(
        other_scope_query["matches"]
            .as_array()
            .expect("matches should be an array")
            .len(),
        0,
        "query must not cross from project plugin scope into engine plugin scope"
    );
}

fn agenthub_result(master: &Value) -> &Value {
    module_result(master, "AgentHub")
}

fn module_result<'a>(master: &'a Value, code_name: &str) -> &'a Value {
    master["moduleResults"]
        .as_array()
        .expect("moduleResults should be an array")
        .iter()
        .find(|item| item["module"]["codeName"] == code_name)
        .unwrap_or_else(|| panic!("master result should include {code_name} module result"))
}

fn assert_blocked_with_no_side_effects(output: &Value) {
    assert_eq!(output["status"], "blocked");
    assert_json_array_is_empty(&output["sideEffects"]);
}

fn assert_json_array_is_empty(value: &Value) {
    assert_eq!(
        value
            .as_array()
            .expect("value should be a JSON array")
            .len(),
        0
    );
}

fn assert_json_array_contains(value: &Value, expected: &str) {
    assert!(
        value
            .as_array()
            .expect("value should be a JSON array")
            .iter()
            .any(|item| item.as_str() == Some(expected)),
        "{expected} should be present in {value}"
    );
}

#[test]
fn unknown_command_returns_machine_error() {
    let output = run_uwf_failure(&["nonsense", "--json"]);
    assert!(output.contains("\"kind\":\"UnrealWorkflowError\""));
    assert!(output.contains("\"status\":\"error\""));
}

#[test]
fn dangerous_actions_are_blocked_by_default() {
    let dev = run_uwf(&[
        "dev",
        "execute",
        "--action",
        "create-task",
        "--confirm",
        "UEWorkflow.DevFlow.create-task.v1",
        "--json",
    ]);
    assert!(dev.contains("\"status\":\"blocked\""));
    assert!(dev.contains("worktree"));

    let agents = run_uwf(&[
        "agents",
        "execute",
        "--action",
        "install-provider",
        "--json",
    ]);
    assert!(agents.contains("\"status\":\"blocked\""));
    assert!(agents.contains("UEWorkflow.AgentHub.install-provider.v1"));

    let kb = run_uwf(&[
        "kb",
        "execute",
        "--action",
        "export",
        "--confirm",
        "UEWorkflow.KnowledgeBase.export.v1",
        "--json",
    ]);
    assert!(kb.contains("\"status\":\"blocked\""));
    assert!(kb.contains("重型"));
}
