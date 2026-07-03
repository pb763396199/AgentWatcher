use crate::plugin_manifest::{
    parse_module_execution_result, validate_agentwatcher_name, AwCommandDescriptor,
    AwCommandOutputKind, AwCommandRunner, AwDiscoveredModule, AwExecutionStatus,
    AwModuleExecutionResult, AwTaskExecutionInfo, AW_SCHEMA_VERSION, RESULT_KIND, TASK_KIND,
};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::Read;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const OUTPUT_LIMIT_BYTES: usize = 128 * 1024;
const PAYLOAD_LIMIT_BYTES: usize = 16 * 1024;
const POLL_INTERVAL_MS: u64 = 50;
const MAX_STORED_RUNS: usize = 256;
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

static RUN_COUNTER: AtomicU64 = AtomicU64::new(1);
static RUNS: OnceLock<Mutex<HashMap<String, StoredRun>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwRunnerError {
    pub message: String,
}

impl AwRunnerError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for AwRunnerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AwRunnerError {}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwModuleCommandRunSnapshot {
    pub run_id: String,
    pub task: AwTaskExecutionInfo,
    pub running: bool,
    pub cancelled: bool,
    pub result: Option<AwModuleExecutionResult>,
}

struct StoredRun {
    task: AwTaskExecutionInfo,
    child: Option<Arc<Mutex<Child>>>,
    cancel_requested: Arc<AtomicBool>,
    result: Option<AwModuleExecutionResult>,
}

struct RunContext {
    run_id: String,
    task_id: String,
    plugin_name: String,
    module_name: String,
    command_name: String,
    started_ms: u64,
    payload_json: Option<String>,
}

pub fn start_module_command_run(
    plugin_name: &str,
    module: &AwDiscoveredModule,
    command: &AwCommandDescriptor,
    task_id: Option<String>,
    payload: Option<Value>,
) -> Result<AwTaskExecutionInfo, AwRunnerError> {
    let external_task_id = match task_id {
        Some(task_id) => {
            let task_id = task_id.trim().to_string();
            validate_agentwatcher_name("taskId", &task_id)
                .map_err(|error| AwRunnerError::new(error.to_string()))?;
            task_id
        }
        None => next_run_id(),
    };
    let run_id = next_run_id();
    let started_ms = current_time_ms();
    let payload_json = serialize_payload(payload)?;
    let context = RunContext {
        run_id: run_id.clone(),
        task_id: external_task_id.clone(),
        plugin_name: plugin_name.to_string(),
        module_name: module.descriptor.name.clone(),
        command_name: command.name.clone(),
        started_ms,
        payload_json,
    };
    let mut process = build_process_command(module, command, &context)?;
    let mut child = process.spawn().map_err(|error| {
        AwRunnerError::new(format!(
            "failed to start command {}: {}",
            command.name, error
        ))
    })?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child = Arc::new(Mutex::new(child));
    let cancel_requested = Arc::new(AtomicBool::new(false));
    let task = AwTaskExecutionInfo {
        schema_version: AW_SCHEMA_VERSION,
        kind: TASK_KIND.to_string(),
        task_id: run_id.clone(),
        plugin_name: plugin_name.to_string(),
        module_name: module.descriptor.name.clone(),
        command_name: command.name.clone(),
        status: AwExecutionStatus::Running,
        started_ms,
        finished_ms: None,
        result_path: None,
        summary: Some("命令运行中".to_string()),
    };

    {
        let mut runs = run_registry()
            .lock()
            .map_err(|_| AwRunnerError::new("module command run registry is unavailable"))?;
        if runs.contains_key(&run_id) {
            return Err(AwRunnerError::new(format!(
                "run id already exists: {run_id}"
            )));
        }
        runs.insert(
            run_id.clone(),
            StoredRun {
                task: task.clone(),
                child: Some(child.clone()),
                cancel_requested: cancel_requested.clone(),
                result: None,
            },
        );
    }

    let command_timeout_ms = command.timeout_ms;
    let command_output_kind = command.produces.clone();
    thread::spawn(move || {
        let stdout_handle = stdout.map(|pipe| thread::spawn(move || read_limited(pipe)));
        let stderr_handle = stderr.map(|pipe| thread::spawn(move || read_limited(pipe)));
        let result = monitor_child(
            child,
            cancel_requested,
            context,
            command_timeout_ms,
            command_output_kind,
            stdout_handle,
            stderr_handle,
        );
        finish_run(result);
    });

    Ok(task)
}

