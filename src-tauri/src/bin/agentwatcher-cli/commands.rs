// 命令编排层：解析好的参数 -> 复用 agentwatcher_lib 的扫描/用量/导出 -> 输出。

use crate::output;
use agentwatcher_lib::{
    prepare_handoff_source_context_blocking, scan_sessions_blocking, session_usage_detail,
    AgentSession, HandoffSourceContextRequest, ScanOptions,
};

/// 列表只暴露卡片级元数据；最后输入/输出等预览文本一律不进 list。
const LIST_KEYS: &[&str] = &[
    "id",
    "provider",
    "providerLabel",
    "title",
    "workspaceLabel",
    "workspacePath",
    "status",
    "timeLabel",
    "updatedAtMs",
    "messageCount",
    "branch",
    "usage",
];

fn scan_options(
    provider: &[String],
    limit: Option<usize>,
    active_days: Option<u64>,
) -> ScanOptions {
    let mut options = ScanOptions {
        max_sessions: limit,
        active_window_days: active_days,
        hide_archived: None,
        include_copilot: None,
        include_copilot_cli: None,
        include_claude: None,
        include_codex: None,
        include_open_code: None,
        include_zcode: None,
        workspace_path_blacklist: None,
    };
    if !provider.is_empty() {
        let selected: Vec<String> = provider
            .iter()
            .map(|name| name.trim().to_ascii_lowercase())
            .collect();
        let picks = |name: &str| Some(selected.iter().any(|item| item == name));
        options.include_copilot = picks("copilot");
        options.include_copilot_cli = picks("copilot-cli");
        options.include_claude = picks("claude");
        options.include_codex = picks("codex");
        options.include_open_code = picks("opencode");
        options.include_zcode = picks("zcode");
    }
    options
}

fn serialize_session(session: &AgentSession) -> serde_json::Value {
    serde_json::to_value(session).expect("AgentSession always serializes")
}

fn project_list_item(session: &AgentSession) -> serde_json::Value {
    let full = serialize_session(session);
    let mut out = serde_json::Map::new();
    if let serde_json::Value::Object(map) = full {
        for key in LIST_KEYS {
            if let Some(value) = map.get(*key) {
                out.insert((*key).to_string(), value.clone());
            }
        }
    }
    serde_json::Value::Object(out)
}

fn workspace_matches(session: &AgentSession, workspace: &str) -> bool {
    let wanted = workspace.trim();
    session
        .workspace_path
        .as_deref()
        .map(|path| path.trim().eq_ignore_ascii_case(wanted))
        .unwrap_or_else(|| session.workspace.trim().eq_ignore_ascii_case(wanted))
}

fn truncate_chars(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_string()
    } else {
        let mut truncated: String = text.chars().take(width.saturating_sub(1)).collect();
        truncated.push('…');
        truncated
    }
}

pub fn session_list(
    provider: &[String],
    status: Option<&str>,
    workspace: Option<&str>,
    limit: Option<usize>,
    active_days: Option<u64>,
) {
    let sessions = scan_sessions_blocking(Some(scan_options(provider, limit, active_days)));
    let filtered: Vec<&AgentSession> = sessions
        .iter()
        .filter(|session| {
            status
                .map(|wanted| session.status.eq_ignore_ascii_case(wanted.trim()))
                .unwrap_or(true)
        })
        .filter(|session| {
            workspace
                .map(|wanted| workspace_matches(session, wanted))
                .unwrap_or(true)
        })
        .collect();

    let items: Vec<serde_json::Value> = filtered.iter().map(|session| project_list_item(session)).collect();
    let data = serde_json::json!({
        "sessions": items,
        "total": filtered.len(),
    });

    let mut human = String::new();
    human.push_str(&format!(
        "{:<9}{:<13}{:<18}{:<40}{}\n",
        "STATUS", "PROVIDER", "TIME", "TITLE", "WORKSPACE"
    ));
    for session in &filtered {
        let title = truncate_chars(&session.title, 38);
        let workspace = truncate_chars(
            session.workspace_path.as_deref().unwrap_or(&session.workspace),
            38,
        );
        human.push_str(&format!(
            "{:<9}{:<13}{:<18}{:<40}{}\n",
            session.status, session.provider, session_time_label(session), title, workspace
        ));
    }

    output::finish_success("session list", data, Some(human));
}

