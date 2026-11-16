use agentwatcher_lib::workflow::{codex, digest, store::Store, Request};
use rusqlite::params;
use serde_json::{json, Value};
use std::fs::OpenOptions;

#[test]
fn workflow_codex_lost_ownership_requires_explicit_same_thread_recovery() {
    let home = std::env::temp_dir().join(format!("aw-codex-contract-{}", uuid::Uuid::new_v4()));
    let mut store = Store::open(&home).unwrap();
    let mut count = 0;
    let mut request = |command: &str, payload: Value| {
        count += 1;
        Request {
            schema_version: 1,
            request_id: format!("r{count}"),
            command: command.into(),
            payload,
        }
    };
    store
        .apply(&request("project.create", json!({"id":"p","title":"p"})))
        .unwrap();
    store.apply(&request("roadmap.publish",json!({"id":"r","projectId":"p","title":"r","expectedRevision":0,"humanConfirmed":true,"nodes":[{"id":"a","title":"a","goal":"a","acceptance":["a"]}]}))).unwrap();
    let claim=store.apply(&request("node.claim",json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":"codex:original","workspace":home}))).unwrap();
    let operation = "fixture-operation";
    let job = json!({"claim":claim,"roadmapId":"r","nodeId":"a","expectedRevision":1,"workspace":home,"sandbox":"read-only"});
    store
        .connection
        .execute(
            "INSERT INTO operations VALUES(?1,'codex.run','running',?2,?3)",
            params![
                operation,
                job.to_string(),
                json!({"startedMs":1,"pid":0}).to_string()
            ],
        )
        .unwrap();
    store
        .connection
        .execute(
            "INSERT INTO runner_state VALUES(?1,?2,?3)",
            params![
                operation,
                claim["executionId"].as_str(),
                json!({"state":"running","threadId":"original"}).to_string()
            ],
        )
        .unwrap();
    let directory = home.join("runners").join(digest(operation.as_bytes()));
    std::fs::create_dir_all(&directory).unwrap();
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("runner.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    codex::reconcile(&mut store).unwrap();
    assert_eq!(
        store.snapshot(None).unwrap()["executions"][0]["state"],
        "working",
        "live owner must be retained"
    );
    drop(lock);
    codex::reconcile(&mut store).unwrap();
    let snapshot = store.snapshot(None).unwrap();
    assert_eq!(snapshot["executions"][0]["state"], "needs_human");
    assert_eq!(snapshot["operations"][0]["state"], "needs_reconciliation");
    assert_eq!(
        store
            .apply(&request(
                "runner.recover",
                json!({"operationId":operation,"reason":"inspected"})
            ))
            .unwrap_err()
            .code,
        "human_confirmation_required"
    );
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(directory.join("runner.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert_eq!(
        store
            .apply(&request(
                "runner.recover",
                json!({"operationId":operation,"reason":"inspected","humanConfirmed":true})
            ))
            .unwrap_err()
            .code,
        "runner_already_active"
    );
    drop(lock);
    let recovered=store.apply(&request("runner.recover",json!({"operationId":operation,"reason":"inspected original thread and worktree","humanConfirmed":true}))).unwrap();
    assert_eq!(recovered["threadId"], "original");
    assert_eq!(recovered["newThread"], false);
    let body: String = store
        .connection
        .query_row(
            "SELECT body FROM operations WHERE id=?1",
            [operation],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&body).unwrap()["resumeThreadId"],
        "original"
    );
    assert_eq!(
        store.snapshot(None).unwrap()["executions"][0]["sessionId"],
        "codex:original"
    );
    drop(store);
    std::fs::remove_dir_all(home).unwrap();
}
