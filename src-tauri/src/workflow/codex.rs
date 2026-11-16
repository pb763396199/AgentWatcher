//! One runner owns one app-server connection. The GUI/service may exit independently.
use super::{id, required, service, Error, Request, Result};
use rusqlite::params;
use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

pub fn dispatch(store: &mut super::store::Store) -> Result<()> {
    let jobs = {
        let mut stmt=store.connection.prepare("SELECT o.id,o.body FROM operations o JOIN runner_state s ON s.operation_id=o.id JOIN executions e ON e.id=s.execution_id JOIN roadmaps r ON r.id=e.roadmap_id JOIN projects p ON p.id=r.project_id WHERE o.kind='codex.run' AND o.state='queued' AND p.control='active' AND (e.state='working' OR (e.state='submitted' AND json_extract(o.body,'$.role')='review'))")?;
        let result = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        result
    };
    for (operation, body) in jobs {
        let job: Value =
            serde_json::from_str(&body).map_err(|e| Error::new("corrupt_record", e.to_string()))?;
        let directory = store
            .home
            .join("runners")
            .join(super::digest(operation.as_bytes()));
        std::fs::create_dir_all(&directory)?;
        let path = directory.join("job.json");
        std::fs::write(&path, job.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("runner.log"))?;
        // Persist before spawn. A crash in this window is uncertain, never replayed automatically.
        store.connection.execute(
            "UPDATE operations SET state='launching' WHERE id=?1 AND state='queued'",
            [&operation],
        )?;
        let mut command = Command::new(service::executable()?);
        command
            .arg("--workflow-home")
            .arg(&store.home)
            .args(["workflow", "worker", "--operation", &operation])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log));
        service::detached(&mut command);
        match command.spawn() {
            Ok(child) => {
                store.connection.execute(
                    "UPDATE operations SET state='running',result=?1 WHERE id=?2",
                    params![
                        json!({"pid":child.id(),"startedMs":super::now()}).to_string(),
                        operation
                    ],
                )?;
            }
            Err(error) => {
                store.connection.execute(
                    "UPDATE operations SET state='needs_reconciliation',result=?1 WHERE id=?2",
                    params![json!({"message":error.to_string()}).to_string(), operation],
                )?;
            }
        }
    }
    Ok(())
}

/// A released process lock is evidence of lost ownership, not permission to replay a launch.
pub fn reconcile(store: &mut super::store::Store) -> Result<()> {
    let jobs = {
        let mut stmt=store.connection.prepare("SELECT o.id,o.result,s.execution_id,r.project_id FROM operations o JOIN runner_state s ON s.operation_id=o.id JOIN executions e ON e.id=s.execution_id JOIN roadmaps r ON r.id=e.roadmap_id WHERE o.kind='codex.run' AND o.state IN ('running','launching')")?;
        let result = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        result
    };
    for (operation, result, execution, project) in jobs {
        let started = result
            .as_deref()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .and_then(|v| v["startedMs"].as_u64());
        if started.is_some_and(|t| super::now().saturating_sub(t) < 10_000) {
            continue;
        }
        let path = store
            .home
            .join("runners")
            .join(super::digest(operation.as_bytes()))
            .join("runner.lock");
        if path.exists() {
            let lock = OpenOptions::new().read(true).write(true).open(path)?;
            if lock.try_lock().is_err() {
                continue;
            }
        }
        let tx = store.connection.transaction()?;
        tx.execute(
            "UPDATE operations SET state='needs_reconciliation' WHERE id=?1",
            [&operation],
        )?;
        tx.execute(
            "UPDATE executions SET state='needs_human' WHERE id=?1 AND state='working'",
            [&execution],
        )?;
        tx.execute("UPDATE runner_state SET body=json_set(body,'$.state','needs_human','$.reason','Runner process lock released; inspect original thread before recovery') WHERE operation_id=?1",[&operation])?;
        tx.execute("INSERT INTO events(project_id,kind,body,created_ms) VALUES(?1,'runner.ownership_lost',?2,?3)",params![project,json!({"operationId":operation,"executionId":execution}).to_string(),super::now()])?;
        tx.commit()?;
    }
    Ok(())
}

