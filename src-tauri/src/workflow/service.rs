use super::store::Store;
use super::{id, Error, Request, Result, PROTOCOL};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const LIMIT: usize = 4 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Card {
    port: u16,
    token: String,
    pid: u32,
    protocol: u32,
}

fn card(home: &Path) -> Result<Card> {
    serde_json::from_slice(&std::fs::read(home.join("service.json"))?)
        .map_err(|e| Error::new("service_unavailable", e.to_string()))
}
fn send(card: &Card, mut message: Value) -> Result<Value> {
    let mut stream =
        TcpStream::connect_timeout(&([127, 0, 0, 1], card.port).into(), Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    stream.set_write_timeout(Some(Duration::from_secs(10)))?;
    message["token"] = Value::String(card.token.clone());
    let mut bytes =
        serde_json::to_vec(&message).map_err(|e| Error::new("invalid_request", e.to_string()))?;
    if bytes.len() > LIMIT {
        return Err(Error::new("request_too_large", "Request exceeds 4 MiB"));
    }
    bytes.push(b'\n');
    stream.write_all(&bytes)?;
    let mut line = String::new();
    BufReader::new(stream)
        .take((LIMIT + 1) as u64)
        .read_line(&mut line)?;
    if line.len() > LIMIT {
        return Err(Error::new(
            "response_too_large",
            "Service response exceeds 4 MiB",
        ));
    }
    let response: Value = serde_json::from_str(&line)
        .map_err(|e| Error::new("service_protocol_error", e.to_string()))?;
    if response["ok"] != true {
        return Err(serde_json::from_value(response["error"].clone())
            .unwrap_or_else(|_| Error::new("service_protocol_error", "Invalid service error")));
    }
    Ok(response["data"].clone())
}
pub fn status(home: &Path) -> Value {
    card(home)
        .and_then(|card| {
            if card.protocol != PROTOCOL {
                return Err(Error::new("protocol_mismatch", "Service protocol differs"));
            }
            let value = send(&card, json!({"method":"health"}))?;
            if value["pid"] != card.pid {
                return Err(Error::new(
                    "service_identity_mismatch",
                    "Service PID differs",
                ));
            }
            Ok(value)
        })
        .unwrap_or_else(|_| json!({"running":false,"protocol":PROTOCOL}))
}
fn lock(path: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    let start = Instant::now();
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(_) if start.elapsed() < Duration::from_secs(15) => {
                std::thread::sleep(Duration::from_millis(40))
            }
            Err(e) => return Err(Error::new("service_busy", e.to_string())),
        }
    }
}
pub fn executable() -> Result<PathBuf> {
    let current = std::env::current_exe()?;
    if current.file_stem().and_then(|n| n.to_str()) == Some("agentwatcher-cli") {
        return Ok(current);
    }
    let sibling = current.with_file_name(if cfg!(windows) {
        "agentwatcher-cli.exe"
    } else {
        "agentwatcher-cli"
    });
    if sibling.is_file() {
        Ok(sibling)
    } else {
        Err(Error::new(
            "cli_unavailable",
            "AgentWatcher CLI must be installed alongside the desktop executable",
        ))
    }
}
pub fn ensure(home: &Path) -> Result<Value> {
    let existing = status(home);
    if existing["running"] == true {
        return Ok(existing);
    }
    std::fs::create_dir_all(home)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(home, std::fs::Permissions::from_mode(0o700))?;
    }
    let _guard = lock(&home.join("startup.lock"))?;
    let existing = status(home);
    if existing["running"] == true {
        return Ok(existing);
    }
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join("service.log"))?;
    // Run an immutable copy, so a live service does not lock the development or release CLI binary.
    let source = executable()?;
    let bytes = std::fs::read(&source)?;
    let runtime = home.join("runtime").join(super::digest(&bytes));
    std::fs::create_dir_all(&runtime)?;
    let binary = runtime.join(if cfg!(windows) {
        "agentwatcher-cli.exe"
    } else {
        "agentwatcher-cli"
    });
    if !binary.exists() {
        std::fs::write(&binary, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700))?;
        }
    }
    let mut command = Command::new(binary);
    command
        .arg("--workflow-home")
        .arg(home)
        .args(["service", "run"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(log));
    detached(&mut command);
    let mut child = command.spawn()?;
    let start = Instant::now();
    loop {
        let status = status(home);
        if status["running"] == true {
            return Ok(status);
        }
        if let Some(exit) = child.try_wait()? {
            return Err(Error::new(
                "service_start_failed",
                format!(
                    "Service exited {exit}; see {}",
                    home.join("service.log").display()
                ),
            ));
        }
        if start.elapsed() > Duration::from_secs(15) {
            return Err(Error::new(
                "service_start_timeout",
                "Service did not become ready; no workflow request was sent",
            ));
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}
pub fn detached(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
}
pub fn stop(home: &Path) -> Result<Value> {
    if status(home)["running"] != true {
        return Ok(json!({"running":false}));
    }
    let result = send(&card(home)?, json!({"method":"stop"}))?;
    let start = Instant::now();
    while status(home)["running"] == true && start.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(30));
    }
    Ok(result)
}
pub fn apply(home: &Path, request: &Request) -> Result<Value> {
    ensure(home)?;
    send(&card(home)?, json!({"method":"apply","request":request}))
}
pub fn snapshot(home: &Path, project: Option<&str>) -> Result<Value> {
    ensure(home)?;
    send(&card(home)?, json!({"method":"snapshot","project":project}))
}
pub fn events(
    home: &Path,
    project: Option<&str>,
    after: u64,
    limit: usize,
    wait_ms: u64,
) -> Result<Value> {
    ensure(home)?;
    send(
        &card(home)?,
        json!({"method":"events","project":project,"after":after,"limit":limit,"waitMs":wait_ms.min(30_000)}),
    )
}
pub fn next(home: &Path, project: Option<&str>) -> Result<Value> {
    ensure(home)?;
    send(&card(home)?, json!({"method":"next","project":project}))
}
pub fn restore(home: &Path, backup: &Path, human_confirmed: bool) -> Result<Value> {
    if !human_confirmed {
        return Err(Error::new(
            "human_confirmation_required",
            "Restore requires explicit confirmation",
        ));
    }
    std::fs::create_dir_all(home)?;
    let _startup = lock(&home.join("startup.lock"))?;
    if status(home)["running"] == true {
        return Err(Error::new(
            "service_running",
            "Stop the service before restoring a database",
        ));
    }
    let _owner = lock(&home.join("service.lock"))?;
    let current_path = home.join("workflow.sqlite3");
    if current_path.exists() {
        let current = rusqlite::Connection::open_with_flags(
            &current_path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let active: u64 = current.query_row(
            "SELECT count(*) FROM executions WHERE state IN ('working','pause_requested')",
            [],
            |r| r.get(0),
        )?;
        if active > 0 {
            return Err(Error::new(
                "active_executions",
                "Pause and obtain safe-stop receipts before restoring over active executions",
            ));
        }
        let mut stmt = current.prepare("SELECT id FROM operations WHERE kind='codex.run'")?;
        for operation in stmt.query_map([], |r| r.get::<_, String>(0))? {
            let path = home
                .join("runners")
                .join(super::digest(operation?.as_bytes()))
                .join("runner.lock");
            if path.exists() {
                let lock = OpenOptions::new().read(true).write(true).open(path)?;
                lock.try_lock().map_err(|_|Error::new("active_runner","A runner still owns the original execution; restore into a separate empty home or reconcile first"))?;
            }
        }
    }
    let (backup, artifacts) = super::backup::unpack(home, backup)?;
    let source =
        rusqlite::Connection::open_with_flags(&backup, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let integrity: String = source.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
    let version: u32 = source.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if integrity != "ok" || version != 1 {
        return Err(Error::new(
            "invalid_backup",
            "Backup integrity or schema is unsupported",
        ));
    }
    let temp = home.join(format!("restore-{}.sqlite3", id("copy")));
    source.execute("VACUUM INTO ?1", [temp.to_string_lossy().as_ref()])?;
    drop(source);
    let restored = rusqlite::Connection::open(&temp)?;
    restored.execute_batch("BEGIN IMMEDIATE; UPDATE projects SET control='paused' WHERE control='active'; UPDATE executions SET state='needs_human' WHERE state IN ('working','submitted','pause_requested','paused'); UPDATE operations SET state='needs_reconciliation' WHERE state IN ('queued','launching','running','pending'); COMMIT;")?;
    let handoffs = {
        let mut stmt = restored.prepare("SELECT id,body FROM handoffs")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows
    };
    for (id, body) in handoffs {
        let mut body: Value =
            serde_json::from_str(&body).map_err(|e| Error::new("invalid_backup", e.to_string()))?;
        for artifact in body["artifacts"].as_array_mut().into_iter().flatten() {
            if let Some(found) = artifacts
                .as_array()
                .into_iter()
                .flatten()
                .find(|entry| entry["sha256"] == artifact["sha256"])
            {
                artifact["restoredPath"] = found["path"].clone();
            }
        }
        restored.execute(
            "UPDATE handoffs SET body=?1,digest=?2 WHERE id=?3",
            rusqlite::params![
                body.to_string(),
                super::digest(body.to_string().as_bytes()),
                id
            ],
        )?;
    }
    drop(restored);
    let retained = home.join("pre-restore").join(id("snapshot"));
    std::fs::create_dir_all(&retained)?;
    for name in [
        "workflow.sqlite3",
        "workflow.sqlite3-wal",
        "workflow.sqlite3-shm",
    ] {
        let path = home.join(name);
        if path.exists() {
            std::fs::rename(path, retained.join(name))?;
        }
    }
    std::fs::rename(temp, home.join("workflow.sqlite3"))?;
    Ok(
        json!({"restored":true,"control":"paused","previousDatabase":retained,"executionsRequireReconciliation":true}),
    )
}

pub fn serve(home: &Path) -> Result<()> {
    std::fs::create_dir_all(home)?;
    let _owner = lock(&home.join("service.lock"))?;
    let mut store = Store::open(home)?;
    super::codex::reconcile(&mut store)?;
    let listener = TcpListener::bind(("127.0.0.1", 0))?;
    let card = Card {
        port: listener.local_addr()?.port(),
        token: id("service"),
        pid: std::process::id(),
        protocol: PROTOCOL,
    };
    let path = home.join("service.json");
    let temp = home.join("service.new");
    std::fs::write(&temp, serde_json::to_vec(&card).unwrap())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600))?;
    }
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    std::fs::rename(temp, &path)?;
    if let Err(error) = store
        .schedule_authorized()
        .and_then(|_| super::codex::dispatch(&mut store))
    {
        eprintln!("startup reconciliation: {}: {}", error.code, error.message);
    }
    let mut stopping = false;
    for stream in listener.incoming() {
        let mut stream = stream?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(10)))?;
        let response = (|| -> Result<Value> {
            let mut line = String::new();
            BufReader::new(stream.try_clone()?)
                .take((LIMIT + 1) as u64)
                .read_line(&mut line)?;
            if line.len() > LIMIT {
                return Err(Error::new("request_too_large", "Request exceeds 4 MiB"));
            }
            let message: Value = serde_json::from_str(&line)
                .map_err(|e| Error::new("invalid_request", e.to_string()))?;
            if message["token"].as_str() != Some(&card.token) {
                return Err(Error::new(
                    "unauthorized",
                    "Service identity token is required",
                ));
            }
            match message["method"].as_str() {
                Some("health") => Ok(json!({"running":true,"pid":card.pid,"protocol":PROTOCOL})),
                Some("stop") => {
                    stopping = true;
                    Ok(json!({"running":false,"workersStopped":false}))
                }
                Some("snapshot") => {
                    super::codex::reconcile(&mut store)?;
                    store.snapshot(message["project"].as_str())
                }
                Some("events") => {
                    super::codex::reconcile(&mut store)?;
                    let project = message["project"].as_str();
                    let after = message["after"].as_u64().unwrap_or(0);
                    let limit = message["limit"].as_u64().unwrap_or(100) as usize;
                    let wait = message["waitMs"].as_u64().unwrap_or(0).min(30_000);
                    let deadline = Instant::now() + Duration::from_millis(wait);
                    loop {
                        let result = store.events_after(project, after, limit)?;
                        if result["events"]
                            .as_array()
                            .is_some_and(|items| !items.is_empty())
                            || wait == 0
                            || Instant::now() >= deadline
                        {
                            break Ok(result);
                        }
                        std::thread::sleep(Duration::from_millis(200));
                    }
                }
                Some("next") => {
                    super::codex::reconcile(&mut store)?;
                    store.next_actions(message["project"].as_str())
                }
                Some("apply") => {
                    let request: Request = serde_json::from_value(message["request"].clone())
                        .map_err(|e| Error::new("invalid_request", e.to_string()))?;
                    let mut result = store.apply(&request)?;
                    store.integrate_accepted(&request, &mut result)?;
                    if request.command == "handoff.accept" {
                        store.connection.execute(
                            "UPDATE requests SET result=?1 WHERE id=?2",
                            rusqlite::params![result.to_string(), request.request_id],
                        )?;
                    }
                    store.schedule_authorized()?;
                    super::codex::dispatch(&mut store)?;
                    Ok(result)
                }
                _ => Err(Error::new("unknown_service_method", "Unsupported method")),
            }
        })();
        let envelope = match response {
            Ok(data) => json!({"ok":true,"data":data}),
            Err(error) => json!({"ok":false,"error":error}),
        };
        let mut bytes = envelope.to_string().into_bytes();
        bytes.push(b'\n');
        let _ = stream.write_all(&bytes);
        if stopping {
            break;
        }
    }
    // Remove only the card owned by this authenticated server instance.
    if self::card(home).is_ok_and(|c| c.token == card.token) {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}