fn session_time_label(session: &AgentSession) -> String {
    serialize_session(session)
        .get("timeLabel")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string()
}

fn find_session(id: &str) -> Option<AgentSession> {
    let sessions = scan_sessions_blocking(None);
    sessions.into_iter().find(|session| session.id == id.trim())
}

fn handoff_request_for(session: &AgentSession) -> HandoffSourceContextRequest {
    HandoffSourceContextRequest {
        id: Some(session.id.clone()),
        provider: Some(session.provider.clone()),
        workspace_path: session.workspace_path.clone(),
        session_path: session.session_path.clone(),
        session_resource: session.session_resource.clone(),
        title: Some(session.title.clone()),
    }
}

fn render_session_human(session: &AgentSession) -> String {
    let value = serialize_session(session);
    let mut lines = Vec::new();
    let ordered = [
        "id",
        "provider",
        "title",
        "status",
        "workspaceLabel",
        "workspacePath",
        "timeLabel",
        "messageCount",
        "branch",
        "lastUserMessage",
        "lastAiMessage",
    ];
    for key in ordered {
        if let Some(text) = value.get(key).and_then(|item| item.as_str()) {
            lines.push(format!("{key}: {text}"));
        }
    }
    format!("{}\n", lines.join("\n"))
}

pub fn session_show(id: &str, content: bool) {
    let Some(session) = find_session(id) else {
        output::finish_failure(
            "session show",
            "session_not_found",
            format!("找不到会话 {id}，先跑 session list 拿有效 ID"),
        );
    };
    let mut data = serde_json::json!({ "session": serialize_session(&session) });
    if content {
        data["content"] = content_block_for(&session);
    }
    output::finish_success("session show", data, Some(render_session_human(&session)));
}

fn content_block_for(session: &AgentSession) -> serde_json::Value {
    let provider = session.provider.as_str();
    if provider == "opencode" || provider == "zcode" {
        let request = handoff_request_for(session);
        return match prepare_handoff_source_context_blocking(request) {
            Ok(context) => serde_json::to_value(&context).expect("context always serializes"),
            Err(message) => serde_json::json!({ "warning": message }),
        };
    }
    serde_json::json!({ "rawSessionPath": session.session_path })
}

pub fn session_usage(id: &str) {
    if find_session(id).is_none() {
        output::finish_failure(
            "session usage",
            "session_not_found",
            format!("找不到会话 {id}，先跑 session list 拿有效 ID"),
        );
    }
    let detail = session_usage_detail(id.trim().to_string());
    let value = serde_json::to_value(&detail).expect("usage detail always serializes");
    let mut human = String::new();
    for key in ["totalInputTokens", "totalOutputTokens", "userTurns", "toolCalls"] {
        if let Some(number) = value.get("usage").and_then(|usage| usage.get(key)) {
            if !number.is_null() {
                human.push_str(&format!("{key}: {number}\n"));
            }
        }
    }
    output::finish_success("session usage", value, Some(human));
}

pub fn handoff_export(id: &str) {
    let Some(session) = find_session(id) else {
        output::finish_failure(
            "handoff export",
            "session_not_found",
            format!("找不到会话 {id}，先跑 session list 拿有效 ID"),
        );
    };
    let request = handoff_request_for(&session);
    match prepare_handoff_source_context_blocking(request) {
        Ok(context) => {
            let human = context
                .primary_source_file
                .as_deref()
                .map(|path| format!("exported: {path}\n"))
                .unwrap_or_else(|| "exported: (no source file)\n".to_string());
            let data = serde_json::to_value(&context).expect("context always serializes");
            output::finish_success("handoff export", data, Some(human));
        }
        Err(message) => output::finish_failure("handoff export", "export_failed", message),
    }
}
