use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const AW_SCHEMA_VERSION: u32 = 1;

pub const ORCHESTRATION_KIND: &str = "AgentWatcherOrchestrationStatus";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AwOrchestrationStatus {
    pub schema_version: u32,
    pub kind: String,
    pub name: String,
    pub display_name: String,
    pub layer: String,
    pub workspace_root: Option<String>,
    pub runtimes: Vec<AwOrchestrationRuntime>,
    pub active_plan: Option<AwOrchestrationPlan>,
    pub module_bindings: Vec<AwOrchestrationModuleBinding>,
    pub safety: AwOrchestrationSafety,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AwOrchestrationRuntime {
    pub name: String,
    pub display_name: String,
    pub role: String,
    pub status: AwOrchestrationRuntimeStatus,
    pub ordinary_module: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AwOrchestrationRuntimeStatus {
    Available,
    Missing,
    Planned,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AwOrchestrationPlan {
    pub status: String,
    pub total_goals: usize,
    pub complete_goals: usize,
    pub pending_goals: usize,
    pub active_goals: usize,
    pub failed_goals: usize,
    pub next_goal_title: Option<String>,
    pub current_goal_title: Option<String>,
    pub goals: Vec<AwOrchestrationGoalStatus>,
    pub ledger: AwOrchestrationLedgerSummary,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AwOrchestrationGoalStatus {
    pub goal_key: String,
    pub title: String,
    pub status: String,
    pub current: bool,
    pub has_evidence: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AwOrchestrationLedgerSummary {
    pub ledger_available: bool,
    pub ledger_entries: usize,
    pub evidence_entries: usize,
    pub checkpoint_entries: usize,
    pub tail_entries: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AwOrchestrationModuleBinding {
    pub module_id: String,
    pub display_name: String,
    pub responsibility: String,
    pub plugin_name: String,
    pub commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AwOrchestrationSafety {
    pub arbitrary_shell_allowed: bool,
    pub raw_ue_build_allowed: bool,
    pub destructive_actions_allowed_by_default: bool,
    pub dry_run_required_before_execute: bool,
    pub execution_boundary: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UltraGoalFile {
    #[serde(default)]
    goals: Vec<UltraGoalEntry>,
    #[serde(default)]
    ledger_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UltraGoalEntry {
    id: String,
    title: String,
    status: String,
    #[serde(default)]
    evidence: Option<String>,
}

pub fn get_orchestration_status(start_dir: &Path) -> Result<AwOrchestrationStatus, String> {
    let workspace_root = find_workspace_root(start_dir);
    let ultragoal_path = workspace_root
        .as_ref()
        .map(|root| root.join(".omx").join("ultragoal").join("goals.json"));
    let mut warnings = Vec::new();

    let active_plan = match (&workspace_root, ultragoal_path.as_ref()) {
        (Some(root), Some(path)) if path.exists() => Some(read_ultragoal_plan(root, path)?),
        _ => {
            warnings
                .push("UltraGoal 计划尚未初始化或当前运行目录未发现 .omx/ultragoal。".to_string());
            None
        }
    };

    Ok(AwOrchestrationStatus {
        schema_version: AW_SCHEMA_VERSION,
        kind: ORCHESTRATION_KIND.to_string(),
        name: "AgentWatcherOrchestration".to_string(),
        display_name: "编排层".to_string(),
        layer: "orchestration".to_string(),
        workspace_root: workspace_root
            .as_ref()
            .map(|root| root.to_string_lossy().to_string()),
        runtimes: orchestration_runtimes(workspace_root.as_deref()),
        active_plan,
        module_bindings: module_bindings(),
        safety: AwOrchestrationSafety {
            arbitrary_shell_allowed: false,
            raw_ue_build_allowed: false,
            destructive_actions_allowed_by_default: false,
            dry_run_required_before_execute: true,
            execution_boundary: "清单声明能力 + dry-run + 明确确认 + 审计记录".to_string(),
        },
        warnings,
    })
}

fn read_ultragoal_plan(root: &Path, path: &Path) -> Result<AwOrchestrationPlan, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read UltraGoal plan: {error}"))?;
    let parsed: UltraGoalFile = serde_json::from_str(&text)
        .map_err(|error| format!("Failed to parse UltraGoal plan: {error}"))?;
    let ledger = read_ledger_summary(root, parsed.ledger_path.as_deref());

    let mut counts = BTreeMap::<String, usize>::new();
    for goal in &parsed.goals {
        *counts.entry(goal.status.clone()).or_insert(0) += 1;
    }

    let current_goal = parsed
        .goals
        .iter()
        .find(|goal| matches!(goal.status.as_str(), "in_progress" | "review-blocked"));
    let next_goal =
        current_goal.or_else(|| parsed.goals.iter().find(|goal| goal.status != "complete"));
    let next_goal_key = next_goal.map(|goal| public_goal_key(&goal.id));

    let goals = parsed
        .goals
        .iter()
        .map(|goal| {
            let goal_key = public_goal_key(&goal.id);
            AwOrchestrationGoalStatus {
                current: next_goal_key.as_deref() == Some(goal_key.as_str()),
                goal_key,
                title: sanitize_public_text(&goal.title),
                status: goal.status.clone(),
                has_evidence: goal
                    .evidence
                    .as_ref()
                    .map(|value| !value.trim().is_empty())
                    .unwrap_or(false),
            }
        })
        .collect::<Vec<_>>();

    let total_goals = parsed.goals.len();
    let complete_goals = count_status(&counts, "complete");
    let failed_goals = count_status(&counts, "failed");
    let pending_goals = total_goals.saturating_sub(complete_goals + failed_goals);
    let active_goals = count_status(&counts, "in_progress")
        + count_status(&counts, "review-blocked")
        + count_status(&counts, "needs-user-decision");

    Ok(AwOrchestrationPlan {
        status: if total_goals > 0 && complete_goals == total_goals {
            "complete".to_string()
        } else {
            "active".to_string()
        },
        total_goals,
        complete_goals,
        pending_goals,
        active_goals,
        failed_goals,
        next_goal_title: next_goal.map(|goal| sanitize_public_text(&goal.title)),
        current_goal_title: current_goal.map(|goal| sanitize_public_text(&goal.title)),
        goals,
        ledger,
    })
}

fn read_ledger_summary(root: &Path, ledger_path: Option<&str>) -> AwOrchestrationLedgerSummary {
    let path = ledger_path
        .and_then(|value| resolve_workspace_path(root, value))
        .unwrap_or_else(|| root.join(".omx").join("ultragoal").join("ledger.jsonl"));
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(_) => {
            return AwOrchestrationLedgerSummary {
                ledger_available: false,
                ledger_entries: 0,
                evidence_entries: 0,
                checkpoint_entries: 0,
                tail_entries: 0,
            }
        }
    };

    let lines = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    let ledger_entries = lines.len();
    let evidence_entries = lines
        .iter()
        .filter(|line| line.contains("\"evidence\""))
        .count();
    let checkpoint_entries = lines
        .iter()
        .filter(|line| line.contains("\"checkpoint\"") || line.contains("\"status\""))
        .count();

    AwOrchestrationLedgerSummary {
        ledger_available: true,
        ledger_entries,
        evidence_entries,
        checkpoint_entries,
        tail_entries: ledger_entries.min(12),
    }
}

fn resolve_workspace_path(root: &Path, value: &str) -> Option<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        return path.starts_with(root).then_some(path);
    }
    let mut resolved = root.to_path_buf();
    for segment in trimmed.replace('\\', "/").split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".." {
            return None;
        }
        resolved.push(segment);
    }
    Some(resolved)
}

fn find_workspace_root(start_dir: &Path) -> Option<PathBuf> {
    let start = fs::canonicalize(start_dir).unwrap_or_else(|_| start_dir.to_path_buf());
    for candidate in start.ancestors() {
        if candidate
            .join(".omx")
            .join("ultragoal")
            .join("goals.json")
            .exists()
        {
            return Some(candidate.to_path_buf());
        }
    }
    for candidate in start.ancestors() {
        if candidate.join(".omx").exists() {
            return Some(candidate.to_path_buf());
        }
    }
    None
}

fn orchestration_runtimes(root: Option<&Path>) -> Vec<AwOrchestrationRuntime> {
    let omx_available = root
        .map(|value| value.join(".omx").exists())
        .unwrap_or(false);
    let ultragoal_available = root
        .map(|value| {
            value
                .join(".omx")
                .join("ultragoal")
                .join("goals.json")
                .exists()
        })
        .unwrap_or(false);

    vec![
        AwOrchestrationRuntime {
            name: "OMX".to_string(),
            display_name: "OMX".to_string(),
            role: "目标拆解与审计入口".to_string(),
            status: if omx_available {
                AwOrchestrationRuntimeStatus::Available
            } else {
                AwOrchestrationRuntimeStatus::Missing
            },
            ordinary_module: false,
        },
        AwOrchestrationRuntime {
            name: "UltraGoal".to_string(),
            display_name: "UltraGoal".to_string(),
            role: "阶段目标和 checkpoint 账本".to_string(),
            status: if ultragoal_available {
                AwOrchestrationRuntimeStatus::Available
            } else {
                AwOrchestrationRuntimeStatus::Missing
            },
            ordinary_module: false,
        },
        AwOrchestrationRuntime {
            name: "UltraWork".to_string(),
            display_name: "UltraWork".to_string(),
            role: "后续并行执行编排".to_string(),
            status: if omx_available {
                AwOrchestrationRuntimeStatus::Planned
            } else {
                AwOrchestrationRuntimeStatus::Missing
            },
            ordinary_module: false,
        },
    ]
}

fn module_bindings() -> Vec<AwOrchestrationModuleBinding> {
    Vec::new()
}

fn count_status(counts: &BTreeMap<String, usize>, status: &str) -> usize {
    counts.get(status).copied().unwrap_or(0)
}

fn public_goal_key(id: &str) -> String {
    id.split('-').next().unwrap_or(id).trim().to_string()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn missing_ultragoal_plan_returns_safe_bootstrap_status() {
        let root = temp_root("missing");
        fs::create_dir_all(&root).unwrap();

        let status = get_orchestration_status(&root).unwrap();

        assert_eq!(status.kind, ORCHESTRATION_KIND);
        assert!(status.active_plan.is_none());
        assert!(status.module_bindings.is_empty());
        assert!(!status.safety.arbitrary_shell_allowed);
        assert!(!status.safety.raw_ue_build_allowed);
        assert!(status
            .warnings
            .iter()
            .any(|item| item.contains("UltraGoal")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reads_ultragoal_plan_as_public_orchestration_status() {
        let root = temp_root("plan");
        let ultragoal = root.join(".omx").join("ultragoal");
        fs::create_dir_all(&ultragoal).unwrap();
        fs::write(
            ultragoal.join("goals.json"),
            r#"{
              "version": 1,
              "ledgerPath": ".omx/ultragoal/ledger.jsonl",
              "goals": [
                {
                  "id": "G001-plugin-host",
                  "title": "目标一：通用插件宿主定型",
                  "status": "complete",
                  "evidence": "ok"
                },
                {
                  "id": "G004-plugin-capabilities",
                  "title": "目标四：插件能力改造",
                  "status": "pending"
                }
              ]
            }"#,
        )
        .unwrap();
        fs::write(
            ultragoal.join("ledger.jsonl"),
            "{\"event\":\"checkpoint\",\"evidence\":\"ok\"}\n{\"event\":\"status\"}\n",
        )
        .unwrap();

        let status = get_orchestration_status(&root).unwrap();
        let plan = status.active_plan.unwrap();
        let serialized = serde_json::to_string(&plan).unwrap();

        assert_eq!(plan.total_goals, 2);
        assert_eq!(plan.complete_goals, 1);
        assert_eq!(plan.pending_goals, 1);
        assert_eq!(
            plan.next_goal_title.as_deref(),
            Some("目标四：插件能力改造")
        );
        assert_eq!(plan.ledger.ledger_entries, 2);
        assert!(!serialized.contains(&legacy_source_name(&["Unreal", "DevFlow"], "")));
        assert!(!serialized.contains(&legacy_source_name(&["UE", "Master", "Agent"], "_")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prefers_parent_ultragoal_plan_over_nested_runtime_state() {
        let root = temp_root("nested");
        let child = root.join("AgentWatcher");
        let ultragoal = root.join(".omx").join("ultragoal");
        fs::create_dir_all(child.join(".omx").join("state")).unwrap();
        fs::create_dir_all(&ultragoal).unwrap();
        fs::write(
            ultragoal.join("goals.json"),
            r#"{
              "version": 1,
              "goals": [
                {
                  "id": "G001-agentwatcher-ue",
                  "title": "目标一：总控台插件底座定型",
                  "status": "complete"
                }
              ]
            }"#,
        )
        .unwrap();

        let status = get_orchestration_status(&child).unwrap();

        assert_eq!(status.active_plan.unwrap().total_goals, 1);
        assert!(status
            .workspace_root
            .as_deref()
            .unwrap()
            .ends_with(root.file_name().unwrap().to_string_lossy().as_ref()));

        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "agentwatcher-orchestration-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
}