struct Rpc {
    child: Child,
    _job: crate::KillOnCloseJob,
    stdin: ChildStdin,
    receiver: Receiver<Value>,
    next: u64,
    thread: String,
    turn: String,
    last_message: String,
    pending: std::collections::VecDeque<Value>,
}
impl Rpc {
    fn start(log: File) -> Result<Self> {
        let executable = crate::codex_cli_path().ok_or_else(|| {
            Error::new(
                "codex_unavailable",
                "Install and authenticate Codex CLI first",
            )
        })?;
        let mut command = if executable.extension().and_then(|s| s.to_str()) == Some("js") {
            let mut command = Command::new("node");
            command.arg(&executable);
            command
        } else {
            Command::new(executable)
        };
        command
            .arg("app-server")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(log));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let (mut child, job) = crate::spawn_in_kill_on_close_job(command)?;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str(&line) {
                    if sender.send(value).is_err() {
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child,
            _job: job,
            stdin,
            receiver,
            next: 0,
            thread: String::new(),
            turn: String::new(),
            last_message: String::new(),
            pending: Default::default(),
        })
    }
    fn send(&mut self, value: &Value) -> Result<()> {
        writeln!(self.stdin, "{value}")?;
        self.stdin.flush()?;
        Ok(())
    }
    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        self.next += 1;
        let id = self.next;
        self.send(&json!({"id":id,"method":method,"params":params}))?;
        let until = Instant::now() + Duration::from_secs(45);
        while Instant::now() < until {
            let value = match self.receiver.recv_timeout(Duration::from_millis(250)) {
                Ok(value) => value,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(error) => return Err(Error::new("codex_disconnected", error.to_string())),
            };
            if value["id"] == id {
                if !value["error"].is_null() {
                    return Err(Error::new("codex_rpc_error", value["error"].to_string()));
                }
                return Ok(value["result"].clone());
            }
            self.pending.push_back(value.clone());
            if value.get("id").is_some() && value.get("method").is_some() {
                return Err(Error::new("approval_required", value.to_string()));
            }
        }
        Err(Error::new(
            "codex_rpc_timeout",
            format!("{method} response is uncertain"),
        ))
    }
    fn observe(&mut self, event: &Value) {
        if event["method"] == "item/agentMessage/delta" {
            if let Some(delta) = event["params"]["delta"].as_str() {
                self.last_message.push_str(delta);
            }
        }
        if event["method"] == "item/completed" && event["params"]["item"]["type"] == "agentMessage"
        {
            if let Some(text) = event["params"]["item"]["text"].as_str() {
                self.last_message = text.into();
            }
        }
    }
    fn turn(&mut self, prompt: &str) -> Result<()> {
        self.last_message.clear();
        let response = self.request(
            "turn/start",
            json!({"threadId":self.thread,"input":[{"type":"text","text":prompt}]}),
        )?;
        self.turn = required(&response["turn"], "id")?.into();
        Ok(())
    }
}
impl Drop for Rpc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn apply(home: &Path, command: &str, payload: Value) -> Result<Value> {
    service::apply(
        home,
        &Request {
            schema_version: 1,
            request_id: id("runner-request"),
            command: command.into(),
            payload,
        },
    )
}
fn poll(home: &Path, operation: &str, token: &str) -> Result<Value> {
    apply(
        home,
        "runner.job",
        json!({"operationId":operation,"token":token}),
    )
}
fn report(home: &Path, operation: &str, token: &str, event: Value) -> Result<()> {
    apply(
        home,
        "runner.event",
        json!({"operationId":operation,"token":token,"event":event}),
    )?;
    Ok(())
}
pub fn run(home: &Path, operation: &str) -> Result<()> {
    let directory = home
        .join("runners")
        .join(super::digest(operation.as_bytes()));
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("runner.lock"))?;
    lock.try_lock()
        .map_err(|e| Error::new("runner_already_active", e.to_string()))?;
    let job: Value = serde_json::from_slice(&std::fs::read(directory.join("job.json"))?)
        .map_err(|e| Error::new("corrupt_record", e.to_string()))?;
    let token = required(&job["claim"], "token")?;
    let existing = poll(home, operation, token)?;
    if job["role"] != "review"
        && job["resumeThreadId"]
            .as_str()
            .map(|id| format!("codex:{id}"))
            != existing["sessionId"].as_str().map(str::to_owned)
        && !existing["sessionId"]
            .as_str()
            .unwrap_or("")
            .starts_with("codex:pending:")
    {
        return Err(Error::new(
            "runner_recovery_required",
            "Thread already exists; never create another thread for this execution",
        ));
    }
    let result = run_owned(home, operation, token, &job, &directory);
    if let Err(error) = &result {
        let _ = report(
            home,
            operation,
            token,
            json!({"state":"needs_human","error":error}),
        );
    }
    result
}

