// 命令编排层：解析好的参数 -> 复用 agentwatcher_lib 的扫描/用量/导出 -> 输出。

use crate::output;
use agentwatcher_lib::{
    generate_suggestion_for_session_with, lookup_session_full_history,
    prepare_handoff_source_context_blocking, scan_sessions_blocking, session_usage_detail,
    suggestion_prompt_preview, AgentSession, HandoffSourceContextRequest, ScanOptions,
    SuggestionEngine, SuggestionEngineOptions,
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
    "updatedMs",
    "messageCount",
    "branch",
    "usage",
];

fn scan_options(
    provider: &[String],
    limit: Option<usize>,
    active_days: Option<u64>,
    include_archived: bool,
    exclude_playground: bool,
) -> ScanOptions {
    let mut options = ScanOptions {
        max_sessions: limit,
        active_window_days: active_days,
        hide_archived: Some(!include_archived),
        include_playground: Some(!exclude_playground),
        include_copilot: None,
        include_copilot_cli: None,
        include_claude: None,
        include_codex: None,
        include_open_code: None,
        include_zcode: None,
        include_workbuddy: None,
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
        options.include_workbuddy = picks("workbuddy");
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
    include_archived: bool,
    exclude_playground: bool,
    no_cache: bool,
) {
    // 全历史模式是分钟级重扫描，走跨进程缓存把同参数并发/紧随调用合并为一次；
    // 默认活跃窗口扫描轻量，不碰缓存。
    let full_history = active_days == Some(0) || include_archived;
    // 投影项带一个内部键 `workspaceFallback`（session.workspace 原值），供
    // workspace 过滤在缓存命中后仍按原语义（workspacePath 优先、缺省回退）执行；
    // 输出前剥掉，保证命中缓存与真实扫描的输出逐字段等价。
    let project_with_fallback = |session: &AgentSession| {
        let mut item = project_list_item(session);
        if let serde_json::Value::Object(map) = &mut item {
            map.insert(
                "workspaceFallback".to_string(),
                serde_json::Value::String(session.workspace.clone()),
            );
        }
        item
    };
    let items: Vec<serde_json::Value> = if full_history && !no_cache {
        let key = crate::scan_cache::ScanCacheKey {
            provider: provider.to_vec(),
            limit,
            active_days,
            include_archived,
            exclude_playground,
        };
        let scanned = crate::scan_cache::cached_full_history_scan(&key, || {
            let sessions = scan_sessions_blocking(Some(scan_options(
                provider, limit, active_days, include_archived, exclude_playground,
            )));
            serde_json::Value::Array(sessions.iter().map(project_with_fallback).collect::<Vec<_>>())
        });
        // 缓存里存的是未过滤的全量投影；status/workspace 过滤在内存里做，不进缓存键。
        scanned
            .as_array()
            .cloned()
            .unwrap_or_default()
    } else {
        let sessions = scan_sessions_blocking(Some(scan_options(
            provider, limit, active_days, include_archived, exclude_playground,
        )));
        sessions.iter().map(project_with_fallback).collect::<Vec<_>>()
    };
    let filtered = items
        .iter()
        .filter(|item| {
            status
                .map(|wanted| {
                    item.get("status")
                        .and_then(|value| value.as_str())
                        .map(|value| value.eq_ignore_ascii_case(wanted.trim()))
                        .unwrap_or(false)
                })
                .unwrap_or(true)
        })
        .filter(|item| {
            workspace
                .map(|wanted| {
                    let wanted = wanted.trim();
                    item.get("workspacePath")
                        .and_then(|value| value.as_str())
                        .map(|value| value.trim().eq_ignore_ascii_case(wanted))
                        .or_else(|| {
                            item.get("workspaceFallback")
                                .and_then(|value| value.as_str())
                                .map(|value| value.trim().eq_ignore_ascii_case(wanted))
                        })
                        .unwrap_or(false)
                })
                .unwrap_or(true)
        })
        .collect::<Vec<&serde_json::Value>>();

    let data = serde_json::json!({
        "sessions": filtered
            .iter()
            .map(|item| {
                let mut clean = (*item).clone();
                if let serde_json::Value::Object(map) = &mut clean {
                    map.remove("workspaceFallback");
                }
                clean
            })
            .collect::<Vec<_>>(),
        "total": filtered.len(),
    });

    let mut human = String::new();
    human.push_str(&format!(
        "{:<9}{:<13}{:<18}{:<40}{}\n",
        "STATUS", "PROVIDER", "TIME", "TITLE", "WORKSPACE"
    ));
    for item in &filtered {
        let field = |name: &str| item.get(name).and_then(|value| value.as_str()).unwrap_or("");
        let title = truncate_chars(field("title"), 38);
        let workspace = truncate_chars(
            item.get("workspacePath")
                .and_then(|value| value.as_str())
                .or_else(|| item.get("workspaceFallback").and_then(|value| value.as_str()))
                .unwrap_or(""),
            38,
        );
        human.push_str(&format!(
            "{:<9}{:<13}{:<18}{:<40}{}\n",
            field("status"),
            field("provider"),
            field("timeLabel"),
            title,
            workspace
        ));
    }

    output::finish_success("session list", data, Some(human));
}

fn find_session(id: &str) -> Option<AgentSession> {
    let trimmed = id.trim();
    // id 形如 provider:key：先按该 provider 窄扫描。显式点名查询不该跟全量扫描
    // 抢共享 JSONL 用量预算——扫描顺序靠后的 provider（如 workbuddy）在全量模式下
    // 可能被前面的 provider 把预算花光，拿到没有 usage 的会话。
    if let Some((provider, _)) = trimmed.split_once(':') {
        if KNOWN_PROVIDERS.contains(&provider) {
            if let Some(session) = scan_sessions_blocking(Some(ScanOptions {
                include_copilot: Some(provider == "copilot"),
                include_copilot_cli: Some(provider == "copilot-cli"),
                include_claude: Some(provider == "claude"),
                include_codex: Some(provider == "codex"),
                include_open_code: Some(provider == "opencode"),
                include_zcode: Some(provider == "zcode"),
                include_workbuddy: Some(provider == "workbuddy"),
                ..scan_options(&[], None, None, false, false)
            }))
            .into_iter()
            .find(|session| session.id == trimmed)
            {
                return Some(session);
            }
        }
    }
    if let Some(session) = scan_sessions_blocking(None)
        .into_iter()
        .find(|session| session.id == trimmed)
    {
        return Some(session);
    }
    // 默认参数查不到：会话可能已归档或在 30 天窗口外，定向兜底再查一次。
    lookup_session_full_history(trimmed)
}

/// id 前缀里认识的 provider 名单；不认识的前缀直接走全量扫描与兜底。
const KNOWN_PROVIDERS: &[&str] = &[
    "copilot",
    "copilot-cli",
    "claude",
    "codex",
    "opencode",
    "zcode",
    "workbuddy",
];

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
    // Codex 归档的 .zst 对调用方不可读，--content 给解压后的 JSONL 副本。
    if let Some(path) = session.session_path.as_deref() {
        if path.to_ascii_lowercase().ends_with(".zst") {
            return match agentwatcher_lib::export_codex_zst_transcript(path) {
                Ok(decompressed) => serde_json::json!({
                    "rawSessionPath": decompressed,
                    "rawSessionArchive": path,
                }),
                Err(message) => serde_json::json!({
                    "rawSessionPath": path,
                    "warning": format!("解压归档失败：{message}"),
                }),
            };
        }
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
        Ok(mut context) => {
            // zst 归档换成解压副本路径，接续方才能读到内容；原始归档路径留在 CLI 层换，GUI 不受影响。
            if let Some(path) = context.primary_source_file.clone() {
                if path.to_ascii_lowercase().ends_with(".zst") {
                    if let Ok(decompressed) = agentwatcher_lib::export_codex_zst_transcript(&path) {
                        context.primary_source_file = Some(decompressed);
                    }
                }
            }
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

/// 建议生成：按会话所属 agent 路由，未接线的 provider 明确报错而不是静默空结果。
pub fn suggest_generate(
    id: &str,
    avoid: Option<&str>,
    model: Option<&str>,
    effort: Option<&str>,
    timeout_secs: Option<u64>,
    engine: Option<&str>,
) {
    let is_supported = id.starts_with("codex:")
        || id.starts_with("codex://")
        || id.starts_with("zcode:")
        || id.starts_with("workbuddy:");
    if !is_supported {
        output::finish_failure(
            "suggest generate",
            "unsupported_provider",
            format!("会话 {id} 的 agent 未接线，目前只支持 codex / zcode / workbuddy"),
        );
    }
    let engine = match engine {
        Some(value) => SuggestionEngine::parse(value).unwrap_or_else(|| {
            output::finish_failure(
                "suggest generate",
                "invalid_argument",
                format!("未知引擎 {value}，可选 auto / sample / exec"),
            )
        }),
        None => SuggestionEngine::Auto,
    };
    let options = SuggestionEngineOptions {
        model: model.map(str::to_string),
        effort: effort.map(str::to_string),
        timeout_secs,
        engine,
    };
    // sample 继承 fork 出的父线程配置，模型与档位不由 CLI 决定。
    // 显式指定时要说明清楚，否则会误以为改档生效了。
    if engine == SuggestionEngine::Sample && (model.is_some() || effort.is_some()) {
        eprintln!(
            "提示：--engine sample 继承会话本身的模型与档位，--model / --effort 对它不生效；需要指定档位请用 --engine exec"
        );
    }
    match generate_suggestion_for_session_with(id, avoid, &options) {
        Some(suggestion) => {
            let data = serde_json::to_value(&suggestion).expect("suggestion always serializes");
            let human = format!("{}\n", suggestion.text);
            output::finish_success("suggest generate", data, Some(human));
        }
        None => output::finish_failure(
            "suggest generate",
            "generation_failed",
            "建议生成失败：可能是会话文件不存在、资格校验未通过、引擎超时或输出无法解析；stderr 里有分类日志".to_string(),
        ),
    }
}

/// 只出提示词不调模型，便于核对 v5 契约与上下文。
pub fn suggest_prompt(id: &str) {
    match suggestion_prompt_preview(id) {
        Ok(preview) => {
            let human = format!("{}\n", preview.prompt);
            let data = serde_json::to_value(&preview).expect("preview always serializes");
            output::finish_success("suggest prompt", data, Some(human));
        }
        Err(message) => output::finish_failure("suggest prompt", "preview_failed", message),
    }
}
