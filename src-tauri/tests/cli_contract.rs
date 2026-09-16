// 输出契约测试：起真实 agentwatcher-cli 二进制，按信封契约逐字段断言。
// 断言只看结构，不依赖本机有没有真实会话数据。

use std::process::Command;

fn run_cli(args: &[&str]) -> (serde_json::Value, String, Option<i32>) {
    let output = Command::new(env!("CARGO_BIN_EXE_agentwatcher-cli"))
        .args(args)
        .output()
        .expect("spawn agentwatcher-cli");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let code = output.status.code();
    let parsed = serde_json::from_str(stdout.trim())
        .unwrap_or(serde_json::Value::String(stdout.clone()));
    (parsed, stderr, code)
}

#[test]
fn json_session_list_emits_single_envelope() {
    let (envelope, _stderr, code) = run_cli(&["--format", "json", "session", "list"]);
    assert_eq!(code, Some(0), "list must exit 0");
    let object = envelope
        .as_object()
        .unwrap_or_else(|| panic!("stdout must be a single JSON object, got: {envelope}"));
    for key in ["command", "ok", "data", "error", "messages"] {
        assert!(object.contains_key(key), "envelope must have {key}");
    }
    assert_eq!(object["command"], "session list");
    assert_eq!(object["ok"], true);
    assert!(object["error"].is_null());
    let sessions = object["data"]["sessions"]
        .as_array()
        .expect("data.sessions must be an array");
    assert_eq!(
        sessions.len(),
        object["data"]["total"].as_u64().unwrap_or(u64::MAX) as usize
    );
    for session in sessions {
        assert!(
            session.get("lastUserMessage").is_none(),
            "list must not leak last user message"
        );
        assert!(
            session.get("lastAiMessage").is_none(),
            "list must not leak last AI message"
        );
        assert!(session.get("id").is_some(), "list item must keep id");
        assert!(session.get("status").is_some(), "list item must keep status");
    }
}

#[test]
fn json_session_show_unknown_id_fails_in_envelope() {
    let (envelope, _stderr, code) = run_cli(&[
        "--format",
        "json",
        "session",
        "show",
        "nosuch:000000000000",
    ]);
    assert_eq!(code, Some(1), "not found must exit 1");
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["command"], "session show");
    assert_eq!(envelope["error"]["code"], "session_not_found");
    assert!(envelope["data"].is_null());
}

#[test]
fn json_session_usage_unknown_id_fails_in_envelope() {
    let (envelope, _stderr, code) = run_cli(&[
        "--format",
        "json",
        "session",
        "usage",
        "nosuch:000000000000",
    ]);
    assert_eq!(code, Some(1), "not found must exit 1");
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["code"], "session_not_found");
}

#[test]
fn json_handoff_export_unknown_id_fails_in_envelope() {
    let (envelope, _stderr, code) = run_cli(&[
        "--format",
        "json",
        "handoff",
        "export",
        "nosuch:000000000000",
    ]);
    assert_eq!(code, Some(1), "not found must exit 1");
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["code"], "session_not_found");
}

#[test]
fn human_session_list_does_not_emit_envelope() {
    let output = Command::new(env!("CARGO_BIN_EXE_agentwatcher-cli"))
        .args(["--format", "human", "session", "list"])
        .output()
        .expect("spawn agentwatcher-cli");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("\"command\""),
        "human mode must not emit the JSON envelope"
    );
    assert!(stdout.contains("STATUS"), "human list prints a table header");
}

#[test]
fn version_flag_exits_zero() {
    let output = Command::new(env!("CARGO_BIN_EXE_agentwatcher-cli"))
        .arg("--version")
        .output()
        .expect("spawn agentwatcher-cli");
    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("agentwatcher-cli"), "version names the binary");
}