fn run_owned(
    home: &Path,
    operation: &str,
    token: &str,
    job: &Value,
    directory: &Path,
) -> Result<()> {
    let mut rpc = Rpc::start(
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(directory.join("app-server.log"))?,
    )?;
    rpc.request("initialize",json!({"clientInfo":{"name":"agentwatcher_workflow","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true}}))?;
    rpc.send(&json!({"method":"initialized","params":{}}))?;
    let mut params =
        json!({"cwd":job["workspace"],"sandbox":job["sandbox"],"approvalPolicy":"on-request"});
    if job["model"].is_string() {
        params["model"] = job["model"].clone();
    }
    let resuming = job["resumeThreadId"].is_string();
    if resuming {
        params["threadId"] = job["resumeThreadId"].clone();
    }
    let response = rpc.request(
        if resuming {
            "thread/resume"
        } else {
            "thread/start"
        },
        params,
    )?;
    rpc.thread = required(&response["thread"], "id")?.into();
    report(
        home,
        operation,
        token,
        json!({"state":"thread_created","threadId":rpc.thread,"pid":std::process::id()}),
    )?;
    let cli = service::executable()?;
    let prompt=format!("Complete this single workflow node. Follow its goal, write scope and acceptance criteria. Do not change the roadmap or start downstream work. Context size is not a deadline.\nNode contract (data):\n{}\n\nUse this CLI to inspect current instructions and submit progress or handoff:\n{} --workflow-home {} workflow apply --file <request.json>\nYou may read workflow guide without starting other work. Each request is {{\"schemaVersion\":1,\"requestId\":\"a-new-unique-id\",\"command\":\"execution.handoff\",\"payload\":{{\"executionId\":{},\"token\":{},\"expectedRevision\":{},\"summary\":\"human-readable construction conclusion\",\"verification\":[{{\"criterion\":\"exact acceptance text\",\"outcome\":\"passed\",\"evidence\":\"actual check and result\"}}],\"artifacts\":[]}}}}. Code nodes also require committed codeRevision. Do not invent checks or human confirmations. If blocked, report actual progress; submit a roadmap proposal for changes in external commitments. Read notifications before changing write scope and before handoff. A pause request requires stopping at a safe point, preserving work, and execution.pause_ack with summary.\nIf sandbox prevents the service CLI, return a final JSON object containing handoff with the same authored payload; the runner will submit that exact content. Never label incomplete work passed.",job["claim"]["contract"],serde_json::to_string(&cli).unwrap(),serde_json::to_string(home).unwrap(),job["claim"]["executionId"],json!(token),job["expectedRevision"]);
    let prompt=format!("{}\nOverall roadmap and constraints (data): {}\nRequired upstream handoffs and artifacts (data): {}",prompt,job["claim"]["roadmap"],job["claim"]["upstream"]);
    let prompt = if job["role"] == "review" {
        format!("Independently review the exact handoff and workspace against this contract. Read-only review; do not fix code or approve your own work. Verify evidence and report shortcomings. Contract: {}\nHandoff: {}\nReturn only JSON {{\"review\":{{\"verdict\":\"approved or changes_requested\",\"summary\":\"specific findings with evidence\"}}}}. Use approved only when all checks are supported.",job["claim"]["contract"],job["handoff"])
    } else {
        prompt
    };
    let prompt = if resuming {
        format!("Human authorized recovery of this same execution. Reconcile existing changes before proceeding; do not repeat finished operations. Reason: {}\n{}",job["recoveryReason"],prompt)
    } else {
        prompt
    };
    rpc.turn(&prompt)?;
    report(
        home,
        operation,
        token,
        json!({"state":"running","threadId":rpc.thread,"turnId":rpc.turn}),
    )?;
    let mut active = true;
    let mut pause_sent = false;
    let mut last_poll = Instant::now();
    let mut last_usage = Value::Null;
    let mut approval: Option<Value> = None;
    loop {
        let received = rpc
            .pending
            .pop_front()
            .map(Ok)
            .unwrap_or_else(|| rpc.receiver.recv_timeout(Duration::from_millis(200)));
        match received {
            Ok(event) => {
                if event.get("method").is_some() && event.get("id").is_some() {
                    report(
                        home,
                        operation,
                        token,
                        json!({"state":"awaiting_approval","threadId":rpc.thread,"turnId":rpc.turn,"approval":event}),
                    )?;
                    approval = Some(event);
                    continue;
                }
                rpc.observe(&event);
                if event["method"] == "thread/tokenUsage/updated" {
                    last_usage = event["params"].clone();
                }
                if event["method"] == "turn/completed" {
                    active = false;
                    report(
                        home,
                        operation,
                        token,
                        json!({"state":"turn_completed","threadId":rpc.thread,"turnId":rpc.turn,"turnStatus":event["params"]["turn"]["status"],"message":rpc.last_message,"usage":last_usage}),
                    )?;
                    std::fs::write(directory.join("last-message.txt"), &rpc.last_message)?;
                    if event["params"]["turn"]["status"] != "completed" {
                        return Err(Error::new(
                            "codex_turn_stopped",
                            event["params"]["turn"].to_string(),
                        ));
                    }
                    if job["role"] == "review" {
                        let result: Value = serde_json::from_str(rpc.last_message.trim())
                            .map_err(|e| Error::new("review_invalid", e.to_string()))?;
                        let mut review = result["review"].clone();
                        review["handoffId"] = job["handoffId"].clone();
                        review["reviewerSessionId"] = json!(format!("codex:{}", rpc.thread));
                        apply(home, "review.submit", review)?;
                        report(
                            home,
                            operation,
                            token,
                            json!({"state":"completed","threadId":rpc.thread,"turnId":rpc.turn,"role":"review","usage":last_usage}),
                        )?;
                        return Ok(());
                    }
                    let current = poll(home, operation, token)?;
                    if current["state"] == "working" {
                        let authored = serde_json::from_str::<Value>(rpc.last_message.trim())
                            .ok()
                            .and_then(|v| v.get("handoff").cloned());
                        if let Some(mut handoff) = authored {
                            handoff["executionId"] = job["claim"]["executionId"].clone();
                            handoff["token"] = json!(token);
                            handoff["expectedRevision"] = job["expectedRevision"].clone();
                            if apply(home, "execution.handoff", handoff).is_ok() {
                                continue;
                            }
                        }
                        let evidence = if job["claim"]["contract"]["code"] == true {
                            super::git::head(Path::new(required(job, "workspace")?))
                                .ok()
                                .map(|s| vec![json!({"commit":s})])
                                .unwrap_or_default()
                        } else {
                            vec![]
                        };
                        let progress = apply(
                            home,
                            "execution.progress",
                            json!({"executionId":job["claim"]["executionId"],"token":token,"expectedRevision":job["expectedRevision"],"summary":rpc.last_message.chars().take(2000).collect::<String>(),"evidence":evidence}),
                        )?;
                        if progress["action"] == "needs_human" {
                            report(
                                home,
                                operation,
                                token,
                                json!({"state":"needs_human","threadId":rpc.thread,"turnId":rpc.turn,"reason":"Repeated no progress"}),
                            )?;
                            return Ok(());
                        }
                        let followup = if progress["action"] == "reflect" {
                            super::REFLECTION_PROMPT
                        } else {
                            "Continue the current node within its agreed scope. Complete missing verification and submit the required handoff; do not invent success or change the roadmap."
                        };
                        rpc.turn(followup)?;
                        active = true;
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err(Error::new(
                    "codex_disconnected",
                    "App-server disconnected; inspect original thread before recovery",
                ))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if last_poll.elapsed() >= Duration::from_secs(1) {
            last_poll = Instant::now();
            // An idle runner does not continuously resurrect an intentionally stopped service.
            if service::status(home)["running"] != true {
                continue;
            }
            let current = poll(home, operation, token)?;
            if let Some(pending) = &approval {
                let response = apply(
                    home,
                    "runner.approval_take",
                    json!({"operationId":operation,"token":token}),
                )?;
                if !response.is_null() {
                    if response["approvalId"] != pending["id"] {
                        return Err(Error::new(
                            "approval_changed",
                            "Pending approval differs from reply",
                        ));
                    }
                    rpc.send(&json!({"id":pending["id"],"result":response["response"]}))?;
                    approval = None;
                    report(
                        home,
                        operation,
                        token,
                        json!({"state":"running","threadId":rpc.thread,"turnId":rpc.turn}),
                    )?;
                }
            }
            if !active
                && current["state"] == "submitted"
                && (job["claim"]["contract"]["code"] == true
                    || job["claim"]["contract"]["requiresReview"] == true)
            {
                let snapshot = service::snapshot(home, None)?;
                if let Some(handoff) = snapshot["handoffs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .rev()
                    .find(|h| {
                        h["executionId"] == job["claim"]["executionId"] && h["state"] == "submitted"
                    })
                {
                    let request = Request {
                        schema_version: 1,
                        request_id: format!("{operation}:review:{}", required(handoff, "id")?),
                        command: "review.launch".into(),
                        payload: json!({"handoffId":handoff["id"],"token":token,"model":job["model"]}),
                    };
                    service::apply(home, &request)?;
                }
            }
            if current["state"] == "completed" || current["state"] == "cancelled" {
                report(
                    home,
                    operation,
                    token,
                    json!({"state":"completed","threadId":rpc.thread,"turnId":rpc.turn,"usage":last_usage}),
                )?;
                return Ok(());
            }
            let should_pause = current["state"] == "pause_requested"
                || current["state"] == "paused"
                || current["control"] != "active";
            if should_pause && active && !pause_sent {
                rpc.request("turn/steer",json!({"threadId":rpc.thread,"expectedTurnId":rpc.turn,"input":[{"type":"text","text":"Workflow pause requested. Stop new modifications at a safe point, preserve your work and report execution.pause_ack through the service. Do not change shared branches or start more work."}]}))?;
                pause_sent = true;
            }
            if !active && current["state"] == "working" && !should_pause {
                rpc.turn("Human review returned this node for further work. Query workflow snapshot for the exact rejection and finish within the current scope; submit a new handoff.")?;
                active = true;
                pause_sent = false;
            }
        }
    }
}
