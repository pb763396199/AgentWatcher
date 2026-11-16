use super::{digest, human, id, now, required, Error, Request, Result, REFLECTION_PROMPT};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct Store {
    pub connection: Connection,
    pub home: PathBuf,
}

impl Store {
    pub fn open(home: &Path) -> Result<Self> {
        std::fs::create_dir_all(home)?;
        let connection = Connection::open(home.join("workflow.sqlite3"))?;
        connection.busy_timeout(Duration::from_secs(10))?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")?;
        let version: u32 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version > 1 {
            return Err(Error::new(
                "schema_too_new",
                "Database requires a newer AgentWatcher",
            ));
        }
        connection.execute_batch("BEGIN IMMEDIATE;
            CREATE TABLE IF NOT EXISTS projects(id TEXT PRIMARY KEY,title TEXT NOT NULL,control TEXT NOT NULL DEFAULT 'active');
            CREATE TABLE IF NOT EXISTS roadmaps(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),title TEXT NOT NULL,revision INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS revisions(roadmap_id TEXT NOT NULL REFERENCES roadmaps(id),revision INTEGER NOT NULL,body TEXT NOT NULL,PRIMARY KEY(roadmap_id,revision));
            CREATE TABLE IF NOT EXISTS nodes(roadmap_id TEXT NOT NULL REFERENCES roadmaps(id),id TEXT NOT NULL,title TEXT NOT NULL,contract TEXT NOT NULL,digest TEXT NOT NULL,state TEXT NOT NULL,PRIMARY KEY(roadmap_id,id));
            CREATE TABLE IF NOT EXISTS executions(id TEXT PRIMARY KEY,roadmap_id TEXT NOT NULL,node_id TEXT NOT NULL,revision INTEGER NOT NULL,contract_digest TEXT NOT NULL,session_id TEXT NOT NULL,token_digest TEXT NOT NULL,state TEXT NOT NULL,no_progress INTEGER NOT NULL DEFAULT 0,evidence TEXT NOT NULL DEFAULT '[]',workspace TEXT,FOREIGN KEY(roadmap_id,node_id) REFERENCES nodes(roadmap_id,id));
            CREATE UNIQUE INDEX IF NOT EXISTS one_writer ON executions(roadmap_id,node_id) WHERE state IN ('working','needs_human','pause_requested','paused','submitted');
            CREATE UNIQUE INDEX IF NOT EXISTS one_active_node_per_session ON executions(session_id) WHERE state IN ('working','needs_human','pause_requested','paused','submitted');
            CREATE TABLE IF NOT EXISTS handoffs(id TEXT PRIMARY KEY,execution_id TEXT NOT NULL REFERENCES executions(id),body TEXT NOT NULL,digest TEXT NOT NULL,state TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS reviews(id TEXT PRIMARY KEY,handoff_id TEXT NOT NULL REFERENCES handoffs(id),reviewer TEXT NOT NULL,verdict TEXT NOT NULL,body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS proposals(id TEXT PRIMARY KEY,roadmap_id TEXT NOT NULL REFERENCES roadmaps(id),base_revision INTEGER NOT NULL,body TEXT NOT NULL,state TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS notifications(id TEXT PRIMARY KEY,execution_id TEXT NOT NULL REFERENCES executions(id),body TEXT NOT NULL,state TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS operations(id TEXT PRIMARY KEY,kind TEXT NOT NULL,state TEXT NOT NULL,body TEXT NOT NULL,result TEXT);
            CREATE TABLE IF NOT EXISTS workspaces(roadmap_id TEXT NOT NULL,node_id TEXT NOT NULL,body TEXT NOT NULL,PRIMARY KEY(roadmap_id,node_id),FOREIGN KEY(roadmap_id,node_id) REFERENCES nodes(roadmap_id,id));
            CREATE TABLE IF NOT EXISTS events(sequence INTEGER PRIMARY KEY AUTOINCREMENT,project_id TEXT,kind TEXT NOT NULL,body TEXT NOT NULL,created_ms INTEGER NOT NULL);
            CREATE TABLE IF NOT EXISTS requests(id TEXT PRIMARY KEY,digest TEXT NOT NULL,result TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS runner_state(operation_id TEXT PRIMARY KEY REFERENCES operations(id),execution_id TEXT NOT NULL REFERENCES executions(id),body TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS grants(id TEXT PRIMARY KEY,roadmap_id TEXT NOT NULL,node_id TEXT NOT NULL,contract_digest TEXT NOT NULL,body TEXT NOT NULL,state TEXT NOT NULL,FOREIGN KEY(roadmap_id,node_id) REFERENCES nodes(roadmap_id,id));
            CREATE TABLE IF NOT EXISTS legacy_sources(project_id TEXT NOT NULL REFERENCES projects(id),source_id TEXT NOT NULL,path TEXT NOT NULL,digest TEXT NOT NULL,body TEXT NOT NULL,PRIMARY KEY(project_id,source_id));
            CREATE TABLE IF NOT EXISTS milestones(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES projects(id),body TEXT NOT NULL,state TEXT NOT NULL);
            PRAGMA user_version=1; COMMIT;")?;
        if connection.query_row(
            "SELECT count(*) FROM pragma_table_info('executions') WHERE name='input_manifest'",
            [],
            |r| r.get::<_, u32>(0),
        )? == 0
        {
            connection.execute_batch(
                "ALTER TABLE executions ADD COLUMN input_manifest TEXT NOT NULL DEFAULT '[]';",
            )?;
        }
        Ok(Self {
            connection,
            home: home.into(),
        })
    }

    pub fn apply(&mut self, request: &Request) -> Result<Value> {
        if request.schema_version != 1
            || request.request_id.trim().is_empty()
            || request.request_id.len() > 256
            || !request.payload.is_object()
        {
            return Err(Error::new(
                "invalid_request",
                "schemaVersion=1 and requestId (1..256 bytes) are required",
            ));
        }
        let hash = digest(
            &serde_json::to_vec(request)
                .map_err(|e| Error::new("invalid_request", e.to_string()))?,
        );
        if let Some((old, result)) = self
            .connection
            .query_row(
                "SELECT digest,result FROM requests WHERE id=?1",
                [&request.request_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
        {
            if old != hash {
                return Err(Error::new("request_conflict", "requestId content differs"));
            }
            return parse(&result);
        }
        if request.command == "storage.backup" {
            human(&request.payload)?;
            let result = super::backup::create(&self.home, &self.connection, &request.request_id)?;
            self.connection.execute(
                "INSERT INTO requests VALUES(?1,?2,?3)",
                params![request.request_id, hash, result.to_string()],
            )?;
            return Ok(result);
        }
        if request.command == "storage.check" {
            let integrity: String =
                self.connection
                    .query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
            let foreign_keys: u64 = self.connection.query_row(
                "SELECT count(*) FROM pragma_foreign_key_check",
                [],
                |r| r.get(0),
            )?;
            let mut missing = Vec::new();
            let mut stmt = self.connection.prepare("SELECT id,body FROM handoffs")?;
            for row in
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            {
                let (id, body) = row?;
                if let Err(error) = super::git::verify_artifacts(&parse(&body)?) {
                    missing.push(json!({"handoffId":id,"error":error}));
                }
            }
            return Ok(
                json!({"integrity":integrity,"foreignKeyErrors":foreign_keys,"artifactErrors":missing}),
            );
        }
        if request.command == "milestone.land" {
            return self.land_milestone(request, &hash);
        }
        if ["node.prepare", "node.integrate", "integration.publish"]
            .contains(&request.command.as_str())
        {
            return self.external(request, &hash);
        }
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((old, result)) = tx
            .query_row(
                "SELECT digest,result FROM requests WHERE id=?1",
                [&request.request_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?
        {
            if old != hash {
                return Err(Error::new(
                    "request_conflict",
                    "requestId was used for different content",
                ));
            }
            return parse(&result);
        }
        let mut internal = request.clone();
        if request.command == "runner.recover" {
            internal.payload["serviceHome"] = json!(self.home);
        }
        let result = dispatch(&tx, &internal)?;
        if request.command == "runner.job"
            || request.command == "legacy.preview"
            || (request.command == "runner.approval_take" && result.is_null())
        {
            return Ok(result);
        }
        tx.execute(
            "INSERT INTO requests VALUES(?1,?2,?3)",
            params![request.request_id, hash, result.to_string()],
        )?;
        tx.commit()?;
        Ok(result)
    }

    fn external(&mut self, request: &Request, hash: &str) -> Result<Value> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((old, result)) = tx
            .query_row(
                "SELECT digest,result FROM requests WHERE id=?1",
                [&request.request_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?
        {
            if old != hash {
                return Err(Error::new("request_conflict", "requestId content differs"));
            }
            return parse(&result);
        }
        if tx
            .query_row(
                "SELECT 1 FROM operations WHERE id=?1",
                [&request.request_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some()
        {
            return Err(Error::new(
                "operation_uncertain",
                "Original operation exists; inspect its result before another Git action",
            ));
        }
        let p = &request.payload;
        let (roadmap, node_id, context) = match request.command.as_str() {
            "node.prepare" => {
                let roadmap = required(p, "roadmapId")?;
                let node_id = required(p, "nodeId")?;
                route(&tx, roadmap, Some(revision(p)?), false)?;
                let (contract, _, state) = node(&tx, roadmap, node_id)?;
                if state != "ready" || contract["code"] != true {
                    return Err(Error::new(
                        "node_not_ready",
                        "Only ready code nodes can prepare workspaces",
                    ));
                }
                if tx
                    .query_row(
                        "SELECT 1 FROM workspaces WHERE roadmap_id=?1 AND node_id=?2",
                        params![roadmap, node_id],
                        |_| Ok(()),
                    )
                    .optional()?
                    .is_some()
                {
                    return Err(Error::new(
                        "already_prepared",
                        "Node already has a workspace",
                    ));
                }
                dependencies_ready(&tx, roadmap, &contract)?;
                let repo = Path::new(required(p, "repositoryPath")?);
                let route_key = digest(roadmap.as_bytes());
                let integration = format!("refs/heads/codex/roadmap/{}", &route_key[..16]);
                let baseline = super::git::run(repo, &["rev-parse", "--verify", &integration])
                    .or_else(|_| {
                        super::git::run(
                            repo,
                            &[
                                "rev-parse",
                                "--verify",
                                &format!("refs/heads/{}", required(p, "targetBranch")?),
                            ],
                        )
                    })?;
                dependencies_in_code(&tx, roadmap, &contract, repo, &baseline)?;
                tx.execute(
                    "UPDATE nodes SET state='preparing' WHERE roadmap_id=?1 AND id=?2",
                    params![roadmap, node_id],
                )?;
                (roadmap.to_owned(), node_id.to_owned(), p.clone())
            }
            "node.integrate" => {
                human(p)?;
                let handoff = required(p, "handoffId")?;
                let (roadmap,node_id,body,state)=tx.query_row("SELECT e.roadmap_id,e.node_id,h.body,h.state FROM handoffs h JOIN executions e ON e.id=h.execution_id WHERE h.id=?1",[handoff],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?;
                route(&tx, &roadmap, Some(revision(p)?), false)?;
                if state != "accepted"
                    || node(&tx, &roadmap, &node_id)?.2 != "accepted_pending_integration"
                {
                    return Err(Error::new(
                        "handoff_not_accepted",
                        "Accepted code handoff required",
                    ));
                }
                let workspace = tx.query_row(
                    "SELECT body FROM workspaces WHERE roadmap_id=?1 AND node_id=?2",
                    params![roadmap, node_id],
                    |r| r.get::<_, String>(0),
                )?;
                let body = parse(&body)?;
                super::git::verify_artifacts(&body)?;
                (
                    roadmap,
                    node_id,
                    json!({"workspace":parse(&workspace)?,"source":required(&body,"codeRevision")?}),
                )
            }
            _ => {
                human(p)?;
                let operation = required(p, "candidateOperationId")?;
                let result=tx.query_row("SELECT result FROM operations WHERE id=?1 AND kind='node.integrate' AND state='completed'",[operation],|r|r.get::<_,String>(0)).optional()?.ok_or_else(||Error::new("candidate_missing","Integration candidate not found"))?;
                let candidate = parse(&result)?;
                let roadmap = required(&candidate, "roadmapId")?.to_owned();
                let node_id = required(&candidate, "nodeId")?.to_owned();
                route(&tx, &roadmap, Some(revision(p)?), false)?;
                if node(&tx, &roadmap, &node_id)?.2 != "accepted_pending_integration" {
                    return Err(Error::new(
                        "version_conflict",
                        "Node is no longer awaiting this integration",
                    ));
                }
                if candidate["requiresValidation"] == true
                    && (p["verifiedRevision"] != candidate["candidateRevision"]
                        || p["verificationPassed"] != true
                        || p["independentReviewApproved"] != true)
                {
                    return Err(Error::new("integration_validation_required","Changed integration requires passing verification and independent review of candidateRevision"));
                }
                (roadmap, node_id, candidate)
            }
        };
        tx.execute(
            "INSERT INTO operations VALUES(?1,?2,'pending',?3,NULL)",
            params![
                request.request_id,
                request.command,
                serde_json::to_string(request).unwrap()
            ],
        )?;
        tx.commit()?;
        let result = match request.command.as_str() {
            "node.prepare" => super::git::prepare(&self.home, &context),
            "node.integrate" => super::git::candidate(
                &self.home,
                &context["workspace"],
                &request.request_id,
                required(&context, "source")?,
            ),
            _ => super::git::publish(&context),
        };
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        match result {
            Ok(mut result) => {
                result["roadmapId"] = json!(roadmap);
                result["nodeId"] = json!(node_id);
                result["operationId"] = json!(request.request_id);
                if request.command == "node.prepare" {
                    tx.execute(
                        "INSERT INTO workspaces VALUES(?1,?2,?3)",
                        params![roadmap, node_id, result.to_string()],
                    )?;
                    tx.execute(
                        "UPDATE nodes SET state='ready' WHERE roadmap_id=?1 AND id=?2",
                        params![roadmap, node_id],
                    )?;
                } else if request.command == "integration.publish" {
                    tx.execute(
                        "UPDATE nodes SET state='integrated' WHERE roadmap_id=?1 AND id=?2",
                        params![roadmap, node_id],
                    )?;
                }
                tx.execute(
                    "UPDATE operations SET state='completed',result=?1 WHERE id=?2",
                    params![result.to_string(), request.request_id],
                )?;
                tx.execute(
                    "INSERT INTO requests VALUES(?1,?2,?3)",
                    params![request.request_id, hash, result.to_string()],
                )?;
                let (project, _) = route(&tx, &roadmap, None, true)?;
                event(&tx, &project, &request.command, &result)?;
                tx.commit()?;
                Ok(result)
            }
            Err(error) => {
                tx.execute(
                    "UPDATE operations SET state='needs_reconciliation',result=?1 WHERE id=?2",
                    params![serde_json::to_string(&error).unwrap(), request.request_id],
                )?;
                tx.commit()?;
                Err(error)
            }
        }
    }

    fn land_milestone(&mut self, request: &Request, hash: &str) -> Result<Value> {
        let p = &request.payload;
        human(p)?;
        let milestone = required(p, "milestoneId")?;
        let (body, state) = self.connection.query_row(
            "SELECT body,state FROM milestones WHERE id=?1",
            [milestone],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?;
        if state != "ready" {
            return Err(Error::new(
                "milestone_not_ready",
                "All milestone nodes must be ready",
            ));
        }
        if self
            .connection
            .query_row(
                "SELECT 1 FROM operations WHERE id=?1",
                [&request.request_id],
                |_| Ok(()),
            )
            .optional()?
            .is_some()
        {
            return Err(Error::new(
                "operation_uncertain",
                "Milestone operation already exists; inspect partial results",
            ));
        }
        let body = parse(&body)?;
        for item in body["nodes"].as_array().into_iter().flatten() {
            let roadmap = required(item, "roadmapId")?;
            let tx = self.connection.unchecked_transaction()?;
            let (project, current) = route(&tx, roadmap, Some(revision(item)?), false)?;
            let (contract, _, state) = node(&tx, roadmap, required(item, "nodeId")?)?;
            if project != body["projectId"].as_str().unwrap_or("")
                || current != revision(item)?
                || (contract["code"] == true && state != "integrated")
                || (contract["code"] != true && state != "accepted")
            {
                return Err(Error::new(
                    "milestone_stale",
                    "Milestone nodes changed; check and validate the milestone again",
                ));
            }
        }
        let expected = p["expectedTargets"].as_array().ok_or_else(|| {
            Error::new(
                "invalid_request",
                "expectedTargets must list repositoryPath and revision",
            )
        })?;
        if p["verifiedRepositoryVersions"] != body["repositoryVersions"] {
            return Err(Error::new(
                "milestone_verification_required",
                "Confirm exactly the repositoryVersions validated for this milestone",
            ));
        }
        self.connection.execute(
            "INSERT INTO operations VALUES(?1,'milestone.land','pending',?2,NULL)",
            params![request.request_id, request.payload.to_string()],
        )?;
        let mut results = Vec::new();
        let mut failure = None;
        for version in body["repositoryVersions"].as_array().into_iter().flatten() {
            let target = expected
                .iter()
                .find(|item| item["repositoryPath"] == version["repositoryPath"])
                .and_then(|item| item["revision"].as_str());
            let result = target
                .ok_or_else(|| {
                    Error::new(
                        "target_revision_required",
                        "Missing expected target revision",
                    )
                })
                .and_then(|target| super::git::land(version, target));
            match result {
                Ok(value) => results.push(value),
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
            self.connection.execute(
                "UPDATE operations SET result=?1 WHERE id=?2",
                params![
                    json!({"repositories":results,"complete":false}).to_string(),
                    request.request_id
                ],
            )?;
        }
        let result = json!({"milestoneId":milestone,"landed":failure.is_none(),"repositories":results,"error":failure,"automaticRollback":false});
        self.connection.execute(
            "UPDATE operations SET state=?1,result=?2 WHERE id=?3",
            params![
                if failure.is_none() {
                    "completed"
                } else {
                    "needs_reconciliation"
                },
                result.to_string(),
                request.request_id
            ],
        )?;
        if failure.is_none() {
            self.connection.execute(
                "UPDATE milestones SET state='landed' WHERE id=?1",
                [milestone],
            )?;
        }
        self.connection.execute(
            "INSERT INTO requests VALUES(?1,?2,?3)",
            params![request.request_id, hash, result.to_string()],
        )?;
        Ok(result)
    }

    pub fn snapshot(&self, project: Option<&str>) -> Result<Value> {
        let projects = rows(&self.connection,"SELECT json_object('id',id,'title',title,'control',control) FROM projects WHERE ?1 IS NULL OR id=?1",project)?;
        let roadmaps = rows(&self.connection,"SELECT json_object('id',id,'projectId',project_id,'title',title,'revision',revision) FROM roadmaps WHERE ?1 IS NULL OR project_id=?1",project)?;
        let nodes = rows(&self.connection,"SELECT json_object('roadmapId',n.roadmap_id,'id',n.id,'title',n.title,'state',n.state,'contract',json(n.contract)) FROM nodes n JOIN roadmaps r ON r.id=n.roadmap_id WHERE ?1 IS NULL OR r.project_id=?1",project)?;
        let executions = rows(&self.connection,"SELECT json_object('id',e.id,'roadmapId',e.roadmap_id,'nodeId',e.node_id,'revision',e.revision,'sessionId',e.session_id,'state',e.state,'noProgress',e.no_progress,'workspace',e.workspace) FROM executions e JOIN roadmaps r ON r.id=e.roadmap_id WHERE ?1 IS NULL OR r.project_id=?1",project)?;
        let handoffs = rows(&self.connection,"SELECT json_object('id',h.id,'executionId',h.execution_id,'state',h.state,'body',json(h.body)) FROM handoffs h JOIN executions e ON e.id=h.execution_id JOIN roadmaps r ON r.id=e.roadmap_id WHERE ?1 IS NULL OR r.project_id=?1",project)?;
        let notifications = rows(&self.connection,"SELECT json_object('id',n.id,'executionId',n.execution_id,'state',n.state,'body',json(n.body)) FROM notifications n JOIN executions e ON e.id=n.execution_id JOIN roadmaps r ON r.id=e.roadmap_id WHERE ?1 IS NULL OR r.project_id=?1",project)?;
        let events = rows(&self.connection,"SELECT json_object('sequence',sequence,'projectId',project_id,'kind',kind,'body',json(body),'createdMs',created_ms) FROM (SELECT * FROM events WHERE ?1 IS NULL OR project_id=?1 ORDER BY sequence DESC LIMIT 200) ORDER BY sequence",project)?;
        let operations=rows(&self.connection,"SELECT json_object('id',id,'kind',kind,'state',state,'result',json(result)) FROM operations WHERE ?1 IS NULL OR json_extract(body,'$.payload.projectId')=?1 OR json_extract(body,'$.projectId')=?1 OR coalesce(json_extract(body,'$.payload.roadmapId'),json_extract(body,'$.roadmapId')) IN (SELECT id FROM roadmaps WHERE project_id=?1)",project)?;
        let grants=rows(&self.connection,"SELECT json_object('id',g.id,'roadmapId',g.roadmap_id,'nodeId',g.node_id,'state',g.state,'body',json(g.body)) FROM grants g JOIN roadmaps r ON r.id=g.roadmap_id WHERE ?1 IS NULL OR r.project_id=?1",project)?;
        let runners=rows(&self.connection,"SELECT json_object('operationId',s.operation_id,'executionId',s.execution_id,'body',json(s.body)) FROM runner_state s JOIN executions e ON e.id=s.execution_id JOIN roadmaps r ON r.id=e.roadmap_id WHERE ?1 IS NULL OR r.project_id=?1",project)?;
        let proposals=rows(&self.connection,"SELECT json_object('id',s.id,'roadmapId',s.roadmap_id,'baseRevision',s.base_revision,'state',s.state,'body',json(s.body)) FROM proposals s JOIN roadmaps r ON r.id=s.roadmap_id WHERE ?1 IS NULL OR r.project_id=?1",project)?;
        let reviews=rows(&self.connection,"SELECT json_object('id',s.id,'handoffId',s.handoff_id,'reviewerSessionId',s.reviewer,'verdict',s.verdict,'body',json(s.body)) FROM reviews s JOIN handoffs h ON h.id=s.handoff_id JOIN executions e ON e.id=h.execution_id JOIN roadmaps r ON r.id=e.roadmap_id WHERE ?1 IS NULL OR r.project_id=?1",project)?;
        let workspaces=rows(&self.connection,"SELECT json(s.body) FROM workspaces s JOIN roadmaps r ON r.id=s.roadmap_id WHERE ?1 IS NULL OR r.project_id=?1",project)?;
        let revisions=rows(&self.connection,"SELECT json_object('roadmapId',s.roadmap_id,'revision',s.revision,'body',json(s.body)) FROM revisions s JOIN roadmaps r ON r.id=s.roadmap_id WHERE ?1 IS NULL OR r.project_id=?1",project)?;
        let milestones=rows(&self.connection,"SELECT json_object('id',id,'state',state,'body',json(body)) FROM milestones WHERE ?1 IS NULL OR project_id=?1",project)?;
        Ok(
            json!({"schemaVersion":1,"projects":projects,"roadmaps":roadmaps,"nodes":nodes,"executions":executions,"handoffs":handoffs,"notifications":notifications,"operations":operations,"grants":grants,"runners":runners,"proposals":proposals,"reviews":reviews,"workspaces":workspaces,"revisions":revisions,"milestones":milestones,"events":events}),
        )
    }

    pub fn events_after(&self, project: Option<&str>, after: u64, limit: usize) -> Result<Value> {
        let limit = limit.clamp(1, 500);
        let mut stmt = self.connection.prepare("SELECT sequence,project_id,kind,body,created_ms FROM events WHERE sequence>?1 AND (?2 IS NULL OR project_id=?2) ORDER BY sequence LIMIT ?3")?;
        let events = stmt.query_map(params![after, project, limit as u64], |row| {
            let body: String = row.get(3)?;
            Ok(json!({"sequence":row.get::<_,u64>(0)?,"projectId":row.get::<_,Option<String>>(1)?,"kind":row.get::<_,String>(2)?,"body":parse(&body).unwrap_or(Value::Null),"createdMs":row.get::<_,u64>(4)?}))
        })?.collect::<rusqlite::Result<Vec<_>>>()?;
        let cursor = events
            .last()
            .and_then(|v| v["sequence"].as_u64())
            .unwrap_or(after);
        Ok(
            json!({"schemaVersion":1,"events":events,"cursor":cursor,"after":after,"hasMore":events.len()==limit}),
        )
    }

    pub fn next_actions(&self, project: Option<&str>) -> Result<Value> {
        let snapshot = self.snapshot(project)?;
        let mut actions = Vec::new();
        for notification in snapshot["notifications"].as_array().into_iter().flatten() {
            if notification["state"] == "pending" {
                actions.push(json!({"kind":"notification","priority":0,"executionId":notification["executionId"],"notificationId":notification["id"],"body":notification["body"]}));
            }
        }
        for handoff in snapshot["handoffs"].as_array().into_iter().flatten() {
            if handoff["state"] == "submitted" {
                actions.push(json!({"kind":"handoff","priority":1,"handoffId":handoff["id"],"executionId":handoff["executionId"],"action":"review_or_accept","body":handoff["body"]}));
            }
        }
        for execution in snapshot["executions"].as_array().into_iter().flatten() {
            if ["needs_human", "pause_requested"]
                .contains(&execution["state"].as_str().unwrap_or(""))
            {
                actions.push(json!({"kind":"execution","priority":0,"executionId":execution["id"],"action":execution["state"],"sessionId":execution["sessionId"]}));
            }
        }
        for grant in snapshot["grants"].as_array().into_iter().flatten() {
            if grant["state"] == "waiting" {
                actions.push(json!({"kind":"authorization","priority":1,"grantId":grant["id"],"roadmapId":grant["roadmapId"],"nodeId":grant["nodeId"],"action":"grant_or_reject"}));
            }
        }
        actions.sort_by_key(|v| v["priority"].as_i64().unwrap_or(9));
        let cursor = snapshot["events"]
            .as_array()
            .and_then(|v| v.last())
            .and_then(|v| v["sequence"].as_u64())
            .unwrap_or(0);
        Ok(
            json!({"schemaVersion":1,"project":project,"cursor":cursor,"actions":actions,"message":if actions.is_empty(){"No human or agent action is currently required"}else{"Actions are ordered by urgency"}}),
        )
    }

    pub fn schedule_authorized(&mut self) -> Result<()> {
        let grants = {
            let mut stmt=self.connection.prepare("SELECT id,roadmap_id,node_id,contract_digest,body FROM grants WHERE state='waiting'")?;
            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        for (grant, roadmap, node_id, hash, body) in grants {
            let current=self.connection.query_row("SELECT n.digest,r.revision,p.control,n.state FROM nodes n JOIN roadmaps r ON r.id=n.roadmap_id JOIN projects p ON p.id=r.project_id WHERE n.roadmap_id=?1 AND n.id=?2",params![roadmap,node_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u64>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?;
            if current.0 != hash {
                self.connection
                    .execute("UPDATE grants SET state='invalidated' WHERE id=?1", [grant])?;
                continue;
            }
            if current.2 != "active" || current.3 != "ready" {
                continue;
            }
            let body = parse(&body)?;
            let contract: String = self.connection.query_row(
                "SELECT contract FROM nodes WHERE roadmap_id=?1 AND id=?2",
                params![roadmap, node_id],
                |r| r.get(0),
            )?;
            if parse(&contract)?["code"] == true
                && self
                    .connection
                    .query_row(
                        "SELECT 1 FROM workspaces WHERE roadmap_id=?1 AND node_id=?2",
                        params![roadmap, node_id],
                        |_| Ok(()),
                    )
                    .optional()?
                    .is_none()
            {
                let source=body["sourceHandoffId"].as_str().and_then(|handoff|self.connection.query_row("SELECT w.body FROM handoffs h JOIN executions e ON e.id=h.execution_id JOIN workspaces w ON w.roadmap_id=e.roadmap_id AND w.node_id=e.node_id WHERE h.id=?1",[handoff],|r|r.get::<_,String>(0)).ok()).and_then(|s|parse(&s).ok());
                if let Some(source) = source {
                    let prepared=self.apply(&Request{schema_version:1,request_id:format!("grant:{grant}:prepare"),command:"node.prepare".into(),payload:json!({"roadmapId":roadmap,"nodeId":node_id,"expectedRevision":current.1,"repositoryPath":source["repositoryPath"],"targetBranch":source["targetBranch"]})});
                    if let Err(error) = prepared {
                        if error.code == "dependencies_not_ready" {
                            continue;
                        }
                        self.connection.execute("UPDATE grants SET state='needs_human',body=json_set(body,'$.error',json(?1)) WHERE id=?2",params![serde_json::to_string(&error).unwrap(),grant])?;
                        continue;
                    }
                }
            }
            let payload = json!({"roadmapId":roadmap,"nodeId":node_id,"expectedRevision":current.1,"workspace":body["workspace"],"model":body["model"],"humanConfirmed":true});
            match self.apply(&Request {
                schema_version: 1,
                request_id: format!("grant:{grant}:launch"),
                command: "node.launch".into(),
                payload,
            }) {
                Ok(_) => {
                    self.connection
                        .execute("UPDATE grants SET state='consumed' WHERE id=?1", [grant])?;
                }
                Err(error)
                    if [
                        "dependencies_not_ready",
                        "workspace_required",
                        "paused",
                        "node_not_ready",
                    ]
                    .contains(&error.code.as_str()) => {}
                Err(error) => {
                    self.connection.execute("UPDATE grants SET state='needs_human',body=json_set(body,'$.error',json(?1)) WHERE id=?2",params![serde_json::to_string(&error).unwrap(),grant])?;
                }
            }
        }
        Ok(())
    }

    pub fn integrate_accepted(&mut self, request: &Request, result: &mut Value) -> Result<()> {
        if request.command != "handoff.accept" {
            return Ok(());
        }
        let handoff = required(&request.payload, "handoffId")?;
        let (roadmap,node_id)=self.connection.query_row("SELECT e.roadmap_id,e.node_id FROM handoffs h JOIN executions e ON e.id=h.execution_id WHERE h.id=?1",[handoff],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?;
        let (state,revision)=self.connection.query_row("SELECT n.state,r.revision FROM nodes n JOIN roadmaps r ON r.id=n.roadmap_id WHERE n.roadmap_id=?1 AND n.id=?2",params![roadmap,node_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u64>(1)?)))?;
        if state != "accepted_pending_integration" {
            return Ok(());
        }
        let operation = format!("accept:{}:candidate", digest(request.request_id.as_bytes()));
        let candidate = self.apply(&Request {
            schema_version: 1,
            request_id: operation.clone(),
            command: "node.integrate".into(),
            payload: json!({"handoffId":handoff,"expectedRevision":revision,"humanConfirmed":true}),
        });
        match candidate {
            Ok(candidate) if candidate["requiresValidation"] == false => {
                let published=self.apply(&Request{schema_version:1,request_id:format!("{operation}:publish"),command:"integration.publish".into(),payload:json!({"candidateOperationId":operation,"expectedRevision":revision,"humanConfirmed":true})});
                result["integration"] = match published {
                    Ok(value) => value,
                    Err(error) => json!({"state":"needs_human","error":error}),
                };
            }
            Ok(candidate) => {
                result["integration"] = candidate;
                result["integration"]["state"] = json!("verification_required");
            }
            Err(error) => {
                result["integration"] = json!({"state":"needs_human","error":error});
            }
        }
        Ok(())
    }
}

fn parse(value: &str) -> Result<Value> {
    serde_json::from_str(value).map_err(|e| Error::new("corrupt_record", e.to_string()))
}
fn rows(connection: &Connection, sql: &str, project: Option<&str>) -> Result<Vec<Value>> {
    let mut stmt = connection.prepare(sql)?;
    let values = stmt
        .query_map([project], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    values.iter().map(|s| parse(s)).collect()
}
fn event(tx: &Transaction<'_>, project: &str, kind: &str, value: &Value) -> Result<()> {
    tx.execute(
        "INSERT INTO events(project_id,kind,body,created_ms) VALUES(?1,?2,?3,?4)",
        params![project, kind, value.to_string(), now()],
    )?;
    Ok(())
}
fn route(
    tx: &Transaction<'_>,
    id: &str,
    revision: Option<u64>,
    allow_paused: bool,
) -> Result<(String, u64)> {
    let (project,current,control) = tx.query_row("SELECT r.project_id,r.revision,p.control FROM roadmaps r JOIN projects p ON p.id=r.project_id WHERE r.id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,u64>(1)?,r.get::<_,String>(2)?))).optional()?.ok_or_else(||Error::new("not_found","Roadmap not found"))?;
    if revision.is_some_and(|v| v != current) {
        return Err(Error::new(
            "version_conflict",
            format!("Current roadmap revision is {current}"),
        ));
    }
    if !allow_paused && control != "active" {
        return Err(Error::new("paused", "Project is paused or stopped"));
    }
    Ok((project, current))
}
fn revision(p: &Value) -> Result<u64> {
    p.get("expectedRevision")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::new("invalid_request", "expectedRevision is required"))
}
fn dependencies_ready(tx: &Transaction<'_>, roadmap: &str, contract: &Value) -> Result<()> {
    for dep in contract["dependencies"].as_array().into_iter().flatten() {
        let (upstream, _, state) = node(tx, roadmap, dep.as_str().unwrap_or(""))?;
        let ready = if upstream["code"] == true {
            state == "integrated"
        } else {
            state == "accepted"
        };
        if !ready {
            return Err(Error::new(
                "dependencies_not_ready",
                "Upstream handoff or code is not ready",
            ));
        }
        let body=tx.query_row("SELECT h.body FROM handoffs h JOIN executions e ON e.id=h.execution_id WHERE e.roadmap_id=?1 AND e.node_id=?2 AND h.state='accepted' ORDER BY h.rowid DESC LIMIT 1",params![roadmap,dep.as_str()],|r|r.get::<_,String>(0)).optional()?;
        if let Some(body) = body {
            super::git::verify_artifacts(&parse(&body)?)?;
        }
    }
    Ok(())
}
fn dependencies_in_code(
    tx: &Transaction<'_>,
    roadmap: &str,
    contract: &Value,
    repo: &Path,
    baseline: &str,
) -> Result<()> {
    for dep in contract["dependencies"].as_array().into_iter().flatten() {
        let (source, _, _) = node(tx, roadmap, dep.as_str().unwrap_or(""))?;
        if source["code"] != true {
            continue;
        }
        let workspace: String = tx.query_row(
            "SELECT body FROM workspaces WHERE roadmap_id=?1 AND node_id=?2",
            params![roadmap, dep.as_str()],
            |r| r.get(0),
        )?;
        let workspace = parse(&workspace)?;
        if super::git::identity(repo)?
            == super::git::identity(Path::new(required(&workspace, "repositoryPath")?))?
        {
            let body:String=tx.query_row("SELECT h.body FROM handoffs h JOIN executions e ON e.id=h.execution_id WHERE e.roadmap_id=?1 AND e.node_id=?2 AND h.state='accepted' ORDER BY h.rowid DESC LIMIT 1",params![roadmap,dep.as_str()],|r|r.get(0))?;
            super::git::contains(repo, baseline, required(&parse(&body)?, "codeRevision")?)?;
        }
    }
    Ok(())
}
fn node(tx: &Transaction<'_>, route: &str, id: &str) -> Result<(Value, String, String)> {
    let (contract, hash, state) = tx
        .query_row(
            "SELECT contract,digest,state FROM nodes WHERE roadmap_id=?1 AND id=?2",
            params![route, id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| Error::new("not_found", "Node not found"))?;
    Ok((parse(&contract)?, hash, state))
}
fn execution(
    tx: &Transaction<'_>,
    p: &Value,
    allow_paused: bool,
) -> Result<(String, String, String, String, String)> {
    let execution_id = required(p, "executionId")?;
    let (roadmap,node_id,token,state,session,contract_hash,started_revision)=tx.query_row("SELECT roadmap_id,node_id,token_digest,state,session_id,contract_digest,revision FROM executions WHERE id=?1",[execution_id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,String>(5)?,r.get::<_,u64>(6)?))).optional()?.ok_or_else(||Error::new("not_found","Execution not found"))?;
    if digest(required(p, "token")?.as_bytes()) != token {
        return Err(Error::new(
            "invalid_owner",
            "Execution token does not match",
        ));
    }
    let (project, current) = route(tx, &roadmap, None, allow_paused)?;
    if revision(p)? != started_revision && revision(p)? != current {
        return Err(Error::new(
            "version_conflict",
            "Execution revision does not match its start or current route",
        ));
    }
    if node(tx, &roadmap, &node_id)?.1 != contract_hash
        && !(allow_paused && (state == "paused" || state == "pause_requested"))
    {
        return Err(Error::new("version_conflict", "Node requirements changed"));
    }
    Ok((roadmap, node_id, project, state, session))
}

fn validate_nodes(p: &Value) -> Result<Vec<Value>> {
    let nodes = p
        .get("nodes")
        .and_then(Value::as_array)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| Error::new("invalid_request", "nodes must be nonempty"))?
        .clone();
    let mut dependencies = HashMap::new();
    for n in &nodes {
        let id = required(n, "id")?;
        required(n, "title")?;
        required(n, "goal")?;
        for flag in ["code", "requiresReview"] {
            if n.get(flag).is_some_and(|v| !v.is_boolean()) {
                return Err(Error::new(
                    "invalid_request",
                    format!("{flag} must be a boolean"),
                ));
            }
        }
        if n.get("dependencies").is_some_and(|v| !v.is_array()) {
            return Err(Error::new(
                "invalid_request",
                "dependencies must be an array",
            ));
        }
        if n.get("acceptance")
            .and_then(Value::as_array)
            .is_none_or(|v| {
                v.is_empty()
                    || v.iter()
                        .any(|x| x.as_str().is_none_or(|s| s.trim().is_empty()))
            })
        {
            return Err(Error::new(
                "invalid_request",
                "Each node requires nonempty acceptance strings",
            ));
        }
        let deps = n
            .get("dependencies")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|x| {
                x.as_str().map(str::to_owned).ok_or_else(|| {
                    Error::new("invalid_request", "dependencies must contain node IDs")
                })
            })
            .collect::<Result<Vec<_>>>()?;
        if dependencies.insert(id.to_owned(), deps).is_some() {
            return Err(Error::new("invalid_graph", "Duplicate node ID"));
        }
    }
    fn visit(
        key: &str,
        graph: &HashMap<String, Vec<String>>,
        active: &mut HashSet<String>,
        done: &mut HashSet<String>,
    ) -> Result<()> {
        if done.contains(key) {
            return Ok(());
        }
        if !active.insert(key.into()) {
            return Err(Error::new("invalid_graph", "Dependency cycle"));
        }
        for dep in graph
            .get(key)
            .ok_or_else(|| Error::new("invalid_graph", format!("Unknown dependency {key}")))?
        {
            visit(dep, graph, active, done)?;
        }
        active.remove(key);
        done.insert(key.into());
        Ok(())
    }
    let mut done = HashSet::new();
    for key in dependencies.keys() {
        visit(key, &dependencies, &mut HashSet::new(), &mut done)?;
    }
    Ok(nodes)
}

fn node_digest(node: &Value, roadmap: &Value) -> String {
    digest(json!({"node":node,"goal":roadmap["goal"],"constraints":roadmap["constraints"],"pluginName":roadmap["pluginName"]}).to_string().as_bytes())
}
fn session_key(value: &str) -> String {
    crate::codex_session_key(value)
        .map(|s| format!("codex:{}", s.to_ascii_lowercase()))
        .unwrap_or_else(|| value.to_owned())
}
fn publish(tx: &Transaction<'_>, p: &Value) -> Result<Value> {
    human(p)?;
    let id = required(p, "id")?;
    let project = required(p, "projectId")?;
    let title = required(p, "title")?;
    let expected = revision(p)?;
    if let Some(plugin) = p["pluginName"].as_str() {
        crate::ensure_plugin_exists(plugin)
            .and_then(|_| crate::ensure_plugin_enabled(plugin))
            .map_err(|e| Error::new("plugin_unavailable", e))?;
    }
    let nodes = validate_nodes(p)?;
    if expected == 0 {
        if tx
            .query_row("SELECT 1 FROM roadmaps WHERE id=?1", [id], |_| Ok(()))
            .optional()?
            .is_some()
        {
            return Err(Error::new("version_conflict", "Roadmap already exists"));
        }
        tx.execute(
            "INSERT INTO roadmaps VALUES(?1,?2,?3,1)",
            params![id, project, title],
        )?;
    } else {
        let (old_project, _) = route(tx, id, Some(expected), true)?;
        if old_project != project {
            return Err(Error::new(
                "scope_conflict",
                "Cannot move roadmap to another project",
            ));
        }
        let proposal = required(p, "proposalId")?;
        let valid=tx.query_row("SELECT 1 FROM proposals WHERE id=?1 AND roadmap_id=?2 AND base_revision=?3 AND state='open'",params![proposal,id,expected],|_|Ok(())).optional()?.is_some();
        if !valid {
            return Err(Error::new(
                "proposal_required",
                "Revision requires an open proposal for the current revision",
            ));
        }
        let proposal_body: String =
            tx.query_row("SELECT body FROM proposals WHERE id=?1", [proposal], |r| {
                r.get(0)
            })?;
        let proposal_body = parse(&proposal_body)?;
        let outstanding: u64 = tx.query_row(
            "SELECT count(*) FROM executions WHERE roadmap_id=?1 AND state='pause_requested'",
            [id],
            |r| r.get(0),
        )?;
        if outstanding > 0 {
            return Err(Error::new(
                "writers_not_stopped",
                "Wait for safe-stop receipts before publishing the new roadmap",
            ));
        }
        let old_nodes = {
            let mut stmt = tx.prepare("SELECT id,digest FROM nodes WHERE roadmap_id=?1")?;
            let rows = stmt
                .query_map([id], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            rows
        };
        for (node_id, old_hash) in old_nodes {
            let changed = nodes
                .iter()
                .find(|n| n["id"] == node_id)
                .is_none_or(|n| node_digest(n, p) != old_hash);
            if changed && proposal_body["previousStates"].get(&node_id).is_none() {
                return Err(Error::new(
                    "affected_scope_expanded",
                    format!("Changed node {node_id} was not included in the stop proposal"),
                ));
            }
        }
        tx.execute(
            "UPDATE proposals SET state='published' WHERE id=?1",
            [proposal],
        )?;
        tx.execute(
            "UPDATE roadmaps SET revision=?1,title=?2 WHERE id=?3",
            params![expected + 1, title, id],
        )?;
    }
    tx.execute(
        "INSERT INTO revisions VALUES(?1,?2,?3)",
        params![id, expected + 1, p.to_string()],
    )?;
    let old_ids = {
        let mut stmt = tx.prepare("SELECT id FROM nodes WHERE roadmap_id=?1")?;
        let values = stmt
            .query_map([id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
    };
    let mut invalidated = HashSet::new();
    for old in &old_ids {
        if nodes
            .iter()
            .find(|n| n["id"] == *old)
            .is_none_or(|n| node(tx, id, old).is_ok_and(|(_, hash, _)| hash != node_digest(n, p)))
        {
            invalidated.insert(old.clone());
        }
    }
    loop {
        let before = invalidated.len();
        for n in &nodes {
            if n["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|d| d.as_str().is_some_and(|d| invalidated.contains(d)))
            {
                invalidated.insert(required(n, "id")?.to_owned());
            }
        }
        if before == invalidated.len() {
            break;
        }
    }
    for affected in &invalidated {
        tx.execute("UPDATE grants SET state='invalidated' WHERE roadmap_id=?1 AND node_id=?2 AND state IN ('waiting','needs_human')",params![id,affected])?;
    }
    for old in old_ids {
        if !nodes.iter().any(|n| n["id"] == old) {
            tx.execute(
                "UPDATE nodes SET state='withdrawn' WHERE roadmap_id=?1 AND id=?2",
                params![id, old],
            )?;
        }
    }
    for n in nodes {
        let node_id = required(&n, "id")?;
        let hash = node_digest(&n, p);
        let state = match node(tx, id, node_id) {
            Ok(_) if invalidated.contains(node_id) => "recheck".into(),
            Ok((_, old_hash, old_state)) if old_hash == hash && old_state != "frozen" => old_state,
            Ok((_, old_hash, _)) if old_hash == hash => {
                let frozen=tx.query_row("SELECT body FROM proposals WHERE roadmap_id=?1 AND base_revision=?2 ORDER BY rowid DESC LIMIT 1",params![id,expected],|r|r.get::<_,String>(0)).optional()?.and_then(|s|parse(&s).ok());
                frozen
                    .and_then(|v| v["previousStates"][node_id].as_str().map(str::to_owned))
                    .unwrap_or_else(|| "recheck".into())
            }
            Ok(_) => "recheck".into(),
            Err(_) => "ready".into(),
        };
        tx.execute("INSERT INTO nodes VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(roadmap_id,id) DO UPDATE SET title=excluded.title,contract=excluded.contract,digest=excluded.digest,state=excluded.state",params![id,node_id,required(&n,"title")?,n.to_string(),hash,state])?;
    }
    event(tx, project, "roadmap.published", p)?;
    Ok(json!({"roadmapId":id,"revision":expected+1}))
}

fn dispatch(tx: &Transaction<'_>, request: &Request) -> Result<Value> {
    let p = &request.payload;
    match request.command.as_str() {
        "project.create" => {
            let id = required(p, "id")?;
            let title = required(p, "title")?;
            tx.execute(
                "INSERT INTO projects(id,title) VALUES(?1,?2)",
                params![id, title],
            )
            .map_err(|e| Error::new("already_exists", e.to_string()))?;
            event(tx, id, "project.created", p)?;
            Ok(json!({"projectId":id}))
        }
        "legacy.preview" => super::legacy::inspect(Path::new(required(p, "path")?)),
        "legacy.import" => {
            human(p)?;
            let project = required(p, "projectId")?;
            let source = super::legacy::inspect(Path::new(required(p, "path")?))?;
            if source["sha256"] != p["expectedDigest"] {
                return Err(Error::new(
                    "source_changed",
                    "Preview digest does not match source",
                ));
            }
            let source_id = required(&source["metadata"], "id")?;
            if let Some(old) = tx
                .query_row(
                    "SELECT digest FROM legacy_sources WHERE project_id=?1 AND source_id=?2",
                    params![project, source_id],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
            {
                if old != source["sha256"].as_str().unwrap() {
                    return Err(Error::new(
                        "legacy_source_conflict",
                        "Same legacy ID has differing sources",
                    ));
                }
                return Ok(
                    json!({"sourceId":source_id,"imported":true,"replayed":true,"currentEvidenceAccepted":false}),
                );
            }
            tx.execute(
                "INSERT INTO legacy_sources VALUES(?1,?2,?3,?4,?5)",
                params![
                    project,
                    source_id,
                    source["path"].as_str(),
                    source["sha256"].as_str(),
                    source.to_string()
                ],
            )?;
            event(
                tx,
                project,
                "legacy.imported",
                &json!({"sourceId":source_id,"path":source["path"],"sha256":source["sha256"]}),
            )?;
            Ok(
                json!({"sourceId":source_id,"imported":true,"currentEvidenceAccepted":false,"state":"requires_mapping_and_revalidation"}),
            )
        }
        "milestone.check" => {
            let project = required(p, "projectId")?;
            let milestone = required(p, "id")?;
            let items = p["nodes"]
                .as_array()
                .filter(|a| !a.is_empty())
                .ok_or_else(|| Error::new("invalid_request", "Milestone requires nodes"))?;
            let mut versions = Vec::new();
            let mut blockers = Vec::new();
            for item in items {
                let roadmap = required(item, "roadmapId")?;
                let node_id = required(item, "nodeId")?;
                if route(tx, roadmap, Some(revision(item)?), true)?.0 != project {
                    return Err(Error::new(
                        "scope_conflict",
                        "Milestone node belongs to another project",
                    ));
                }
                let (contract, _, state) = node(tx, roadmap, node_id)?;
                if (contract["code"] == true && state != "integrated")
                    || (contract["code"] != true && state != "accepted")
                {
                    blockers.push(json!({"roadmapId":roadmap,"nodeId":node_id,"state":state}));
                    continue;
                }
                if contract["code"] == true {
                    let body: String = tx.query_row(
                        "SELECT body FROM workspaces WHERE roadmap_id=?1 AND node_id=?2",
                        params![roadmap, node_id],
                        |r| r.get(0),
                    )?;
                    let workspace = parse(&body)?;
                    let repo = Path::new(required(&workspace, "repositoryPath")?);
                    let reference =
                        format!("refs/heads/{}", required(&workspace, "integrationBranch")?);
                    versions.push(json!({"repositoryPath":repo,"integrationBranch":workspace["integrationBranch"],"revision":super::git::run(repo,&["rev-parse","--verify",&reference])?,"targetBranch":workspace["targetBranch"]}));
                }
            }
            if tx
                .query_row(
                    "SELECT project_id FROM milestones WHERE id=?1",
                    [milestone],
                    |r| r.get::<_, String>(0),
                )
                .optional()?
                .is_some_and(|old| old != project)
            {
                return Err(Error::new(
                    "scope_conflict",
                    "Milestone belongs to another project",
                ));
            }
            let mut unique = Vec::<Value>::new();
            for version in versions {
                if let Some(old) = unique.iter().find(|v| {
                    v["repositoryPath"] == version["repositoryPath"]
                        && v["targetBranch"] == version["targetBranch"]
                }) {
                    if old != &version {
                        blockers.push(json!({"reason":"Multiple integration versions target the same repository branch; consolidate and validate first","repositoryPath":version["repositoryPath"]}));
                    }
                } else {
                    unique.push(version);
                }
            }
            let result = json!({"id":milestone,"projectId":project,"ready":blockers.is_empty(),"blockers":blockers,"repositoryVersions":unique,"nodes":items,"landed":false});
            tx.execute("INSERT INTO milestones VALUES(?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET body=excluded.body,state=excluded.state",params![milestone,project,result.to_string(),if blockers.is_empty(){"ready"}else{"blocked"}])?;
            event(tx, project, "milestone.checked", &result)?;
            Ok(result)
        }
        "roadmap.publish" => publish(tx, p),
        "node.launch" => {
            human(p)?;
            let roadmap = required(p, "roadmapId")?;
            let body:String=tx.query_row("SELECT v.body FROM revisions v JOIN roadmaps r ON r.id=v.roadmap_id AND r.revision=v.revision WHERE r.id=?1",[roadmap],|r|r.get(0))?;
            if let Some(plugin) = parse(&body)?["pluginName"].as_str() {
                crate::ensure_plugin_exists(plugin)
                    .and_then(|_| crate::ensure_plugin_enabled(plugin))
                    .map_err(|e| Error::new("plugin_unavailable", e))?;
            }
            let mut payload = p.clone();
            payload["sessionId"] = json!(format!("codex:pending:{}", request.request_id));
            let claim = dispatch(
                tx,
                &Request {
                    schema_version: 1,
                    request_id: format!("{}:claim", request.request_id),
                    command: "node.claim".into(),
                    payload,
                },
            )?;
            let path = claim["workspace"]["workspacePath"]
                .as_str()
                .or_else(|| p["workspace"].as_str())
                .ok_or_else(|| {
                    Error::new(
                        "workspace_required",
                        "A real workspace is required to launch Codex",
                    )
                })?;
            let path = std::fs::canonicalize(path)?;
            if !path.is_dir() {
                return Err(Error::new(
                    "workspace_required",
                    "Workspace must be a directory",
                ));
            }
            let job = json!({"claim":claim,"workspace":path,"model":p["model"],"roadmapId":p["roadmapId"],"nodeId":p["nodeId"],"expectedRevision":p["expectedRevision"],"sandbox":if claim["contract"]["code"]==true{"workspace-write"}else{"read-only"}});
            tx.execute(
                "INSERT INTO operations VALUES(?1,'codex.run','queued',?2,NULL)",
                params![request.request_id, job.to_string()],
            )?;
            tx.execute(
                "INSERT INTO runner_state VALUES(?1,?2,?3)",
                params![
                    request.request_id,
                    claim["executionId"].as_str(),
                    json!({"state":"queued","threadId":null,"turnId":null}).to_string()
                ],
            )?;
            Ok(
                json!({"operationId":request.request_id,"executionId":claim["executionId"],"state":"queued"}),
            )
        }
        "runner.recover" => {
            human(p)?;
            let reason = required(p, "reason")?;
            let operation = required(p, "operationId")?;
            let (body, state): (String, String) = tx.query_row(
                "SELECT body,state FROM operations WHERE id=?1 AND kind='codex.run'",
                [operation],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            if !["needs_human", "failed", "needs_reconciliation"].contains(&state.as_str()) {
                return Err(Error::new(
                    "reconciliation_required",
                    "Only stopped, uncertain runners may be recovered",
                ));
            }
            let directory = Path::new(required(p, "serviceHome")?)
                .join("runners")
                .join(digest(operation.as_bytes()));
            let lock = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(directory.join("runner.lock"))?;
            lock.try_lock().map_err(|_| {
                Error::new("runner_already_active", "Original runner is still alive")
            })?;
            let mut job = parse(&body)?;
            let execution_id = required(&job["claim"], "executionId")?.to_owned();
            let runner: String = tx.query_row(
                "SELECT body FROM runner_state WHERE operation_id=?1",
                [operation],
                |r| r.get(0),
            )?;
            let runner = parse(&runner)?;
            let thread = required(&runner, "threadId").map_err(|_| {
                Error::new(
                    "reconciliation_required",
                    "Thread identity unknown; inspect original process before any new launch",
                )
            })?;
            let roadmap = required(&job, "roadmapId")?;
            let (project, current) = route(tx, roadmap, None, false)?;
            let (_, hash, node_state) = node(tx, roadmap, required(&job, "nodeId")?)?;
            let (execution_state, old_hash): (String, String) = tx.query_row(
                "SELECT state,contract_digest FROM executions WHERE id=?1",
                [&execution_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            if hash != old_hash || ["frozen", "recheck", "withdrawn"].contains(&node_state.as_str())
            {
                return Err(Error::new(
                    "version_conflict",
                    "Resolve roadmap changes first",
                ));
            }
            if job["role"] == "review" {
                if execution_state != "submitted" {
                    return Err(Error::new("handoff_not_pending", execution_state));
                }
            } else if !["needs_human", "paused", "working"].contains(&execution_state.as_str()) {
                return Err(Error::new("execution_not_paused", execution_state));
            }
            job["resumeThreadId"] = json!(thread);
            job["recoveryReason"] = json!(reason);
            job["expectedRevision"] = json!(current);
            tx.execute(
                "UPDATE operations SET state='queued',body=?1 WHERE id=?2",
                params![job.to_string(), operation],
            )?;
            tx.execute("UPDATE runner_state SET body=json_set(body,'$.state','queued','$.approval',NULL) WHERE operation_id=?1",[operation])?;
            if job["role"] != "review" {
                tx.execute(
                    "UPDATE executions SET state='working',no_progress=0 WHERE id=?1",
                    [execution_id],
                )?;
            }
            event(
                tx,
                &project,
                "runner.recovery_authorized",
                &json!({"operationId":operation,"threadId":thread,"reason":reason}),
            )?;
            Ok(
                json!({"operationId":operation,"threadId":thread,"state":"queued","newThread":false}),
            )
        }
        "runner.job" => {
            let operation = required(p, "operationId")?;
            let body: String = tx.query_row(
                "SELECT body FROM operations WHERE id=?1 AND kind='codex.run'",
                [operation],
                |r| r.get(0),
            )?;
            let job = parse(&body)?;
            if digest(required(p, "token")?.as_bytes())
                != tx.query_row(
                    "SELECT token_digest FROM executions WHERE id=?1",
                    [job["claim"]["executionId"].as_str()],
                    |r| r.get::<_, String>(0),
                )?
            {
                return Err(Error::new("invalid_owner", "Runner token does not match"));
            }
            let (state, session) = tx.query_row(
                "SELECT state,session_id FROM executions WHERE id=?1",
                [job["claim"]["executionId"].as_str()],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )?;
            let (project, current) = route(tx, required(&job, "roadmapId")?, None, true)?;
            let mut control: String =
                tx.query_row("SELECT control FROM projects WHERE id=?1", [project], |r| {
                    r.get(0)
                })?;
            let route_body: String = tx.query_row(
                "SELECT body FROM revisions WHERE roadmap_id=?1 AND revision=?2",
                params![job["roadmapId"].as_str(), current],
                |r| r.get(0),
            )?;
            if let Some(plugin) = parse(&route_body)?["pluginName"].as_str() {
                if crate::ensure_plugin_exists(plugin)
                    .and_then(|_| crate::ensure_plugin_enabled(plugin))
                    .is_err()
                {
                    control = "plugin_disabled".into();
                }
            }
            Ok(
                json!({"job":job,"state":state,"sessionId":session,"control":control,"currentRevision":current}),
            )
        }
        "review.launch" => {
            let handoff = required(p, "handoffId")?;
            let (execution_id,roadmap,node_id,token,state,workspace,hash)=tx.query_row("SELECT e.id,e.roadmap_id,e.node_id,e.token_digest,h.state,e.workspace,e.contract_digest FROM handoffs h JOIN executions e ON e.id=h.execution_id WHERE h.id=?1",[handoff],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,String>(6)?)))?;
            if digest(required(p, "token")?.as_bytes()) != token {
                return Err(Error::new(
                    "invalid_owner",
                    "Review launch requires the original execution token",
                ));
            }
            if state != "submitted" {
                return Err(Error::new("handoff_not_pending", state));
            }
            let receipt: String =
                tx.query_row("SELECT body FROM handoffs WHERE id=?1", [handoff], |r| {
                    r.get(0)
                })?;
            super::git::verify_artifacts(&parse(&receipt)?)?;
            let (_, current) = route(tx, &roadmap, None, false)?;
            let (contract, current_hash, _) = node(tx, &roadmap, &node_id)?;
            if hash != current_hash {
                return Err(Error::new("version_conflict", "Handoff contract changed"));
            }
            let body: String =
                tx.query_row("SELECT body FROM handoffs WHERE id=?1", [handoff], |r| {
                    r.get(0)
                })?;
            let job = json!({"role":"review","handoffId":handoff,"handoff":parse(&body)?,"claim":{"executionId":execution_id,"token":p["token"],"contract":contract},"workspace":workspace,"sandbox":"read-only","roadmapId":roadmap,"nodeId":node_id,"expectedRevision":current,"model":p["model"]});
            tx.execute(
                "INSERT INTO operations VALUES(?1,'codex.run','queued',?2,NULL)",
                params![request.request_id, job.to_string()],
            )?;
            tx.execute(
                "INSERT INTO runner_state VALUES(?1,?2,?3)",
                params![
                    request.request_id,
                    execution_id,
                    json!({"state":"queued","role":"review"}).to_string()
                ],
            )?;
            Ok(json!({"operationId":request.request_id,"role":"review"}))
        }
        "runner.event" => {
            let operation = required(p, "operationId")?;
            let token = required(p, "token")?;
            let execution_id: String = tx.query_row(
                "SELECT execution_id FROM runner_state WHERE operation_id=?1",
                [operation],
                |r| r.get(0),
            )?;
            let (expected, roadmap) = tx.query_row(
                "SELECT token_digest,roadmap_id FROM executions WHERE id=?1",
                [&execution_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )?;
            if digest(token.as_bytes()) != expected {
                return Err(Error::new("invalid_owner", "Runner token does not match"));
            }
            let data = p
                .get("event")
                .ok_or_else(|| Error::new("invalid_request", "event is required"))?;
            let previous: String = tx.query_row(
                "SELECT body FROM runner_state WHERE operation_id=?1",
                [operation],
                |r| r.get(0),
            )?;
            let mut merged = parse(&previous)?;
            let values = data
                .as_object()
                .ok_or_else(|| Error::new("invalid_request", "event must be an object"))?;
            for (key, value) in values {
                merged[key] = value.clone();
            }
            tx.execute(
                "UPDATE runner_state SET body=?1 WHERE operation_id=?2",
                params![merged.to_string(), operation],
            )?;
            if let Some(thread) = data["threadId"].as_str() {
                tx.execute("UPDATE executions SET session_id=?1 WHERE id=?2 AND session_id LIKE 'codex:pending:%'",params![format!("codex:{thread}"),execution_id])?;
            }
            if matches!(
                data["state"].as_str(),
                Some("completed" | "failed" | "needs_human")
            ) {
                tx.execute(
                    "UPDATE operations SET state=?1,result=?2 WHERE id=?3",
                    params![data["state"].as_str(), data.to_string(), operation],
                )?;
                let body: String = tx.query_row(
                    "SELECT body FROM operations WHERE id=?1",
                    [operation],
                    |r| r.get(0),
                )?;
                if parse(&body)?["role"] != "review"
                    && matches!(data["state"].as_str(), Some("failed" | "needs_human"))
                {
                    tx.execute(
                        "UPDATE executions SET state='needs_human' WHERE id=?1 AND state='working'",
                        [&execution_id],
                    )?;
                }
            }
            let (project, _) = route(tx, &roadmap, None, true)?;
            event(
                tx,
                &project,
                "runner.event",
                &json!({"operationId":operation,"executionId":execution_id,"event":data}),
            )?;
            Ok(json!({"recorded":true}))
        }
        "runner.approval_reply" => {
            human(p)?;
            let operation = required(p, "operationId")?;
            let current: String = tx.query_row(
                "SELECT body FROM runner_state WHERE operation_id=?1",
                [operation],
                |r| r.get(0),
            )?;
            let current = parse(&current)?;
            if current["approval"]["id"].is_null() || current["approval"]["id"] != p["approvalId"] {
                return Err(Error::new(
                    "approval_changed",
                    "Pending approval does not match",
                ));
            }
            if !p["response"].is_object() {
                return Err(Error::new(
                    "invalid_request",
                    "Approval response must be an object",
                ));
            }
            let execution: String = tx.query_row(
                "SELECT execution_id FROM runner_state WHERE operation_id=?1",
                [operation],
                |r| r.get(0),
            )?;
            tx.execute("INSERT INTO notifications VALUES(?1,?2,?3,'pending')",params![id("approval-response"),execution,json!({"action":"approval_reply","operationId":operation,"approvalId":p["approvalId"],"response":p["response"]}).to_string()])?;
            Ok(json!({"queued":true}))
        }
        "runner.approval_take" => {
            let operation = required(p, "operationId")?;
            let token = required(p, "token")?;
            let (execution,expected)=tx.query_row("SELECT e.id,e.token_digest FROM runner_state s JOIN executions e ON e.id=s.execution_id WHERE s.operation_id=?1",[operation],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?)))?;
            if digest(token.as_bytes()) != expected {
                return Err(Error::new("invalid_owner", "Runner token mismatch"));
            }
            let found=tx.query_row("SELECT id,body FROM notifications WHERE execution_id=?1 AND state='pending' AND json_extract(body,'$.action')='approval_reply' AND json_extract(body,'$.operationId')=?2 ORDER BY rowid LIMIT 1",params![execution,operation],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional()?;
            if let Some((id, body)) = found {
                tx.execute("UPDATE notifications SET state='applied' WHERE id=?1", [id])?;
                Ok(parse(&body)?)
            } else {
                Ok(Value::Null)
            }
        }
        "node.claim" => {
            let roadmap = required(p, "roadmapId")?;
            let node_id = required(p, "nodeId")?;
            let expected = revision(p)?;
            let (project, _) = route(tx, roadmap, Some(expected), false)?;
            let (contract, hash, state) = node(tx, roadmap, node_id)?;
            if state != "ready" {
                return Err(Error::new(
                    if state == "working" || state == "submitted" {
                        "already_claimed"
                    } else {
                        "node_not_ready"
                    },
                    format!("Node state is {state}"),
                ));
            }
            dependencies_ready(tx, roadmap, &contract)?;
            let prepared = if contract["code"] == true {
                let body = tx
                    .query_row(
                        "SELECT body FROM workspaces WHERE roadmap_id=?1 AND node_id=?2",
                        params![roadmap, node_id],
                        |r| r.get::<_, String>(0),
                    )
                    .optional()?
                    .ok_or_else(|| {
                        Error::new("workspace_required", "Use node.prepare for code nodes")
                    })?;
                let body = parse(&body)?;
                let workspace = Path::new(required(&body, "workspacePath")?);
                if super::git::run(workspace, &["branch", "--show-current"])?
                    != required(&body, "branch")?
                {
                    return Err(Error::new("branch_changed", "Prepared branch changed"));
                }
                dependencies_in_code(
                    tx,
                    roadmap,
                    &contract,
                    workspace,
                    &super::git::head(workspace)?,
                )?;
                Some(body)
            } else {
                None
            };
            let execution_id = id("execution");
            let token = id("owner");
            let session = session_key(required(p, "sessionId")?);
            let workspace = prepared
                .as_ref()
                .and_then(|v| v["workspacePath"].as_str())
                .or_else(|| p["workspace"].as_str());
            tx.execute("INSERT INTO executions(id,roadmap_id,node_id,revision,contract_digest,session_id,token_digest,state,workspace) VALUES(?1,?2,?3,?4,?5,?6,?7,'working',?8)",params![execution_id,roadmap,node_id,expected,hash,session,digest(token.as_bytes()),workspace]).map_err(|e|Error::new("already_claimed",e.to_string()))?;
            if contract["code"] == true {
                let initial = super::git::head(Path::new(workspace.unwrap()))?;
                tx.execute(
                    "UPDATE executions SET evidence=?1 WHERE id=?2",
                    params![json!([{"commit":initial}]).to_string(), execution_id],
                )?;
            }
            tx.execute(
                "UPDATE nodes SET state='working' WHERE roadmap_id=?1 AND id=?2",
                params![roadmap, node_id],
            )?;
            event(
                tx,
                &project,
                "execution.claimed",
                &json!({"executionId":execution_id,"nodeId":node_id,"sessionId":session}),
            )?;
            let mut upstream = Vec::new();
            for dep in contract["dependencies"].as_array().into_iter().flatten() {
                let value=tx.query_row("SELECT h.id,h.digest,h.body FROM handoffs h JOIN executions e ON e.id=h.execution_id WHERE e.roadmap_id=?1 AND e.node_id=?2 AND h.state='accepted' ORDER BY h.rowid DESC LIMIT 1",params![roadmap,dep.as_str()],|row|Ok((row.get::<_,String>(0)?,row.get::<_,String>(1)?,row.get::<_,String>(2)?)))?;
                upstream.push(json!({"nodeId":dep,"handoffId":value.0,"digest":value.1,"body":parse(&value.2)?}));
            }
            let route_body: String = tx.query_row(
                "SELECT body FROM revisions WHERE roadmap_id=?1 AND revision=?2",
                params![roadmap, expected],
                |row| row.get(0),
            )?;
            let route_body = parse(&route_body)?;
            tx.execute(
                "UPDATE executions SET input_manifest=?1 WHERE id=?2",
                params![serde_json::to_string(&upstream).unwrap(), execution_id],
            )?;
            Ok(
                json!({"executionId":execution_id,"token":token,"revision":expected,"nodeContractDigest":hash,"contract":contract,"workspace":prepared,"upstream":upstream,"roadmap":{"title":route_body["title"],"goal":route_body["goal"],"constraints":route_body["constraints"]}}),
            )
        }
        "execution.progress" => {
            let (_, _, project, state, _) = execution(tx, p, false)?;
            if state != "working" {
                return Err(Error::new("execution_not_running", state));
            }
            required(p, "summary")?;
            let execution_id = required(p, "executionId")?;
            let evidence = p
                .get("evidence")
                .and_then(Value::as_array)
                .ok_or_else(|| Error::new("invalid_request", "evidence must be an array"))?;
            let (count, old) = tx.query_row(
                "SELECT no_progress,evidence FROM executions WHERE id=?1",
                [execution_id],
                |r| Ok((r.get::<_, u32>(0)?, r.get::<_, String>(1)?)),
            )?;
            let old: Value = parse(&old)?;
            let changed = !evidence.is_empty() && old != p["evidence"];
            let count = if changed { 0 } else { count + 1 };
            let action = match count {
                0 | 1 => "continue",
                2 => "reflect",
                _ => "needs_human",
            };
            tx.execute(
                "UPDATE executions SET no_progress=?1,evidence=?2,state=?3 WHERE id=?4",
                params![
                    count,
                    p["evidence"].to_string(),
                    if count >= 3 { "needs_human" } else { "working" },
                    execution_id
                ],
            )?;
            let result = json!({"executionId":execution_id,"noProgress":count,"action":action,"prompt":if count==2{REFLECTION_PROMPT}else{""}});
            event(tx, &project, "execution.progress", &result)?;
            Ok(result)
        }
        "execution.handoff" => {
            let (roadmap, node_id, project, state, _) = execution(tx, p, true)?;
            if state != "working" && state != "needs_human" {
                return Err(Error::new("execution_not_running", state));
            }
            let (contract, _, node_state) = node(tx, &roadmap, &node_id)?;
            if node_state == "frozen" || node_state == "recheck" || node_state == "withdrawn" {
                return Err(Error::new("version_conflict", "Node is frozen or changed"));
            }
            required(p, "summary")?;
            if contract["code"] == true {
                let body: String = tx.query_row(
                    "SELECT body FROM workspaces WHERE roadmap_id=?1 AND node_id=?2",
                    params![roadmap, node_id],
                    |r| r.get(0),
                )?;
                let body = parse(&body)?;
                let path = Path::new(required(&body, "workspacePath")?);
                super::git::clean(path)?;
                if super::git::head(path)? != required(p, "codeRevision")? {
                    return Err(Error::new(
                        "version_conflict",
                        "codeRevision differs from workspace HEAD",
                    ));
                }
            }
            let checks = p["verification"]
                .as_array()
                .ok_or_else(|| Error::new("incomplete_handoff", "verification is required"))?;
            for criterion in contract["acceptance"].as_array().unwrap() {
                if !checks.iter().any(|check| {
                    check["criterion"] == *criterion
                        && check["outcome"] == "passed"
                        && check["evidence"]
                            .as_str()
                            .is_some_and(|s| !s.trim().is_empty())
                }) {
                    return Err(Error::new(
                        "incomplete_handoff",
                        format!("Missing passing evidence for {criterion}"),
                    ));
                }
            }
            let artifacts = p["artifacts"]
                .as_array()
                .ok_or_else(|| Error::new("incomplete_handoff", "artifacts is required"))?;
            for artifact in artifacts {
                let path = Path::new(required(artifact, "path")?);
                if !path.is_absolute() {
                    return Err(Error::new(
                        "invalid_artifact",
                        "Artifact paths must be absolute",
                    ));
                }
                let bytes = std::fs::read(path)
                    .map_err(|e| Error::new("artifact_missing", e.to_string()))?;
                if digest(&bytes) != required(artifact, "sha256")? {
                    return Err(Error::new("artifact_changed", "Artifact digest differs"));
                }
            }
            let execution_id = required(p, "executionId")?;
            let handoff_id = id("handoff");
            let mut body = p.clone();
            body.as_object_mut().unwrap().remove("token");
            let inputs: String = tx.query_row(
                "SELECT input_manifest FROM executions WHERE id=?1",
                [execution_id],
                |row| row.get(0),
            )?;
            body["inputs"] = parse(&inputs)?;
            body["nodeContractDigest"] = json!(node(tx, &roadmap, &node_id)?.1);
            tx.execute(
                "INSERT INTO handoffs VALUES(?1,?2,?3,?4,'submitted')",
                params![
                    handoff_id,
                    execution_id,
                    body.to_string(),
                    digest(body.to_string().as_bytes())
                ],
            )?;
            tx.execute(
                "UPDATE executions SET state='submitted' WHERE id=?1",
                [execution_id],
            )?;
            tx.execute(
                "UPDATE nodes SET state='submitted' WHERE roadmap_id=?1 AND id=?2",
                params![roadmap, node_id],
            )?;
            event(
                tx,
                &project,
                "handoff.received",
                &json!({"handoffId":handoff_id,"executionId":execution_id}),
            )?;
            Ok(json!({"handoffId":handoff_id,"state":"submitted","persisted":true}))
        }
        "review.submit" => {
            let handoff = required(p, "handoffId")?;
            let reviewer = session_key(required(p, "reviewerSessionId")?);
            let verdict = required(p, "verdict")?;
            required(p, "summary")?;
            if !["approved", "changes_requested"].contains(&verdict) {
                return Err(Error::new("invalid_request", "Invalid review verdict"));
            }
            let owner:String=tx.query_row("SELECT e.session_id FROM handoffs h JOIN executions e ON e.id=h.execution_id WHERE h.id=?1",[handoff],|r|r.get(0))?;
            if owner == reviewer {
                return Err(Error::new(
                    "independent_review_required",
                    "Author cannot approve own handoff",
                ));
            }
            let review_id = id("review");
            tx.execute(
                "INSERT INTO reviews VALUES(?1,?2,?3,?4,?5)",
                params![review_id, handoff, reviewer, verdict, p.to_string()],
            )?;
            let (execution_id,roadmap,node_id,handoff_state)=tx.query_row("SELECT e.id,e.roadmap_id,e.node_id,h.state FROM handoffs h JOIN executions e ON e.id=h.execution_id WHERE h.id=?1",[handoff],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?;
            if handoff_state != "submitted" {
                return Err(Error::new(
                    "handoff_not_pending",
                    "Review target is no longer pending",
                ));
            }
            let (project, _) = route(tx, &roadmap, None, true)?;
            if verdict == "changes_requested" {
                tx.execute(
                    "UPDATE handoffs SET state='rejected' WHERE id=?1",
                    [handoff],
                )?;
                tx.execute(
                    "UPDATE executions SET state='working' WHERE id=?1 AND state='submitted'",
                    [&execution_id],
                )?;
                tx.execute("UPDATE nodes SET state='working' WHERE roadmap_id=?1 AND id=?2 AND state='submitted'",params![roadmap,node_id])?;
            }
            event(tx, &project, "review.submitted", p)?;
            Ok(json!({"reviewId":review_id,"verdict":verdict}))
        }
        "handoff.accept" | "handoff.reject" => {
            human(p)?;
            let handoff = required(p, "handoffId")?;
            let (execution_id,roadmap,node_id,state)=tx.query_row("SELECT e.id,e.roadmap_id,e.node_id,h.state FROM handoffs h JOIN executions e ON e.id=h.execution_id WHERE h.id=?1",[handoff],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?)))?;
            if state != "submitted" {
                return Err(Error::new("handoff_not_pending", state));
            }
            let (project, _) = route(tx, &roadmap, None, true)?;
            let (contract, hash, node_state) = node(tx, &roadmap, &node_id)?;
            let old_hash: String = tx.query_row(
                "SELECT contract_digest FROM executions WHERE id=?1",
                [&execution_id],
                |r| r.get(0),
            )?;
            if hash != old_hash || ["frozen", "recheck", "withdrawn"].contains(&node_state.as_str())
            {
                return Err(Error::new(
                    "version_conflict",
                    "Handoff requires revalidation",
                ));
            }
            let accept = request.command == "handoff.accept";
            if accept {
                let receipt: String =
                    tx.query_row("SELECT body FROM handoffs WHERE id=?1", [handoff], |r| {
                        r.get(0)
                    })?;
                super::git::verify_artifacts(&parse(&receipt)?)?;
            }
            if accept && (contract["code"] == true || contract["requiresReview"] == true) {
                let verdict=tx.query_row("SELECT verdict FROM reviews WHERE handoff_id=?1 ORDER BY rowid DESC LIMIT 1",[handoff],|r|r.get::<_,String>(0)).optional()?;
                if verdict.as_deref() != Some("approved") {
                    return Err(Error::new(
                        "review_required",
                        "Independent approved review is required",
                    ));
                }
            }
            if !accept {
                required(p, "reason")?;
            }
            let next = if accept { "accepted" } else { "rejected" };
            tx.execute(
                "UPDATE handoffs SET state=?1 WHERE id=?2",
                params![next, handoff],
            )?;
            tx.execute(
                "UPDATE executions SET state=?1 WHERE id=?2",
                params![if accept { "completed" } else { "working" }, execution_id],
            )?;
            tx.execute(
                "UPDATE nodes SET state=?1 WHERE roadmap_id=?2 AND id=?3",
                params![
                    if accept {
                        if contract["code"] == true {
                            "accepted_pending_integration"
                        } else {
                            "accepted"
                        }
                    } else {
                        "working"
                    },
                    roadmap,
                    node_id
                ],
            )?;
            event(
                tx,
                &project,
                if accept {
                    "handoff.accepted"
                } else {
                    "handoff.rejected"
                },
                p,
            )?;
            if accept {
                if let Some(selected) = p["continueNodes"].as_array() {
                    for selected in selected {
                        let selected = selected.as_str().ok_or_else(|| {
                            Error::new("invalid_request", "continueNodes contains node IDs")
                        })?;
                        let (child, hash, _) = node(tx, &roadmap, selected)?;
                        if !child["dependencies"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .any(|dep| dep == &node_id)
                        {
                            return Err(Error::new(
                                "scope_conflict",
                                "Selected node must directly depend on the accepted node",
                            ));
                        }
                        let workspace = p["workspace"].as_str().map(str::to_owned).or_else(|| {
                            tx.query_row(
                                "SELECT workspace FROM executions WHERE id=?1",
                                [&execution_id],
                                |r| r.get::<_, Option<String>>(0),
                            )
                            .ok()
                            .flatten()
                        });
                        tx.execute("INSERT INTO grants VALUES(?1,?2,?3,?4,?5,'waiting')",params![id("grant"),roadmap,selected,hash,json!({"sourceHandoffId":handoff,"workspace":workspace,"model":p["model"],"expectedRevision":revision(p).ok()}).to_string()])?;
                    }
                }
            }
            Ok(json!({"handoffId":handoff,"state":next,"downstreamStarted":false}))
        }
        "authorization.grant" => {
            human(p)?;
            let roadmap = required(p, "roadmapId")?;
            let node_id = required(p, "nodeId")?;
            let (project, _) = route(tx, roadmap, Some(revision(p)?), true)?;
            let (_, hash, _) = node(tx, roadmap, node_id)?;
            let grant = id("grant");
            tx.execute(
                "INSERT INTO grants VALUES(?1,?2,?3,?4,?5,'waiting')",
                params![grant, roadmap, node_id, hash, p.to_string()],
            )?;
            event(
                tx,
                &project,
                "authorization.granted",
                &json!({"grantId":grant,"nodeId":node_id}),
            )?;
            Ok(json!({"grantId":grant,"state":"waiting"}))
        }
        "control.set" => {
            human(p)?;
            let project = required(p, "projectId")?;
            let state = required(p, "state")?;
            if !["active", "paused", "stopped"].contains(&state) {
                return Err(Error::new("invalid_request", "Invalid control state"));
            }
            if tx.execute(
                "UPDATE projects SET control=?1 WHERE id=?2",
                params![state, project],
            )? != 1
            {
                return Err(Error::new("not_found", "Project not found"));
            }
            if state != "active" {
                notify_project(tx, project, "Project paused", None)?;
            }
            event(tx, project, "control.changed", p)?;
            Ok(json!({"projectId":project,"control":state}))
        }
        "roadmap.propose" => {
            let roadmap = required(p, "roadmapId")?;
            let (project, current) = route(tx, roadmap, Some(revision(p)?), true)?;
            required(p, "reason")?;
            let proposal_id = id("proposal");
            let all = {
                let mut stmt =
                    tx.prepare("SELECT id,contract,state FROM nodes WHERE roadmap_id=?1")?;
                let values = stmt
                    .query_map([roadmap], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, String>(2)?,
                        ))
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                values
            };
            let mut affected: HashSet<String> = p["affectedNodes"]
                .as_array()
                .map(|ids| {
                    ids.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_else(|| all.iter().map(|(id, _, _)| id.clone()).collect());
            if affected.is_empty()
                || affected
                    .iter()
                    .any(|id| !all.iter().any(|(old, _, _)| old == id))
            {
                return Err(Error::new(
                    "invalid_request",
                    "affectedNodes must name existing nodes",
                ));
            }
            loop {
                let old = affected.len();
                for (id, body, _) in &all {
                    let body = parse(body)?;
                    if body["dependencies"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|dep| dep.as_str().is_some_and(|d| affected.contains(d)))
                    {
                        affected.insert(id.clone());
                    }
                }
                if old == affected.len() {
                    break;
                }
            }
            let mut proposal = p.clone();
            let mut states = serde_json::Map::new();
            for (node_id, _, state) in &all {
                if affected.contains(node_id) {
                    states.insert(node_id.clone(), json!(state));
                    tx.execute(
                        "UPDATE nodes SET state='frozen' WHERE roadmap_id=?1 AND id=?2",
                        params![roadmap, node_id],
                    )?;
                    let executions = {
                        let mut stmt=tx.prepare("SELECT id FROM executions WHERE roadmap_id=?1 AND node_id=?2 AND state IN ('working','needs_human','submitted')")?;
                        let ids = stmt
                            .query_map(params![roadmap, node_id], |r| r.get::<_, String>(0))?
                            .collect::<rusqlite::Result<Vec<_>>>()?;
                        ids
                    };
                    for execution in executions {
                        tx.execute(
                            "UPDATE executions SET state='pause_requested' WHERE id=?1",
                            [&execution],
                        )?;
                        tx.execute("INSERT INTO notifications VALUES(?1,?2,?3,'pending')",params![id("notification"),execution,json!({"action":"pause","reason":p["reason"],"proposalId":proposal_id}).to_string()])?;
                    }
                }
            }
            proposal["previousStates"] = Value::Object(states);
            tx.execute(
                "INSERT INTO proposals VALUES(?1,?2,?3,?4,'open')",
                params![proposal_id, roadmap, current, proposal.to_string()],
            )?;
            event(
                tx,
                &project,
                "roadmap.proposed",
                &json!({"proposalId":proposal_id,"roadmapId":roadmap,"reason":p["reason"]}),
            )?;
            Ok(json!({"proposalId":proposal_id,"state":"open","affectedRoadmap":roadmap}))
        }
        "execution.pause_ack" => {
            let (_, _, project, _, _) = execution(tx, p, true)?;
            required(p, "summary")?;
            let execution_id = required(p, "executionId")?;
            if tx.execute(
                "UPDATE executions SET state='paused' WHERE id=?1 AND state='pause_requested'",
                [execution_id],
            )? != 1
            {
                return Err(Error::new(
                    "pause_not_requested",
                    "Execution has no pending pause request",
                ));
            }
            tx.execute("UPDATE notifications SET state='applied' WHERE execution_id=?1 AND state='pending'",[execution_id])?;
            event(
                tx,
                &project,
                "execution.paused",
                &json!({"executionId":execution_id,"summary":p["summary"]}),
            )?;
            Ok(json!({"executionId":execution_id,"state":"paused"}))
        }
        "execution.resume" => {
            human(p)?;
            let (roadmap, node_id, project, state, _) = execution(tx, p, false)?;
            if state != "paused" && state != "needs_human" {
                return Err(Error::new("execution_not_paused", state));
            }
            let (_, _, node_state) = node(tx, &roadmap, &node_id)?;
            if ["frozen", "withdrawn", "recheck"].contains(&node_state.as_str()) {
                return Err(Error::new(
                    "version_conflict",
                    "Resolve roadmap changes before resuming",
                ));
            }
            tx.execute(
                "UPDATE executions SET state='working',revision=?1,no_progress=0 WHERE id=?2",
                params![revision(p)?, required(p, "executionId")?],
            )?;
            event(
                tx,
                &project,
                "execution.resumed",
                &json!({"executionId":p["executionId"]}),
            )?;
            Ok(json!({"state":"working"}))
        }
        "execution.release" => {
            human(p)?;
            let (roadmap, node_id, project, state, _) = execution(tx, p, true)?;
            required(p, "reason")?;
            if state != "paused" {
                return Err(Error::new(
                    "pause_required",
                    "Confirm safe pause before releasing a writer",
                ));
            }
            tx.execute(
                "UPDATE executions SET state='cancelled' WHERE id=?1",
                [required(p, "executionId")?],
            )?;
            let (_, _, state) = node(tx, &roadmap, &node_id)?;
            if state != "withdrawn" && state != "frozen" {
                tx.execute(
                    "UPDATE nodes SET state='ready' WHERE roadmap_id=?1 AND id=?2",
                    params![roadmap, node_id],
                )?;
            }
            event(
                tx,
                &project,
                "execution.released",
                &json!({"executionId":p["executionId"],"reason":p["reason"]}),
            )?;
            Ok(json!({"state":"cancelled"}))
        }
        "node.revalidate" => {
            human(p)?;
            let roadmap = required(p, "roadmapId")?;
            let node_id = required(p, "nodeId")?;
            let (project, _) = route(tx, roadmap, Some(revision(p)?), true)?;
            required(p, "reason")?;
            if tx.query_row("SELECT 1 FROM executions WHERE roadmap_id=?1 AND node_id=?2 AND state IN ('working','needs_human','pause_requested','paused','submitted')",params![roadmap,node_id],|_|Ok(())).optional()?.is_some(){return Err(Error::new("active_writer","Release the original safely paused writer first"));}
            let decision = required(p, "decision")?;
            let state =
                match decision {
                    "redo" => "ready",
                    "withdraw" => "withdrawn",
                    _ => return Err(Error::new(
                        "invalid_request",
                        "decision must be redo or withdraw; reuse requires a new verified handoff",
                    )),
                };
            tx.execute(
                "UPDATE nodes SET state=?1 WHERE roadmap_id=?2 AND id=?3",
                params![state, roadmap, node_id],
            )?;
            event(tx, &project, "node.revalidated", p)?;
            Ok(json!({"state":state}))
        }
        _ => Err(Error::new(
            "unknown_workflow_command",
            format!("Unsupported workflow command {}", request.command),
        )),
    }
}

fn notify_project(
    tx: &Transaction<'_>,
    project: &str,
    reason: &str,
    roadmap: Option<&str>,
) -> Result<()> {
    let ids = {
        let mut stmt=tx.prepare("SELECT e.id FROM executions e JOIN roadmaps r ON r.id=e.roadmap_id WHERE r.project_id=?1 AND (?2 IS NULL OR r.id=?2) AND e.state IN ('working','needs_human')")?;
        let values = stmt
            .query_map(params![project, roadmap], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        values
    };
    for execution in ids {
        tx.execute(
            "UPDATE executions SET state='pause_requested' WHERE id=?1",
            [&execution],
        )?;
        tx.execute(
            "INSERT INTO notifications VALUES(?1,?2,?3,'pending')",
            params![
                id("notification"),
                execution,
                json!({"action":"pause","reason":reason}).to_string()
            ],
        )?;
    }
    Ok(())
}
