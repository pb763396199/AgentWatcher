use crate::plugin_manifest::AW_SCHEMA_VERSION;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const UEWORKFLOW_EXECUTION_KIND: &str = "AgentWatcherUEWorkflowExecution";
pub const UEWORKFLOW_KNOWLEDGE_KIND: &str = "AgentWatcherUEWorkflowKnowledge";
const UEWORKFLOW_GOAL_MAX_CHARS: usize = 2000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwUeWorkflowExecutionRequest {
    pub goal: String,
    #[serde(default)]
    pub dry_run: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AwUeWorkflowKnowledgeRequest {
    pub goal: String,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub dry_run: Value,
    #[serde(default)]
    pub execution: Value,
    #[serde(default)]
    pub review: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AwUeWorkflowExecutionRecord {
    pub schema_version: u32,
    pub kind: String,
    pub execution_id: String,
    pub status: String,
    pub goal: String,
    pub created_ms: u64,
    pub artifacts: Vec<AwUeWorkflowExecutionArtifact>,
    pub confirmation_points: Vec<String>,
    pub safety: AwUeWorkflowExecutionSafety,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AwUeWorkflowKnowledgeRecord {
    pub schema_version: u32,
    pub kind: String,
    pub knowledge_id: String,
    pub dedupe_key: String,
    pub goal: String,
    pub task_id: Option<String>,
    pub status: String,
    pub summary: String,
    pub created_ms: u64,
    pub artifacts: Vec<AwUeWorkflowExecutionArtifact>,
    pub requirements: Vec<String>,
    pub context: Vec<String>,
    pub plan: Vec<String>,
    pub diagnostics: Vec<String>,
    pub review: Vec<String>,
    pub safety: AwUeWorkflowExecutionSafety,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AwUeWorkflowExecutionArtifact {
    pub label: String,
    pub artifact_type: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AwUeWorkflowExecutionSafety {
    pub arbitrary_shell_allowed: bool,
    pub raw_ue_build_allowed: bool,
    pub external_worktree_created: bool,
    pub destructive_actions_performed: bool,
}

pub fn execution_root_from_appdata(appdata: &Path) -> PathBuf {
    appdata
        .join("AgentWatcher")
        .join("ueworkflow")
        .join("executions")
}

pub fn knowledge_root_from_appdata(appdata: &Path) -> PathBuf {
    appdata
        .join("AgentWatcher")
        .join("ueworkflow")
        .join("knowledge")
}

pub fn commit_dry_run_execution(
    root: &Path,
    request: AwUeWorkflowExecutionRequest,
) -> Result<AwUeWorkflowExecutionRecord, String> {
    let goal = clean_goal(&request.goal)?;
    let created_ms = now_ms();
    let execution_id = format!("ueexec-{created_ms}");
    let execution_dir = root.join(&execution_id);
    fs::create_dir_all(&execution_dir)
        .map_err(|error| format!("Failed to create UEWorkflow execution directory: {error}"))?;

    let prompt_path = execution_dir.join("agent-prompt.md");
    let task_path = execution_dir.join("task-draft.md");
    let record_path = execution_dir.join("execution.json");

    fs::write(&prompt_path, render_agent_prompt(&goal, &request.dry_run))
        .map_err(|error| format!("Failed to write UEWorkflow agent prompt: {error}"))?;
    fs::write(&task_path, render_task_draft(&goal, &request.dry_run))
        .map_err(|error| format!("Failed to write UEWorkflow task draft: {error}"))?;

    let record = AwUeWorkflowExecutionRecord {
        schema_version: AW_SCHEMA_VERSION,
        kind: UEWORKFLOW_EXECUTION_KIND.to_string(),
        execution_id,
        status: "recorded".to_string(),
        goal,
        created_ms,
        artifacts: vec![
            artifact("智能体提示词", "markdown", &prompt_path),
            artifact("开发流任务草案", "markdown", &task_path),
            artifact("执行记录", "json", &record_path),
        ],
        confirmation_points: vec![
            "确认 UE 目标和验收标准".to_string(),
            "确认知识上下文".to_string(),
            "确认智能体计划".to_string(),
            "确认开发流任务草案".to_string(),
            "确认后续 worktree 创建边界".to_string(),
        ],
        safety: AwUeWorkflowExecutionSafety {
            arbitrary_shell_allowed: false,
            raw_ue_build_allowed: false,
            external_worktree_created: false,
            destructive_actions_performed: false,
        },
    };

    let record_text = serde_json::to_string_pretty(&json!({
        "record": record,
        "dryRun": request.dry_run
    }))
    .map_err(|error| format!("Failed to serialize UEWorkflow execution record: {error}"))?;
    fs::write(&record_path, record_text)
        .map_err(|error| format!("Failed to write UEWorkflow execution record: {error}"))?;

    Ok(record)
}

pub fn commit_external_command_execution(
    root: &Path,
    command_id: &str,
    goal: &str,
    result: Value,
) -> Result<AwUeWorkflowExecutionRecord, String> {
    let command_id = clean_command_id(command_id)?;
    let goal = clean_goal(goal)?;
    let created_ms = now_ms();
    let execution_id = format!("ueexec-{created_ms}");
    let execution_dir = root.join(&execution_id);
    fs::create_dir_all(&execution_dir).map_err(|error| {
        format!("Failed to create UEWorkflow command execution directory: {error}")
    })?;

    let result_path = execution_dir.join("command-result.json");
    let record_path = execution_dir.join("execution.json");
    let record = AwUeWorkflowExecutionRecord {
        schema_version: AW_SCHEMA_VERSION,
        kind: UEWORKFLOW_EXECUTION_KIND.to_string(),
        execution_id,
        status: "complete".to_string(),
        goal,
        created_ms,
        artifacts: vec![
            artifact("命令结果", "json", &result_path),
            artifact("执行记录", "json", &record_path),
        ],
        confirmation_points: vec![
            "已通过 AgentWatcher uwf 命令白名单".to_string(),
            "已校验命令确认边界".to_string(),
            format!("命令：{command_id}"),
        ],
        safety: AwUeWorkflowExecutionSafety {
            arbitrary_shell_allowed: false,
            raw_ue_build_allowed: false,
            external_worktree_created: false,
            destructive_actions_performed: false,
        },
    };

    let result_text = serde_json::to_string_pretty(&json!({
        "commandId": command_id,
        "result": result
    }))
    .map_err(|error| format!("Failed to serialize UEWorkflow command result: {error}"))?;
    fs::write(&result_path, result_text)
        .map_err(|error| format!("Failed to write UEWorkflow command result: {error}"))?;

    let record_text = serde_json::to_string_pretty(&json!({
        "record": record,
        "commandId": command_id
    }))
    .map_err(|error| format!("Failed to serialize UEWorkflow execution record: {error}"))?;
    fs::write(&record_path, record_text)
        .map_err(|error| format!("Failed to write UEWorkflow execution record: {error}"))?;

    Ok(record)
}

pub fn commit_knowledge_record(
    root: &Path,
    request: AwUeWorkflowKnowledgeRequest,
) -> Result<AwUeWorkflowKnowledgeRecord, String> {
    let goal = clean_goal(&request.goal)?;
    let task_id = clean_optional_id(request.task_id.as_deref())?;
    let status = clean_status(request.status.as_deref());
    let dedupe_key = stable_hash(&format!(
        "{}\n{}\n{}",
        goal,
        task_id.as_deref().unwrap_or(""),
        status
    ));

    fs::create_dir_all(root.join("records"))
        .map_err(|error| format!("Failed to create UEWorkflow knowledge directory: {error}"))?;
    let index_path = root.join("index.jsonl");
    if let Some(existing) = find_knowledge_by_dedupe(&index_path, &dedupe_key)? {
        return Ok(existing);
    }

    let created_ms = now_ms();
    let id_suffix = dedupe_key.chars().take(8).collect::<String>();
    let knowledge_id = task_id
        .as_deref()
        .map(|id| format!("uekb-{id}-{id_suffix}"))
        .unwrap_or_else(|| format!("uekb-{created_ms}-{id_suffix}"));
    let record_path = root.join("records").join(format!("{knowledge_id}.json"));
    let markdown_path = root.join("records").join(format!("{knowledge_id}.md"));
    let dry_run_plan = extract_dry_run_plan(&request.dry_run);
    let diagnostics = extract_diagnostics(&request.execution);
    let review = extract_review(&request.review);

    let record = AwUeWorkflowKnowledgeRecord {
        schema_version: AW_SCHEMA_VERSION,
        kind: UEWORKFLOW_KNOWLEDGE_KIND.to_string(),
        knowledge_id,
        dedupe_key,
        goal: goal.clone(),
        task_id,
        status,
        summary: format!("UEWorkflow 知识沉淀：{goal}"),
        created_ms,
        artifacts: vec![
            artifact("知识记录", "json", &record_path),
            artifact("知识摘要", "markdown", &markdown_path),
            artifact("知识索引", "jsonl", &index_path),
        ],
        requirements: vec![goal],
        context: extract_stage_names(&request.dry_run),
        plan: dry_run_plan,
        diagnostics,
        review,
        safety: AwUeWorkflowExecutionSafety {
            arbitrary_shell_allowed: false,
            raw_ue_build_allowed: false,
            external_worktree_created: false,
            destructive_actions_performed: false,
        },
    };

    let record_text = serde_json::to_string_pretty(&json!({
        "record": record,
        "dryRun": request.dry_run,
        "execution": request.execution,
        "review": request.review
    }))
    .map_err(|error| format!("Failed to serialize UEWorkflow knowledge record: {error}"))?;
    fs::write(&record_path, record_text)
        .map_err(|error| format!("Failed to write UEWorkflow knowledge record: {error}"))?;
    fs::write(&markdown_path, render_knowledge_markdown(&record))
        .map_err(|error| format!("Failed to write UEWorkflow knowledge summary: {error}"))?;

    let line = serde_json::to_string(&record)
        .map_err(|error| format!("Failed to serialize UEWorkflow knowledge index: {error}"))?;
    let mut index = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&index_path)
        .map_err(|error| format!("Failed to open UEWorkflow knowledge index: {error}"))?;
    writeln!(index, "{line}")
        .map_err(|error| format!("Failed to append UEWorkflow knowledge index: {error}"))?;

    Ok(record)
}

fn render_agent_prompt(goal: &str, dry_run: &Value) -> String {
    let stages = dry_run
        .get("stages")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("displayName").and_then(Value::as_str))
                .map(|name| format!("- {name}"))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "- 知识库\n- 智能体\n- 开发流".to_string());

    format!(
        "# UEWorkflow 智能体提示词\n\n目标：{goal}\n\n预演链路：\n{stages}\n\n安全边界：不执行任意 shell，不运行 raw UE build，不创建外部 worktree。\n"
    )
}

fn render_task_draft(goal: &str, _dry_run: &Value) -> String {
    format!(
        "# UEWorkflow 开发流任务草案\n\n目标：{goal}\n\n状态：已生成受控执行包，等待后续 worktree 创建确认。\n\n验收：先完成 dry-run 审核，再进入声明过的 bounded execute。\n"
    )
}

fn render_knowledge_markdown(record: &AwUeWorkflowKnowledgeRecord) -> String {
    format!(
        "# UEWorkflow 知识沉淀\n\n需求：{}\n\n任务：{}\n\n状态：{}\n\n上下文：\n{}\n\n计划：\n{}\n\n失败诊断：\n{}\n\n评审结论：\n{}\n\n安全边界：不执行任意 shell，不运行 raw UE build，不执行破坏性删除。\n",
        record.goal,
        record.task_id.as_deref().unwrap_or("未绑定"),
        record.status,
        markdown_list(&record.context),
        markdown_list(&record.plan),
        markdown_list(&record.diagnostics),
        markdown_list(&record.review)
    )
}

fn markdown_list(items: &[String]) -> String {
    if items.is_empty() {
        return "- 暂无".to_string();
    }
    items
        .iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn artifact(label: &str, artifact_type: &str, path: &Path) -> AwUeWorkflowExecutionArtifact {
    AwUeWorkflowExecutionArtifact {
        label: label.to_string(),
        artifact_type: artifact_type.to_string(),
        path: path.to_string_lossy().to_string(),
    }
}

fn clean_goal(goal: &str) -> Result<String, String> {
    let trimmed = sanitize_public_text(goal.trim());
    if trimmed.is_empty() {
        return Err("UEWorkflow execution goal must not be empty".to_string());
    }
    if trimmed.chars().count() > UEWORKFLOW_GOAL_MAX_CHARS {
        return Err("UEWorkflow execution goal is too long".to_string());
    }
    Ok(trimmed)
}

fn clean_optional_id(task_id: Option<&str>) -> Result<Option<String>, String> {
    let Some(task_id) = task_id else {
        return Ok(None);
    };
    let trimmed = task_id.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > 96
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("UEWorkflow knowledge task id is invalid".to_string());
    }
    Ok(Some(trimmed.to_string()))
}

fn clean_status(status: Option<&str>) -> String {
    let cleaned = status
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(sanitize_public_text)
        .unwrap_or_else(|| "recorded".to_string());
    cleaned.chars().take(64).collect()
}

fn clean_command_id(command_id: &str) -> Result<String, String> {
    let trimmed = command_id.trim();
    if trimmed.is_empty()
        || trimmed.len() > 96
        || !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '.')
    {
        return Err("UEWorkflow command id is invalid".to_string());
    }
    Ok(trimmed.to_string())
}

fn sanitize_public_text(text: &str) -> String {
    text.replace(&legacy_source_name(&["Unreal", "DevFlow"], ""), "开发流")
        .replace(
            &legacy_source_name(&["UE", "Master", "Agent"], "_"),
            "智能体",
        )
        .replace(
            &legacy_source_name(&["UE5", "KnowledgeBaseMaker"], "_"),
            "知识库",
        )
        .replace("F:\\", "[路径]\\")
}

fn legacy_source_name(parts: &[&str], separator: &str) -> String {
    parts.join(separator)
}

fn extract_stage_names(dry_run: &Value) -> Vec<String> {
    dry_run
        .get("stages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|stage| stage.get("displayName").and_then(Value::as_str))
        .map(sanitize_public_text)
        .filter(|value| !value.trim().is_empty())
        .collect()
}

fn extract_dry_run_plan(dry_run: &Value) -> Vec<String> {
    dry_run
        .get("stages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|stage| {
            stage
                .get("plan")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|step| step.get("label").and_then(Value::as_str))
                .map(sanitize_public_text)
                .collect::<Vec<_>>()
        })
        .filter(|value| !value.trim().is_empty())
        .collect()
}

