use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Runtime {
    home: PathBuf,
}

impl Runtime {
    fn new() -> Self {
        let home = std::env::temp_dir().join(format!(
            "aw-workflow-contract-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        Self { home }
    }

    fn cli(&self, args: &[&str], input: Option<&Value>) -> (i32, Value) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_agentwatcher-cli"))
            .arg("--workflow-home")
            .arg(&self.home)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn CLI");
        if let Some(input) = input {
            child
                .stdin
                .take()
                .unwrap()
                .write_all(input.to_string().as_bytes())
                .unwrap();
        }
        drop(child.stdin.take());
        let output = child.wait_with_output().unwrap();
        let value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
            panic!(
                "invalid envelope: {} / {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
        (output.status.code().unwrap_or(-1), value)
    }

    fn request(&self, id: &str, command: &str, payload: Value) -> Value {
        let (_, result) = self.cli(
            &["workflow", "apply", "--file", "-"],
            Some(&json!({
                "schemaVersion": 1, "requestId": id, "command": command, "payload": payload
            })),
        );
        result
    }

    fn setup(&self, code: bool) {
        assert_eq!(
            self.request(
                "project",
                "project.create",
                json!({"id":"p", "title":"Project"})
            )["ok"],
            true
        );
        let result = self.request("publish", "roadmap.publish", json!({
            "id":"r", "projectId":"p", "title":"Route", "expectedRevision":0,
            "humanConfirmed":true,
            "nodes":[{"id":"a","title":"First","goal":"Deliver evidence","acceptance":["Evidence exists"],"code":code},
                     {"id":"b","title":"Second","goal":"Use first","acceptance":["Uses first"],"dependencies":["a"]}]
        }));
        assert_eq!(result["ok"], true, "{result}");
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = Command::new(env!("CARGO_BIN_EXE_agentwatcher-cli"))
            .arg("--workflow-home")
            .arg(&self.home)
            .args(["service", "stop"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        // Test-owned directory; preserve it on failure for diagnosis.
        if !std::thread::panicking() {
            let _ = std::fs::remove_dir_all(&self.home);
        }
    }
}

#[test]
fn workflow_status_is_read_only_and_writes_wake_single_service() {
    let runtime = Runtime::new();
    let (_, status) = runtime.cli(&["service", "status"], None);
    assert_eq!(status["data"]["running"], false);
    assert!(!runtime.home.exists(), "status must not create a database");
    runtime.setup(false);
    let (_, first) = runtime.cli(&["service", "status"], None);
    let (_, second) = runtime.cli(&["service", "start"], None);
    assert_eq!(first["data"]["pid"], second["data"]["pid"]);
    assert_eq!(runtime.cli(&["service", "stop"], None).0, 0);
    let (_, snapshot) = runtime.cli(&["workflow", "snapshot", "--project", "p"], None);
    assert_eq!(snapshot["data"]["roadmaps"][0]["revision"], 1);
}

#[test]
fn workflow_cli_watch_cursor_and_next_actions_are_machine_readable() {
    let runtime = Runtime::new();
    runtime.setup(false);
    let (_, events) = runtime.cli(
        &["workflow", "watch", "--project", "p", "--after", "0"],
        None,
    );
    assert_eq!(events["ok"], true, "{events}");
    assert!(events["data"]["cursor"].as_u64().unwrap_or(0) > 0);
    assert!(!events["data"]["events"].as_array().unwrap().is_empty());
    let (_, empty) = runtime.cli(
        &["workflow", "watch", "--project", "p", "--after", "999999"],
        None,
    );
    assert_eq!(empty["data"]["events"], json!([]));
    let claim = runtime.request(
        "watch-claim",
        "node.claim",
        json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":"codex:watch"}),
    );
    let handoff = runtime.request("watch-handoff", "execution.handoff", json!({"executionId":claim["data"]["executionId"],"token":claim["data"]["token"],"expectedRevision":1,"summary":"watchable","verification":[{"criterion":"Evidence exists","outcome":"passed","evidence":"fixture"}],"artifacts":[]}));
    assert_eq!(handoff["ok"], true, "{handoff}");
    let (_, next) = runtime.cli(&["workflow", "next", "--project", "p"], None);
    assert_eq!(next["ok"], true, "{next}");
    assert_eq!(next["data"]["actions"][0]["kind"], "handoff");
    assert_eq!(
        next["data"]["actions"][0]["handoffId"],
        handoff["data"]["handoffId"]
    );
}

#[test]
fn workflow_claim_and_receipt_are_exclusive_idempotent_and_revision_checked() {
    let runtime = Runtime::new();
    runtime.setup(false);
    let payload =
        json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":"codex:first"});
    let claim = runtime.request("claim", "node.claim", payload.clone());
    assert_eq!(claim["ok"], true, "{claim}");
    assert_eq!(claim, runtime.request("claim", "node.claim", payload));
    let competing = runtime.request(
        "compete",
        "node.claim",
        json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":"codex:second"}),
    );
    assert_eq!(competing["error"]["code"], "already_claimed");
    let conflict = runtime.request(
        "claim",
        "project.create",
        json!({"id":"other","title":"other"}),
    );
    assert_eq!(conflict["error"]["code"], "request_conflict");
    let dependent = runtime.request(
        "dependent",
        "node.claim",
        json!({"roadmapId":"r","nodeId":"b","expectedRevision":1,"sessionId":"codex:second"}),
    );
    assert_eq!(dependent["error"]["code"], "dependencies_not_ready");
    let id = claim["data"]["executionId"].as_str().unwrap();
    let token = claim["data"]["token"].as_str().unwrap();
    let handoff = json!({"executionId":id,"token":token,"expectedRevision":1,"summary":"Evidence delivered",
        "verification":[{"criterion":"Evidence exists","outcome":"passed","evidence":"checked local output"}],"artifacts":[]});
    let receipt = runtime.request("receipt", "execution.handoff", handoff.clone());
    assert_eq!(receipt["ok"], true, "{receipt}");
    assert_eq!(
        receipt,
        runtime.request("receipt", "execution.handoff", handoff)
    );
    let accepted = runtime.request(
        "accept",
        "handoff.accept",
        json!({"handoffId":receipt["data"]["handoffId"],"humanConfirmed":true}),
    );
    assert_eq!(accepted["ok"], true, "{accepted}");
    let next = runtime.request(
        "next",
        "node.claim",
        json!({"roadmapId":"r","nodeId":"b","expectedRevision":1,"sessionId":"codex:second"}),
    );
    assert_eq!(next["ok"], true, "{next}");
    assert_eq!(
        next["data"]["upstream"][0]["handoffId"],
        receipt["data"]["handoffId"]
    );
    let (_, snapshot) = runtime.cli(&["workflow", "snapshot"], None);
    assert_eq!(snapshot["data"]["handoffs"][0]["body"]["inputs"], json!([]));
    assert!(snapshot["data"]["handoffs"][0]["body"]["nodeContractDigest"].is_string());
}

#[test]
fn workflow_changed_upstream_invalidates_downstream_results_and_scoped_authorization() {
    let runtime = Runtime::new();
    runtime.setup(false);
    for (node, criterion) in [("a", "Evidence exists"), ("b", "Uses first")] {
        let claim=runtime.request(&format!("claim-{node}"),"node.claim",json!({"roadmapId":"r","nodeId":node,"expectedRevision":1,"sessionId":format!("codex:{node}")}));
        assert_eq!(claim["ok"], true, "{claim}");
        let handoff=runtime.request(&format!("handoff-{node}"),"execution.handoff",json!({"executionId":claim["data"]["executionId"],"token":claim["data"]["token"],"expectedRevision":1,"summary":"verified","verification":[{"criterion":criterion,"outcome":"passed","evidence":"actual fixture"}],"artifacts":[]}));
        assert_eq!(
            runtime.request(
                &format!("accept-{node}"),
                "handoff.accept",
                json!({"handoffId":handoff["data"]["handoffId"],"humanConfirmed":true})
            )["ok"],
            true
        );
    }
    runtime.request(
        "pause",
        "control.set",
        json!({"projectId":"p","state":"paused","humanConfirmed":true}),
    );
    assert_eq!(
        runtime.request(
            "grant",
            "authorization.grant",
            json!({"roadmapId":"r","nodeId":"b","expectedRevision":1,"humanConfirmed":true})
        )["ok"],
        true
    );
    let proposal=runtime.request("proposal","roadmap.propose",json!({"roadmapId":"r","expectedRevision":1,"reason":"Upstream requirement wrong","affectedNodes":["a"]}));
    let changed=runtime.request("revised","roadmap.publish",json!({"id":"r","projectId":"p","title":"Route","expectedRevision":1,"proposalId":proposal["data"]["proposalId"],"humanConfirmed":true,"nodes":[
        {"id":"a","title":"First","goal":"Deliver corrected evidence","acceptance":["Evidence exists"],"code":false},
        {"id":"b","title":"Second","goal":"Use first","acceptance":["Uses first"],"dependencies":["a"]}]}));
    assert_eq!(changed["ok"], true, "{changed}");
    let (_, snapshot) = runtime.cli(&["workflow", "snapshot"], None);
    assert!(snapshot["data"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["state"] == "recheck"));
    assert_eq!(snapshot["data"]["grants"][0]["state"], "invalidated");
}

#[test]
fn workflow_restore_refuses_to_replace_active_execution_state() {
    let runtime = Runtime::new();
    runtime.setup(false);
    runtime.request(
        "claim",
        "node.claim",
        json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":"codex:owner"}),
    );
    let backup = runtime.request("backup", "storage.backup", json!({"humanConfirmed":true}));
    runtime.cli(&["service", "stop"], None);
    let (_, restored) = runtime.cli(
        &[
            "workflow",
            "restore",
            "--file",
            backup["data"]["path"].as_str().unwrap(),
            "--confirm",
        ],
        None,
    );
    assert_eq!(restored["error"]["code"], "active_executions");
    let (_, snapshot) = runtime.cli(&["workflow", "snapshot"], None);
    assert_eq!(snapshot["data"]["executions"][0]["state"], "working");
}

#[test]
fn workflow_codex_session_aliases_cannot_claim_two_nodes() {
    let runtime = Runtime::new();
    runtime.setup(false);
    let other=runtime.request("r2","roadmap.publish",json!({"id":"r2","projectId":"p","title":"independent","expectedRevision":0,"humanConfirmed":true,"nodes":[{"id":"a","title":"a","goal":"a","acceptance":["a"]}]}));
    assert_eq!(other["ok"], true);
    let uuid = "01a1d47f-6a5f-7622-8df3-48e8b6b70e7c";
    assert_eq!(runtime.request("claim","node.claim",json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":format!("codex:{uuid}")}))["ok"],true);
    let duplicate=runtime.request("alias","node.claim",json!({"roadmapId":"r2","nodeId":"a","expectedRevision":1,"sessionId":format!("codex://threads/{}",uuid.to_ascii_uppercase())}));
    assert_eq!(duplicate["error"]["code"], "already_claimed");
}

#[test]
fn workflow_reflects_after_two_no_progress_results_and_pause_survives_restart() {
    let runtime = Runtime::new();
    runtime.setup(false);
    let claim = runtime.request(
        "claim",
        "node.claim",
        json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":"codex:a"}),
    );
    let progress = json!({"executionId":claim["data"]["executionId"],"token":claim["data"]["token"],"expectedRevision":1,"evidence":[],"summary":"No new evidence"});
    assert_eq!(
        runtime.request("p1", "execution.progress", progress.clone())["data"]["action"],
        "continue"
    );
    let second = runtime.request("p2", "execution.progress", progress.clone());
    assert_eq!(second["data"]["action"], "reflect");
    assert_eq!(
        second["data"]["prompt"],
        "反思一下，方向对了还是错了，继续深挖还是换个思路"
    );
    assert_eq!(
        runtime.request("p3", "execution.progress", progress)["data"]["action"],
        "needs_human"
    );
    assert_eq!(
        runtime.request(
            "pause",
            "control.set",
            json!({"projectId":"p","state":"paused","humanConfirmed":true})
        )["ok"],
        true
    );
    runtime.cli(&["service", "stop"], None);
    let (_, snapshot) = runtime.cli(&["workflow", "snapshot", "--project", "p"], None);
    assert_eq!(snapshot["data"]["projects"][0]["control"], "paused");
}

#[test]
fn workflow_roadmap_changes_freeze_only_the_declared_dependency_closure() {
    let runtime = Runtime::new();
    runtime.setup(false);
    runtime.request(
        "other-project",
        "project.create",
        json!({"id":"other","title":"Other"}),
    );
    let claim = runtime.request(
        "claim",
        "node.claim",
        json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":"codex:a"}),
    );
    let proposal=runtime.request("proposal","roadmap.propose",json!({"roadmapId":"r","expectedRevision":1,"reason":"First assumption failed","affectedNodes":["a"]}));
    assert_eq!(proposal["ok"], true, "{proposal}");
    let (_, snapshot) = runtime.cli(&["workflow", "snapshot"], None);
    assert!(snapshot["data"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["state"] == "frozen"));
    assert_eq!(snapshot["data"]["notifications"][0]["state"], "pending");
    assert_eq!(
        snapshot["data"]["projects"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "other")
            .unwrap()["control"],
        "active"
    );
    let ack=runtime.request("ack","execution.pause_ack",json!({"executionId":claim["data"]["executionId"],"token":claim["data"]["token"],"expectedRevision":1,"summary":"Stopped before next edit"}));
    assert_eq!(ack["ok"], true, "{ack}");
    let denied=runtime.request("new-route","roadmap.publish",json!({"id":"r","projectId":"p","title":"route","expectedRevision":1,"proposalId":proposal["data"]["proposalId"],"nodes":[{"id":"a","title":"Changed","goal":"New goal","acceptance":["New proof"]}]}));
    assert_eq!(denied["error"]["code"], "human_confirmation_required");
}

#[test]
fn workflow_backup_restore_keeps_pauses_and_validates_storage() {
    let runtime = Runtime::new();
    runtime.setup(false);
    let backup = runtime.request("backup", "storage.backup", json!({"humanConfirmed":true}));
    assert_eq!(backup["ok"], true, "{backup}");
    let path = backup["data"]["path"].as_str().unwrap();
    assert_eq!(
        runtime.request("check", "storage.check", json!({}))["data"]["integrity"],
        "ok"
    );
    let denied = runtime
        .cli(&["workflow", "restore", "--file", path, "--confirm"], None)
        .1;
    assert_eq!(denied["error"]["code"], "service_running");
    runtime.cli(&["service", "stop"], None);
    let restored = runtime
        .cli(&["workflow", "restore", "--file", path, "--confirm"], None)
        .1;
    assert_eq!(restored["ok"], true, "{restored}");
    let (_, snapshot) = runtime.cli(&["workflow", "snapshot"], None);
    assert_eq!(snapshot["data"]["projects"][0]["control"], "paused");
}

#[test]
fn workflow_rejects_malformed_payload_and_cyclic_dependencies_without_losing_service() {
    let runtime = Runtime::new();
    let bad = runtime.request("bad", "project.create", Value::Null);
    assert_eq!(bad["error"]["code"], "invalid_request");
    assert_eq!(
        runtime.request("p", "project.create", json!({"id":"p","title":"p"}))["ok"],
        true
    );
    let cycle=runtime.request("cycle","roadmap.publish",json!({"id":"r","projectId":"p","title":"r","expectedRevision":0,"humanConfirmed":true,"nodes":[
        {"id":"a","title":"a","goal":"a","acceptance":["a"],"dependencies":["b"]},
        {"id":"b","title":"b","goal":"b","acceptance":["b"],"dependencies":["a"]}
    ]}));
    assert_eq!(cycle["error"]["code"], "invalid_graph");
    let (_, snapshot) = runtime.cli(&["workflow", "snapshot"], None);
    assert!(snapshot["data"]["roadmaps"].as_array().unwrap().is_empty());
    assert_eq!(
        runtime.request("shell", "shell.run", json!({"program":"cmd.exe"}))["error"]["code"],
        "unknown_workflow_command"
    );
}

#[test]
fn workflow_concurrent_claims_have_one_owner() {
    let runtime = Runtime::new();
    runtime.setup(false);
    let workers=(0..4).map(|i| {
        let home=runtime.home.clone();
        std::thread::spawn(move||{
            let client=Runtime{home};
            let response=client.request(&format!("claim-{i}"),"node.claim",json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":format!("codex:{i}")}));
            std::mem::forget(client);response
        })
    }).collect::<Vec<_>>();
    let results = workers
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        results.iter().filter(|r| r["ok"] == true).count(),
        1,
        "{results:?}"
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| r["error"]["code"] == "already_claimed")
            .count(),
        3
    );
}

#[test]
fn workflow_backup_restores_referenced_artifacts_and_legacy_import_is_explicit() {
    let runtime = Runtime::new();
    runtime.setup(false);
    let artifact = runtime.home.join("proof.txt");
    std::fs::write(&artifact, "proof").unwrap();
    let hash = agentwatcher_lib::workflow::digest(b"proof");
    let claim = runtime.request(
        "claim",
        "node.claim",
        json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":"codex:owner"}),
    );
    let receipt=runtime.request("handoff","execution.handoff",json!({"executionId":claim["data"]["executionId"],"token":claim["data"]["token"],"expectedRevision":1,"summary":"proof","verification":[{"criterion":"Evidence exists","outcome":"passed","evidence":"proof.txt"}],"artifacts":[{"path":artifact,"sha256":hash}]}));
    assert_eq!(receipt["ok"], true, "{receipt}");
    let source = runtime.home.join("legacy.md");
    std::fs::write(
        &source,
        "---\nid: wi_legacy\ntitle: Old task\nstatus: done\n---\n# Old task\n",
    )
    .unwrap();
    let preview = runtime.request("preview", "legacy.preview", json!({"path":source}));
    assert_eq!(preview["ok"], true);
    let imported=runtime.request("import","legacy.import",json!({"projectId":"p","path":source,"expectedDigest":preview["data"]["sha256"],"humanConfirmed":true}));
    assert_eq!(imported["ok"], true, "{imported}");
    assert_eq!(imported["data"]["currentEvidenceAccepted"], false);
    let backup = runtime.request("backup", "storage.backup", json!({"humanConfirmed":true}));
    let path = backup["data"]["path"].as_str().unwrap();
    runtime.cli(&["service", "stop"], None);
    std::fs::remove_file(&artifact).unwrap();
    let restored = runtime
        .cli(&["workflow", "restore", "--file", path, "--confirm"], None)
        .1;
    assert_eq!(restored["ok"], true, "{restored}");
    let check = runtime.request("check", "storage.check", json!({}));
    assert_eq!(check["data"]["artifactErrors"], json!([]), "{check}");
}