pub fn get_module_command_run(run_id: &str) -> Result<AwModuleCommandRunSnapshot, AwRunnerError> {
    let runs = run_registry()
        .lock()
        .map_err(|_| AwRunnerError::new("module command run registry is unavailable"))?;
    let run = runs
        .get(run_id)
        .ok_or_else(|| AwRunnerError::new(format!("unknown module command run: {run_id}")))?;
    Ok(snapshot_for_run(run_id, run))
}

pub fn cancel_module_command_run(
    run_id: &str,
) -> Result<AwModuleCommandRunSnapshot, AwRunnerError> {
    let mut runs = run_registry()
        .lock()
        .map_err(|_| AwRunnerError::new("module command run registry is unavailable"))?;
    let run = runs
        .get_mut(run_id)
        .ok_or_else(|| AwRunnerError::new(format!("unknown module command run: {run_id}")))?;
    run.cancel_requested.store(true, Ordering::SeqCst);
    if let Some(child) = &run.child {
        if let Ok(mut child) = child.lock() {
            let _ = child.kill();
        }
    }
    Ok(snapshot_for_run(run_id, run))
}

fn build_process_command(
    module: &AwDiscoveredModule,
    command: &AwCommandDescriptor,
    context: &RunContext,
) -> Result<Command, AwRunnerError> {
    let module_root = fs::canonicalize(&module.identity_path).map_err(|error| {
        AwRunnerError::new(format!(
            "failed to resolve module root {}: {}",
            module.identity_path, error
        ))
    })?;
    if !module_root.is_dir() {
        return Err(AwRunnerError::new(format!(
            "module root is not a directory: {}",
            module.identity_path
        )));
    }

    let mut process = match &command.runner {
        AwCommandRunner::NativeExecutable { program, args } => {
            let program_path = resolve_module_file(&module_root, program)?;
            let mut process = Command::new(program_path);
            process.args(render_args(args, context)?);
            process
        }
        AwCommandRunner::PowerShellFile { script, args } => {
            let script_path = resolve_module_file(&module_root, script)?;
            let mut process = Command::new("powershell.exe");
            process.args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ]);
            process.arg(powershell_file_arg(&script_path));
            process.args(render_args(args, context)?);
            process
        }
        AwCommandRunner::NodeScript { script, args } => {
            let script_path = resolve_module_file(&module_root, script)?;
            let mut process = Command::new("node");
            process.arg(script_path);
            process.args(render_args(args, context)?);
            process
        }
        AwCommandRunner::PythonModule { module, args } => {
            let launcher = python_launcher();
            let mut process = Command::new(launcher);
            if cfg!(target_os = "windows") && launcher == "py" {
                process.arg("-3");
            }
            process.args(["-m", module.as_str()]);
            process.args(render_args(args, context)?);
            process
        }
        AwCommandRunner::CargoRun { package, args } => {
            let mut process = Command::new("cargo");
            process.args(["run", "--quiet"]);
            if let Some(package) = package {
                process.args(["--package", package.as_str()]);
            }
            process.arg("--");
            process.args(render_args(args, context)?);
            process
        }
    };

    process.current_dir(module_root);
    process.stdin(Stdio::null());
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());
    apply_restricted_env(&mut process, context);
    #[cfg(target_os = "windows")]
    process.creation_flags(CREATE_NO_WINDOW);
    Ok(process)
}