fn extract_diagnostics(execution: &Value) -> Vec<String> {
    let status = execution
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let reason = execution
        .get("reason")
        .and_then(Value::as_str)
        .map(sanitize_public_text)
        .unwrap_or_default();
    if reason.is_empty() {
        vec![format!("执行状态：{}", sanitize_public_text(status))]
    } else {
        vec![format!(
            "执行状态：{}；原因：{}",
            sanitize_public_text(status),
            reason
        )]
    }
}

fn extract_review(review: &Value) -> Vec<String> {
    review
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(sanitize_public_text)
        .filter(|value| !value.trim().is_empty())
        .collect::<Vec<_>>()
}

fn find_knowledge_by_dedupe(
    index_path: &Path,
    dedupe_key: &str,
) -> Result<Option<AwUeWorkflowKnowledgeRecord>, String> {
    if !index_path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(index_path)
        .map_err(|error| format!("Failed to read UEWorkflow knowledge index: {error}"))?;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(record) = serde_json::from_str::<AwUeWorkflowKnowledgeRecord>(trimmed) else {
            continue;
        };
        if record.dedupe_key == dedupe_key {
            return Ok(Some(record));
        }
    }
    Ok(None)
}

fn stable_hash(text: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn commit_dry_run_execution_writes_safe_artifacts() {
        let root = temp_root("write");
        let record = commit_dry_run_execution(
            &root,
            AwUeWorkflowExecutionRequest {
                goal: "修复材质参数同步".to_string(),
                dry_run: json!({
                    "stages": [
                        { "displayName": "知识库" },
                        { "displayName": "智能体" },
                        { "displayName": "开发流" }
                    ]
                }),
            },
        )
        .unwrap();

        assert_eq!(record.kind, UEWORKFLOW_EXECUTION_KIND);
        assert_eq!(record.status, "recorded");
        assert_eq!(record.artifacts.len(), 3);
        assert!(!record.safety.arbitrary_shell_allowed);
        assert!(!record.safety.raw_ue_build_allowed);
        assert!(!record.safety.external_worktree_created);
        for artifact in &record.artifacts {
            assert!(Path::new(&artifact.path).exists());
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn commit_dry_run_execution_rejects_empty_goal() {
        let root = temp_root("empty");
        let error = commit_dry_run_execution(
            &root,
            AwUeWorkflowExecutionRequest {
                goal: " ".to_string(),
                dry_run: Value::Null,
            },
        )
        .unwrap_err();

        assert!(error.contains("must not be empty"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn commit_dry_run_execution_accepts_full_task_goal() {
        let root = temp_root("long-goal");
        let goal = "优化AesEarth建筑生成分层逻辑。".repeat(35);
        assert!(goal.chars().count() > 240);
        let record = commit_dry_run_execution(
            &root,
            AwUeWorkflowExecutionRequest {
                goal: goal.clone(),
                dry_run: Value::Null,
            },
        )
        .unwrap();

        assert_eq!(record.goal, goal);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn commit_dry_run_execution_rejects_oversized_goal() {
        let root = temp_root("oversized-goal");
        let error = commit_dry_run_execution(
            &root,
            AwUeWorkflowExecutionRequest {
                goal: "超".repeat(UEWORKFLOW_GOAL_MAX_CHARS + 1),
                dry_run: Value::Null,
            },
        )
        .unwrap_err();

        assert!(error.contains("too long"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn commit_knowledge_record_writes_index_and_dedupes() {
        let root = temp_root("knowledge");
        let request = AwUeWorkflowKnowledgeRequest {
            goal: "修复材质参数同步".to_string(),
            task_id: Some("aw-kb-smoke".to_string()),
            status: Some("success".to_string()),
            dry_run: json!({
                "stages": [
                    { "displayName": "知识库", "plan": [{ "label": "读取项目规则" }] },
                    { "displayName": "智能体", "plan": [{ "label": "生成计划" }] },
                    { "displayName": "开发流", "plan": [{ "label": "创建任务草案" }] }
                ]
            }),
            execution: json!({
                "status": "success",
                "externalWorktreeCreated": true
            }),
            review: json!({
                "items": ["dry-run 和受控执行均已通过"]
            }),
        };

        let first = commit_knowledge_record(&root, request.clone()).unwrap();
        let second = commit_knowledge_record(&root, request).unwrap();

        assert_eq!(first.dedupe_key, second.dedupe_key);
        assert_eq!(first.knowledge_id, second.knowledge_id);
        assert!(!first.safety.external_worktree_created);
        assert!(!first.safety.raw_ue_build_allowed);
        assert!(root.join("index.jsonl").exists());
        assert_eq!(
            fs::read_to_string(root.join("index.jsonl"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        for artifact in &first.artifacts {
            assert!(Path::new(&artifact.path).exists());
        }

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn commit_knowledge_record_does_not_overwrite_same_task_status_variants() {
        let root = temp_root("knowledge-status");
        let base = AwUeWorkflowKnowledgeRequest {
            goal: "修复材质参数同步".to_string(),
            task_id: Some("aw-kb-status".to_string()),
            status: Some("success".to_string()),
            dry_run: Value::Null,
            execution: json!({ "status": "success", "externalWorktreeCreated": true }),
            review: Value::Null,
        };
        let success = commit_knowledge_record(&root, base.clone()).unwrap();
        let blocked = commit_knowledge_record(
            &root,
            AwUeWorkflowKnowledgeRequest {
                status: Some("blocked".to_string()),
                execution: json!({ "status": "blocked" }),
                ..base
            },
        )
        .unwrap();

        assert_ne!(success.knowledge_id, blocked.knowledge_id);
        assert_ne!(success.artifacts[0].path, blocked.artifacts[0].path);
        for artifact in success.artifacts.iter().chain(blocked.artifacts.iter()) {
            assert!(Path::new(&artifact.path).exists());
        }
        assert_eq!(
            fs::read_to_string(root.join("index.jsonl"))
                .unwrap()
                .lines()
                .count(),
            2
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn commit_external_command_execution_writes_audited_record() {
        let root = temp_root("external-command");
        let record = commit_external_command_execution(
            &root,
            "dev.switch.execute",
            "切换到测试任务",
            json!({
                "status": "complete",
                "executedAction": "switch"
            }),
        )
        .unwrap();

        assert_eq!(record.status, "complete");
        assert!(record
            .confirmation_points
            .iter()
            .any(|point| point.contains("dev.switch.execute")));
        assert!(!record.safety.arbitrary_shell_allowed);
        assert!(!record.safety.raw_ue_build_allowed);
        assert!(!record.safety.external_worktree_created);
        for artifact in &record.artifacts {
            assert!(Path::new(&artifact.path).exists());
        }

        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "agentwatcher-ueworkflow-execution-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
