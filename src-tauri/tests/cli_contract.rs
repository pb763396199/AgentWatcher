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

#[test]
fn skill_lifecycle_in_explicit_dir_is_idempotent() {
    let dir = std::env::temp_dir().join(format!("awc-skill-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let dir_arg = dir.to_string_lossy().into_owned();
    let skill_file = dir.join("agentwatcher").join("SKILL.md");

    // 安装前：list 报 host-absent（目录还不存在）
    let (envelope, _stderr, code) = run_cli(&["--format", "json", "skill", "list", "--dir", &dir_arg]);
    assert_eq!(code, Some(0));
    assert_eq!(envelope["command"], "skill list");
    assert_eq!(envelope["data"]["hosts"][0]["state"], "host-absent");

    // 安装：文件落盘、带 name 头
    let (envelope, _stderr, code) = run_cli(&["--format", "json", "skill", "install", "--dir", &dir_arg]);
    assert_eq!(code, Some(0));
    assert_eq!(envelope["ok"], true);
    let installed_count = envelope["data"]["installed"].as_array().map(Vec::len).unwrap_or(0);
    assert_eq!(installed_count, 1);
    let content = std::fs::read_to_string(&skill_file).expect("skill file written");
    assert!(content.contains("name: agentwatcher"), "skill doc has frontmatter");
    assert!(content.contains("session list"), "skill doc teaches the CLI");

    // 状态：installed
    let (envelope, _stderr, _code) = run_cli(&["--format", "json", "skill", "list", "--dir", &dir_arg]);
    assert_eq!(envelope["data"]["hosts"][0]["state"], "installed");

    // 改写文件后：outdated；重装恢复 installed（覆盖升级通道）
    std::fs::write(&skill_file, "stale").expect("tamper skill file");
    let (envelope, _stderr, _code) = run_cli(&["--format", "json", "skill", "list", "--dir", &dir_arg]);
    assert_eq!(envelope["data"]["hosts"][0]["state"], "outdated");
    run_cli(&["--format", "json", "skill", "install", "--dir", &dir_arg]);
    let (envelope, _stderr, _code) = run_cli(&["--format", "json", "skill", "list", "--dir", &dir_arg]);
    assert_eq!(envelope["data"]["hosts"][0]["state"], "installed");

    // 移除：文件消失、目录清空；再移除幂等（missing 报告）
    let (envelope, _stderr, code) = run_cli(&["--format", "json", "skill", "remove", "--dir", &dir_arg]);
    assert_eq!(code, Some(0));
    assert_eq!(envelope["data"]["removed"].as_array().map(Vec::len).unwrap_or(0), 1);
    assert!(!skill_file.exists());
    let (envelope, _stderr, _code) = run_cli(&["--format", "json", "skill", "remove", "--dir", &dir_arg]);
    assert_eq!(envelope["data"]["missing"].as_array().map(Vec::len).unwrap_or(0), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn skill_list_without_dir_reports_host_candidates() {
    // 只读命令，不打真实宿主目录：断言信封形状与状态词表
    let (envelope, _stderr, code) = run_cli(&["--format", "json", "skill", "list"]);
    assert_eq!(code, Some(0));
    assert_eq!(envelope["command"], "skill list");
    let hosts = envelope["data"]["hosts"]
        .as_array()
        .expect("data.hosts is an array");
    let valid_states = ["installed", "outdated", "not-installed", "host-absent"];
    for host in hosts {
        assert!(host.get("host").is_some(), "entry has host");
        assert!(host.get("path").is_some(), "entry has path");
        let state = host["state"].as_str().unwrap_or_default();
        assert!(
            valid_states.contains(&state),
            "state {state} must be one of {valid_states:?}"
        );
    }
}