fn monitor_child(
    child: Arc<Mutex<Child>>,
    cancel_requested: Arc<AtomicBool>,
    context: RunContext,
    timeout_ms: u64,
    output_kind: AwCommandOutputKind,
    stdout_handle: Option<thread::JoinHandle<String>>,
    stderr_handle: Option<thread::JoinHandle<String>>,
) -> FinishedRun {
    let deadline = Duration::from_millis(timeout_ms);
    let started = Instant::now();
    let mut timed_out = false;
    let exit_code = loop {
        let status = {
            let mut child = child.lock().expect("child process lock poisoned");
            child.try_wait()
        };
        match status {
            Ok(Some(status)) => break status.code(),
            Ok(None) => {}
            Err(_) => break None,
        }

        if cancel_requested.load(Ordering::SeqCst) {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
            }
        } else if started.elapsed() >= deadline {
            timed_out = true;
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
            }
        }

        thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    };

    let stdout_excerpt = join_pipe(stdout_handle);
    let stderr_excerpt = join_pipe(stderr_handle);
    let finished_ms = current_time_ms();
    let cancelled = cancel_requested.load(Ordering::SeqCst);
    let status = if cancelled {
        AwExecutionStatus::Cancelled
    } else if timed_out {
        AwExecutionStatus::Blocked
    } else if exit_code == Some(0) {
        AwExecutionStatus::Success
    } else {
        AwExecutionStatus::Failed
    };
    let summary = match status {
        AwExecutionStatus::Success => "命令执行成功".to_string(),
        AwExecutionStatus::Cancelled => "命令已取消".to_string(),
        AwExecutionStatus::Blocked => "命令超时，已终止".to_string(),
        AwExecutionStatus::Failed => format!("命令执行失败，退出码 {:?}", exit_code),
        AwExecutionStatus::Running => "命令运行中".to_string(),
    };
    let output = FinishedModuleOutput {
        exit_code,
        finished_ms,
        stdout_excerpt,
        stderr_excerpt,
    };
    let result = module_result_from_output(&context, &output_kind, status, summary, output);
    let task = AwTaskExecutionInfo {
        schema_version: AW_SCHEMA_VERSION,
        kind: TASK_KIND.to_string(),
        task_id: context.task_id.clone(),
        plugin_name: context.plugin_name.clone(),
        module_name: context.module_name.clone(),
        command_name: context.command_name.clone(),
        status: result.status.clone(),
        started_ms: context.started_ms,
        finished_ms: Some(finished_ms),
        result_path: None,
        summary: Some(result.summary.clone()),
    };

    FinishedRun {
        run_id: context.run_id,
        task,
        cancelled,
        result,
    }
}

fn module_result_from_output(
    context: &RunContext,
    output_kind: &AwCommandOutputKind,
    fallback_status: AwExecutionStatus,
    fallback_summary: String,
    output: FinishedModuleOutput,
) -> AwModuleExecutionResult {
    if matches!(output_kind, AwCommandOutputKind::AgentWatcherModuleResult)
        && !matches!(
            fallback_status,
            AwExecutionStatus::Cancelled | AwExecutionStatus::Blocked
        )
    {
        match parse_module_execution_result_from_stdout(&output.stdout_excerpt) {
            Ok(mut parsed)
                if parsed.module_name == context.module_name
                    && parsed.command_name == context.command_name =>
            {
                if output.exit_code != Some(0) && parsed.status == AwExecutionStatus::Success {
                    parsed.status = AwExecutionStatus::Failed;
                    parsed.summary =
                        format!("命令退出码 {:?}；{}", output.exit_code, parsed.summary);
                }
                parsed.plugin_name = Some(context.plugin_name.clone());
                parsed.task_id = Some(context.task_id.clone());
                parsed.started_at = Some(context.started_ms.to_string());
                parsed.finished_at = Some(output.finished_ms.to_string());
                parsed.exit_code = output.exit_code;
                parsed.stdout_excerpt = non_empty(output.stdout_excerpt);
                parsed.stderr_excerpt = non_empty(output.stderr_excerpt);
                return parsed;
            }
            Ok(parsed) => {
                return synthetic_module_result(
                    context,
                    AwExecutionStatus::Failed,
                    format!(
                        "模块结果身份不匹配：收到 {}/{}",
                        parsed.module_name, parsed.command_name
                    ),
                    output.exit_code,
                    output.finished_ms,
                    output.stdout_excerpt,
                    output.stderr_excerpt,
                );
            }
            Err(error) => {
                return synthetic_module_result(
                    context,
                    AwExecutionStatus::Failed,
                    format!("模块输出不是有效 AgentWatcherModuleResult：{}", error),
                    output.exit_code,
                    output.finished_ms,
                    output.stdout_excerpt,
                    output.stderr_excerpt,
                );
            }
        }
    }

    synthetic_module_result(
        context,
        fallback_status,
        fallback_summary,
        output.exit_code,
        output.finished_ms,
        output.stdout_excerpt,
        output.stderr_excerpt,
    )
}

