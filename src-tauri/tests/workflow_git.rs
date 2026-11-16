use agentwatcher_lib::workflow::{git, store::Store, Request};
use serde_json::{json, Value};
use std::path::PathBuf;

#[test]
fn workflow_git_requires_real_code_and_serializes_target_updates() {
    let root = std::env::temp_dir().join(format!("aw-workflow-git-{}", uuid::Uuid::new_v4()));
    let repo = root.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    git::run(&repo, &["init", "-b", "dev"]).unwrap();
    git::run(&repo, &["config", "user.name", "Workflow Test"]).unwrap();
    git::run(&repo, &["config", "user.email", "workflow@example.invalid"]).unwrap();
    std::fs::write(repo.join("base.txt"), "base").unwrap();
    git::run(&repo, &["add", "."]).unwrap();
    git::run(&repo, &["commit", "-m", "base"]).unwrap();
    let mut store = Store::open(&root.join("state")).unwrap();
    let mut n = 0;
    let mut call = |command: &str, payload: Value| {
        n += 1;
        store.apply(&Request {
            schema_version: 1,
            request_id: format!("r{n}"),
            command: command.into(),
            payload,
        })
    };
    call("project.create", json!({"id":"p","title":"project"})).unwrap();
    call("roadmap.publish",json!({"id":"r","projectId":"p","title":"route","expectedRevision":0,"humanConfirmed":true,"nodes":[
        {"id":"a","title":"code","goal":"write result","acceptance":["result committed"],"code":true},
        {"id":"b","title":"dependent","goal":"use result","acceptance":["result available"],"dependencies":["a"]}
    ]})).unwrap();
    assert_eq!(
        call(
            "node.claim",
            json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":"codex:author"})
        )
        .unwrap_err()
        .code,
        "workspace_required"
    );
    let workspace=call("node.prepare",json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"repositoryPath":repo,"targetBranch":"dev"})).unwrap();
    let worktree = PathBuf::from(workspace["workspacePath"].as_str().unwrap());
    let claim = call(
        "node.claim",
        json!({"roadmapId":"r","nodeId":"a","expectedRevision":1,"sessionId":"codex:author"}),
    )
    .unwrap();
    std::fs::write(worktree.join("result.txt"), "result").unwrap();
    git::run(&worktree, &["add", "."]).unwrap();
    git::run(&worktree, &["commit", "-m", "result"]).unwrap();
    let revision = git::head(&worktree).unwrap();
    let handoff=call("execution.handoff",json!({"executionId":claim["executionId"],"token":claim["token"],"expectedRevision":1,"summary":"committed result","codeRevision":revision,"verification":[{"criterion":"result committed","outcome":"passed","evidence":"git commit"}],"artifacts":[]})).unwrap();
    assert_eq!(
        call(
            "handoff.accept",
            json!({"handoffId":handoff["handoffId"],"humanConfirmed":true})
        )
        .unwrap_err()
        .code,
        "review_required"
    );
    assert_eq!(call("review.submit",json!({"handoffId":handoff["handoffId"],"reviewerSessionId":"codex:author","verdict":"approved","summary":"self"})).unwrap_err().code,"independent_review_required");
    call("review.submit",json!({"handoffId":handoff["handoffId"],"reviewerSessionId":"codex:reviewer","verdict":"approved","summary":"reviewed exact handoff"})).unwrap();
    call(
        "handoff.accept",
        json!({"handoffId":handoff["handoffId"],"humanConfirmed":true}),
    )
    .unwrap();
    assert_eq!(call("node.claim",json!({"roadmapId":"r","nodeId":"b","expectedRevision":1,"sessionId":"codex:downstream"})).unwrap_err().code,"dependencies_not_ready");
    let candidate = call(
        "node.integrate",
        json!({"handoffId":handoff["handoffId"],"expectedRevision":1,"humanConfirmed":true}),
    )
    .unwrap();
    assert_eq!(candidate["requiresValidation"], false);
    call("integration.publish",json!({"candidateOperationId":candidate["operationId"],"expectedRevision":1,"humanConfirmed":true})).unwrap();
    let ready = call(
        "node.claim",
        json!({"roadmapId":"r","nodeId":"b","expectedRevision":1,"sessionId":"codex:downstream"}),
    )
    .unwrap();
    assert!(ready["executionId"].is_string());
    let integration = format!(
        "refs/heads/{}",
        workspace["integrationBranch"].as_str().unwrap()
    );
    assert_eq!(
        git::run(&repo, &["rev-parse", &integration]).unwrap(),
        revision
    );
    assert_ne!(
        git::head(&repo).unwrap(),
        revision,
        "project branch must remain unchanged"
    );
    let milestone=call("milestone.check",json!({"id":"m","projectId":"p","nodes":[{"roadmapId":"r","nodeId":"a","expectedRevision":1}]})).unwrap();
    assert_eq!(milestone["ready"], true);
    let target = git::head(&repo).unwrap();
    let landed=call("milestone.land",json!({"milestoneId":"m","humanConfirmed":true,"verifiedRepositoryVersions":milestone["repositoryVersions"],"expectedTargets":[{"repositoryPath":milestone["repositoryVersions"][0]["repositoryPath"],"revision":target}]})).unwrap();
    assert_eq!(landed["landed"], true, "{landed}");
    assert_eq!(git::head(&repo).unwrap(), revision);
    assert_eq!(
        std::fs::read_to_string(repo.join("result.txt")).unwrap(),
        "result"
    );
    drop(call);
    drop(store);
    // The fixture owns all paths, and Git worktrees must be removed through Git.
    git::run(
        &repo,
        &[
            "worktree",
            "remove",
            candidate["candidatePath"].as_str().unwrap(),
        ],
    )
    .unwrap();
    git::run(&repo, &["worktree", "remove", &worktree.to_string_lossy()]).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