struct FinishedModuleOutput {
    exit_code: Option<i32>,
    finished_ms: u64,
    stdout_excerpt: String,
    stderr_excerpt: String,
}

fn parse_module_execution_result_from_stdout(
    stdout: &str,
) -> Result<AwModuleExecutionResult, crate::plugin_manifest::AwValidationError> {
    parse_module_execution_result(stdout).or_else(|_| {
        let Some(start) = stdout.find('{') else {
            return parse_module_execution_result(stdout);
        };
        let Some(end) = stdout.rfind('}') else {
            return parse_module_execution_result(stdout);
        };
        if end <= start {
            return parse_module_execution_result(stdout);
        }
        parse_module_execution_result(&stdout[start..=end])
    })
}

fn synthetic_module_result(
    context: &RunContext,
    status: AwExecutionStatus,
    summary: String,
    exit_code: Option<i32>,
    finished_ms: u64,
    stdout_excerpt: String,
    stderr_excerpt: String,
) -> AwModuleExecutionResult {
    AwModuleExecutionResult {
        schema_version: AW_SCHEMA_VERSION,
        kind: RESULT_KIND.to_string(),
        plugin_name: Some(context.plugin_name.clone()),
        module_name: context.module_name.clone(),
        command_name: context.command_name.clone(),
        status,
        summary,
        task_id: Some(context.task_id.clone()),
        started_at: Some(context.started_ms.to_string()),
        finished_at: Some(finished_ms.to_string()),
        exit_code,
        artifacts: Vec::new(),
        metrics: Default::default(),
        details: None,
        stdout_excerpt: non_empty(stdout_excerpt),
        stderr_excerpt: non_empty(stderr_excerpt),
    }
}

fn finish_run(finished: FinishedRun) {
    if let Ok(mut runs) = run_registry().lock() {
        prune_completed_runs(&mut runs);
        runs.insert(
            finished.run_id,
            StoredRun {
                task: finished.task,
                child: None,
                cancel_requested: Arc::new(AtomicBool::new(finished.cancelled)),
                result: Some(finished.result),
            },
        );
    }
}

fn prune_completed_runs(runs: &mut HashMap<String, StoredRun>) {
    if runs.len() < MAX_STORED_RUNS {
        return;
    }
    let mut completed = runs
        .iter()
        .filter(|(_, run)| run.child.is_none())
        .map(|(run_id, run)| {
            (
                run_id.clone(),
                run.task
                    .finished_ms
                    .or(Some(run.task.started_ms))
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    completed.sort_by_key(|(_, finished_ms)| *finished_ms);
    let remove_count = runs.len().saturating_sub(MAX_STORED_RUNS - 1);
    for (run_id, _) in completed.into_iter().take(remove_count) {
        runs.remove(&run_id);
    }
}

struct FinishedRun {
    run_id: String,
    task: AwTaskExecutionInfo,
    cancelled: bool,
    result: AwModuleExecutionResult,
}

fn snapshot_for_run(run_id: &str, run: &StoredRun) -> AwModuleCommandRunSnapshot {
    AwModuleCommandRunSnapshot {
        run_id: run_id.to_string(),
        task: run.task.clone(),
        running: run.child.is_some(),
        cancelled: run.cancel_requested.load(Ordering::SeqCst),
        result: run.result.clone(),
    }
}

fn resolve_module_file(module_root: &Path, relative_path: &str) -> Result<PathBuf, AwRunnerError> {
    if !is_safe_relative_path(relative_path) {
        return Err(AwRunnerError::new(format!(
            "unsafe command path outside module root: {relative_path}"
        )));
    }

    let mut candidate = module_root.to_path_buf();
    for segment in relative_path.replace('\\', "/").split('/') {
        candidate.push(segment);
    }
    let candidate = fs::canonicalize(&candidate).map_err(|error| {
        AwRunnerError::new(format!(
            "failed to resolve command path {}: {}",
            candidate.to_string_lossy(),
            error
        ))
    })?;
    if !candidate.starts_with(module_root) {
        return Err(AwRunnerError::new(format!(
            "command path escapes module root: {}",
            candidate.to_string_lossy()
        )));
    }
    if !candidate.is_file() {
        return Err(AwRunnerError::new(format!(
            "command path is not a file: {}",
            candidate.to_string_lossy()
        )));
    }
    Ok(candidate)
}

#[cfg(windows)]
fn powershell_file_arg(path: &Path) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{}", rest));
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path.to_path_buf()
}

#[cfg(not(windows))]
fn powershell_file_arg(path: &Path) -> PathBuf {
    path.to_path_buf()
}

fn render_args(args: &[String], context: &RunContext) -> Result<Vec<String>, AwRunnerError> {
    args.iter()
        .map(|arg| render_arg(arg, context))
        .collect::<Result<Vec<_>, _>>()
}

fn render_arg(arg: &str, context: &RunContext) -> Result<String, AwRunnerError> {
    let rendered = arg
        .replace("{{runId}}", &context.run_id)
        .replace("{{taskId}}", &context.task_id)
        .replace("{{pluginName}}", &context.plugin_name)
        .replace("{{moduleName}}", &context.module_name)
        .replace("{{commandName}}", &context.command_name);
    if rendered.contains("{{") || rendered.contains("}}") {
        return Err(AwRunnerError::new(format!(
            "unsupported command argument template: {arg}"
        )));
    }
    Ok(rendered)
}

fn apply_restricted_env(process: &mut Command, context: &RunContext) {
    process.env_clear();
    for key in ["PATH", "PATHEXT", "SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(key) {
            process.env(key, value);
        }
    }
    process.env("AW_RUN_ID", &context.run_id);
    process.env("AW_TASK_ID", &context.task_id);
    process.env("AW_PLUGIN_NAME", &context.plugin_name);
    process.env("AW_MODULE_NAME", &context.module_name);
    process.env("AW_COMMAND_NAME", &context.command_name);
    if let Some(payload_json) = &context.payload_json {
        process.env("AW_COMMAND_PAYLOAD_JSON", payload_json);
    }
}

fn serialize_payload(payload: Option<Value>) -> Result<Option<String>, AwRunnerError> {
    let Some(payload) = payload else {
        return Ok(None);
    };
    let text = serde_json::to_string(&payload).map_err(|error| {
        AwRunnerError::new(format!("failed to serialize command payload: {error}"))
    })?;
    if text.len() > PAYLOAD_LIMIT_BYTES {
        return Err(AwRunnerError::new(format!(
            "command payload is too large: {} bytes",
            text.len()
        )));
    }
    Ok(Some(text))
}

fn read_limited(mut reader: impl Read) -> String {
    let mut output = Vec::with_capacity(OUTPUT_LIMIT_BYTES.min(8 * 1024));
    let mut buffer = [0_u8; 8192];
    while let Ok(read) = reader.read(&mut buffer) {
        if read == 0 {
            break;
        }
        let remaining = OUTPUT_LIMIT_BYTES.saturating_sub(output.len());
        if remaining > 0 {
            output.extend_from_slice(&buffer[..read.min(remaining)]);
        }
    }
    String::from_utf8_lossy(&output).to_string()
}

fn join_pipe(handle: Option<thread::JoinHandle<String>>) -> String {
    handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default()
}

fn non_empty(text: String) -> Option<String> {
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn next_run_id() -> String {
    format!(
        "module-run-{}-{}",
        current_time_ms(),
        RUN_COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn run_registry() -> &'static Mutex<HashMap<String, StoredRun>> {
    RUNS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn python_launcher() -> &'static str {
    if cfg!(target_os = "windows") && program_exists_on_path("py.exe") {
        "py"
    } else {
        "python"
    }
}

fn program_exists_on_path(program_name: &str) -> bool {
    let Some(path_value) = std::env::var_os("PATH") else {
        return false;
    };

    std::env::split_paths(&path_value).any(|path| path.join(program_name).is_file())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin_manifest::{
        AwCommandOutputKind, AwCommandSafety, AwModuleDescriptor, AwModuleSource, AwModuleType,
    };

    #[cfg(windows)]
    #[test]
    fn powershell_file_arg_strips_windows_extended_prefix() {
        assert_eq!(
            powershell_file_arg(Path::new(r"\\?\X:\Workspace\Module\aw\Probe.ps1")),
            PathBuf::from(r"X:\Workspace\Module\aw\Probe.ps1")
        );
        assert_eq!(
            powershell_file_arg(Path::new(r"\\?\UNC\server\share\Probe.ps1")),
            PathBuf::from(r"\\server\share\Probe.ps1")
        );
    }

    #[test]
    fn native_runner_uses_declared_executable_and_captures_result() {
        let temp = temp_run_root("native");
        let module = test_module(
            &temp,
            native_probe_command(vec![
                "module_runner::tests::module_runner_json_probe".to_string(),
                "--exact".to_string(),
                "--ignored".to_string(),
                "--nocapture".to_string(),
            ]),
        );

        let task = start_module_command_run(
            "UEWorkflow",
            &module,
            &module.descriptor.commands[0],
            None,
            None,
        )
        .unwrap();
        let snapshot = wait_for_run(&task.task_id);

        assert_eq!(task.status, AwExecutionStatus::Running);
        assert!(!snapshot.running);
        assert_eq!(
            snapshot.result.as_ref().unwrap().status,
            AwExecutionStatus::Success
        );
        assert_eq!(snapshot.result.as_ref().unwrap().summary, "probe status ok");
        assert_eq!(snapshot.result.as_ref().unwrap().exit_code, Some(0));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runner_rejects_invalid_external_task_id() {
        let temp = temp_run_root("bad-task-id");
        let module = test_module(&temp, native_probe_command(vec!["--help".to_string()]));

        let error = start_module_command_run(
            "UEWorkflow",
            &module,
            &module.descriptor.commands[0],
            Some("bad task id".to_string()),
            None,
        )
        .unwrap_err();

        assert!(error.message.contains("taskId must start"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runner_allows_repeated_runs_for_same_external_task_id() {
        let temp = temp_run_root("repeat");
        let module = test_module(
            &temp,
            native_probe_command(vec![
                "module_runner::tests::module_runner_json_probe".to_string(),
                "--exact".to_string(),
                "--ignored".to_string(),
                "--nocapture".to_string(),
            ]),
        );

        let first = start_module_command_run(
            "UEWorkflow",
            &module,
            &module.descriptor.commands[0],
            Some("todo-repeat-1".to_string()),
            None,
        )
        .unwrap();
        let second = start_module_command_run(
            "UEWorkflow",
            &module,
            &module.descriptor.commands[0],
            Some("todo-repeat-1".to_string()),
            None,
        )
        .unwrap();
        let first_snapshot = wait_for_run(&first.task_id);
        let second_snapshot = wait_for_run(&second.task_id);

        assert_ne!(first.task_id, second.task_id);
        assert_eq!(
            first_snapshot.result.as_ref().unwrap().task_id.as_deref(),
            Some("todo-repeat-1")
        );
        assert_eq!(
            second_snapshot.result.as_ref().unwrap().task_id.as_deref(),
            Some("todo-repeat-1")
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runner_marks_malformed_module_output_as_failed_result() {
        let temp = temp_run_root("malformed-output");
        let module = test_module(&temp, native_probe_command(vec!["--help".to_string()]));

        let task = start_module_command_run(
            "UEWorkflow",
            &module,
            &module.descriptor.commands[0],
            None,
            None,
        )
        .unwrap();
        let snapshot = wait_for_run(&task.task_id);

        assert_eq!(
            snapshot.result.as_ref().unwrap().status,
            AwExecutionStatus::Failed
        );
        assert!(snapshot
            .result
            .as_ref()
            .unwrap()
            .summary
            .contains("模块输出不是有效 AgentWatcherModuleResult"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runner_marks_mismatched_module_output_as_failed_result() {
        let temp = temp_run_root("mismatch-output");
        let module = test_module(
            &temp,
            native_probe_command(vec![
                "module_runner::tests::module_runner_mismatch_json_probe".to_string(),
                "--exact".to_string(),
                "--ignored".to_string(),
                "--nocapture".to_string(),
            ]),
        );

        let task = start_module_command_run(
            "UEWorkflow",
            &module,
            &module.descriptor.commands[0],
            None,
            None,
        )
        .unwrap();
        let snapshot = wait_for_run(&task.task_id);

        assert_eq!(
            snapshot.result.as_ref().unwrap().status,
            AwExecutionStatus::Failed
        );
        assert!(snapshot
            .result
            .as_ref()
            .unwrap()
            .summary
            .contains("模块结果身份不匹配"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runner_rejects_executable_outside_module_root() {
        let temp = temp_run_root("unsafe");
        let mut command = native_probe_command(vec!["--help".to_string()]);
        command.runner = AwCommandRunner::NativeExecutable {
            program: "../outside.exe".to_string(),
            args: Vec::new(),
        };
        let module = test_module(&temp, command);

        let error = start_module_command_run(
            "UEWorkflow",
            &module,
            &module.descriptor.commands[0],
            None,
            None,
        )
        .unwrap_err();

        assert!(error.message.contains("unsafe command path"));
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runner_supports_cancelling_running_command() {
        let temp = temp_run_root("cancel");
        let module = test_module(
            &temp,
            native_probe_command(vec![
                "module_runner::tests::module_runner_sleep_probe".to_string(),
                "--exact".to_string(),
                "--ignored".to_string(),
                "--nocapture".to_string(),
            ]),
        );

        let task = start_module_command_run(
            "UEWorkflow",
            &module,
            &module.descriptor.commands[0],
            None,
            None,
        )
        .unwrap();
        thread::sleep(Duration::from_millis(200));
        let cancelling = cancel_module_command_run(&task.task_id).unwrap();
        let snapshot = wait_for_run(&task.task_id);

        assert!(cancelling.cancelled);
        assert!(!snapshot.running);
        assert_eq!(
            snapshot.result.as_ref().unwrap().status,
            AwExecutionStatus::Cancelled
        );
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn runner_passes_payload_as_restricted_env() {
        let temp = temp_run_root("payload");
        let module = test_module(
            &temp,
            native_probe_command(vec![
                "module_runner::tests::module_runner_payload_probe".to_string(),
                "--exact".to_string(),
                "--ignored".to_string(),
                "--nocapture".to_string(),
            ]),
        );

        let task = start_module_command_run(
            "UEWorkflow",
            &module,
            &module.descriptor.commands[0],
            None,
            Some(serde_json::json!({
                "action": "createTask",
                "confirmed": true,
                "dryRunAccepted": true
            })),
        )
        .unwrap();
        let snapshot = wait_for_run(&task.task_id);
        let result = snapshot.result.as_ref().unwrap();
        let details = result.details.as_ref().unwrap();

        assert_eq!(result.status, AwExecutionStatus::Success);
        assert_eq!(details["payloadAction"], "createTask");
        assert_eq!(details["payloadConfirmed"], true);
        let _ = fs::remove_dir_all(temp);
    }

    #[test]
    fn completed_run_registry_prunes_oldest_entries() {
        let mut runs = HashMap::new();
        for index in 0..=MAX_STORED_RUNS {
            let run_id = format!("run-{index:03}");
            runs.insert(
                run_id.clone(),
                StoredRun {
                    task: AwTaskExecutionInfo {
                        schema_version: AW_SCHEMA_VERSION,
                        kind: TASK_KIND.to_string(),
                        task_id: run_id,
                        plugin_name: "UEWorkflow".to_string(),
                        module_name: "DevFlow".to_string(),
                        command_name: "status".to_string(),
                        status: AwExecutionStatus::Success,
                        started_ms: index as u64,
                        finished_ms: Some(index as u64),
                        result_path: None,
                        summary: Some("done".to_string()),
                    },
                    child: None,
                    cancel_requested: Arc::new(AtomicBool::new(false)),
                    result: None,
                },
            );
        }

        prune_completed_runs(&mut runs);

        assert!(runs.len() < MAX_STORED_RUNS);
        assert!(!runs.contains_key("run-000"));
        assert!(!runs.contains_key("run-001"));
        assert!(runs.contains_key(&format!("run-{MAX_STORED_RUNS:03}")));
    }

    #[test]
    #[ignore]
    fn module_runner_sleep_probe() {
        thread::sleep(Duration::from_secs(30));
    }

    #[test]
    #[ignore]
    fn module_runner_json_probe() {
        println!(
            r#"{{
  "schemaVersion": 1,
  "kind": "AgentWatcherModuleResult",
  "moduleName": "DevFlow",
  "commandName": "probe",
  "status": "success",
  "summary": "probe status ok",
  "exitCode": 0,
  "details": {{ "probe": true }}
}}"#
        );
    }

    #[test]
    #[ignore]
    fn module_runner_mismatch_json_probe() {
        println!(
            r#"{{
  "schemaVersion": 1,
  "kind": "AgentWatcherModuleResult",
  "moduleName": "WrongModule",
  "commandName": "probe",
  "status": "success",
  "summary": "wrong module",
  "exitCode": 0
}}"#
        );
    }

    #[test]
    #[ignore]
    fn module_runner_payload_probe() {
        let payload_text = std::env::var("AW_COMMAND_PAYLOAD_JSON").unwrap_or_default();
        let payload: Value = serde_json::from_str(&payload_text).unwrap();
        println!(
            r#"{{
  "schemaVersion": 1,
  "kind": "AgentWatcherModuleResult",
  "moduleName": "DevFlow",
  "commandName": "probe",
  "status": "success",
  "summary": "probe payload ok",
  "exitCode": 0,
  "details": {{
    "payloadAction": "{}",
    "payloadConfirmed": {}
  }}
}}"#,
            payload["action"].as_str().unwrap_or(""),
            payload["confirmed"].as_bool().unwrap_or(false)
        );
    }

    fn test_module(root: &Path, command: AwCommandDescriptor) -> AwDiscoveredModule {
        let bin_dir = root.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let probe_name = if cfg!(target_os = "windows") {
            "probe.exe"
        } else {
            "probe"
        };
        fs::copy(std::env::current_exe().unwrap(), bin_dir.join(probe_name)).unwrap();
        let root_path = fs::canonicalize(root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        AwDiscoveredModule {
            descriptor: AwModuleDescriptor {
                schema_version: AW_SCHEMA_VERSION,
                kind: crate::plugin_manifest::MODULE_KIND.to_string(),
                name: "DevFlow".to_string(),
                display_name: "开发流".to_string(),
                module_type: AwModuleType::Workflow,
                source: AwModuleSource {
                    project_path: ".".to_string(),
                },
                description: None,
                commands: vec![command],
            },
            manifest_path: root
                .join("AgentWatcher.awmodule.json")
                .to_string_lossy()
                .to_string(),
            root_path: root_path.clone(),
            identity_path: root_path.clone(),
            display_path: root_path,
        }
    }

    fn native_probe_command(args: Vec<String>) -> AwCommandDescriptor {
        let probe_name = if cfg!(target_os = "windows") {
            "bin/probe.exe"
        } else {
            "bin/probe"
        };
        AwCommandDescriptor {
            name: "probe".to_string(),
            display_name: "Probe".to_string(),
            safety: AwCommandSafety::ReadOnly,
            runner: AwCommandRunner::NativeExecutable {
                program: probe_name.to_string(),
                args,
            },
            description: None,
            timeout_ms: 5_000,
            produces: AwCommandOutputKind::AgentWatcherModuleResult,
            safety_contract: None,
        }
    }

    fn wait_for_run(run_id: &str) -> AwModuleCommandRunSnapshot {
        let start = Instant::now();
        loop {
            let snapshot = get_module_command_run(run_id).unwrap();
            if !snapshot.running {
                return snapshot;
            }
            assert!(start.elapsed() < Duration::from_secs(10));
            thread::sleep(Duration::from_millis(50));
        }
    }

    fn temp_run_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "agentwatcher-module-runner-{label}-{}",
            current_time_ms()
        ))
    }
}
