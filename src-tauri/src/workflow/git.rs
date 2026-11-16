use super::{digest, required, Error, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run(repo: &Path, args: &[&str]) -> Result<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo).args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.output()?;
    if !output.status.success() {
        return Err(Error::new(
            "git_failed",
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
fn branch(repo: &Path, name: &str) -> Result<()> {
    if name.starts_with('-') || name.starts_with("refs/") {
        return Err(Error::new(
            "invalid_branch",
            "Use a branch name, not a revision expression",
        ));
    }
    run(repo, &["check-ref-format", "--branch", name])?;
    Ok(())
}
pub fn head(repo: &Path) -> Result<String> {
    run(repo, &["rev-parse", "--verify", "HEAD^{commit}"])
}
pub fn identity(repo: &Path) -> Result<String> {
    if let Ok(remote) = run(repo, &["remote", "get-url", "origin"]) {
        return Ok(remote
            .trim_end_matches(".git")
            .replace('\\', "/")
            .to_ascii_lowercase());
    }
    Ok(run(
        repo,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?
    .replace('\\', "/")
    .to_ascii_lowercase())
}
pub fn contains(repo: &Path, base: &str, required: &str) -> Result<()> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["merge-base", "--is-ancestor", required, base])
        .output()?;
    if !output.status.success() {
        return Err(Error::new(
            "upstream_code_unavailable",
            format!("Required upstream commit {required} is not in {base}"),
        ));
    }
    Ok(())
}
pub fn clean(repo: &Path) -> Result<()> {
    if !run(repo, &["status", "--porcelain", "--untracked-files=all"])?.is_empty() {
        return Err(Error::new(
            "dirty_workspace",
            "Workspace contains uncommitted changes; nothing was stashed or removed",
        ));
    }
    Ok(())
}
pub fn prepare(home: &Path, p: &Value) -> Result<Value> {
    let repo = std::fs::canonicalize(required(p, "repositoryPath")?)?;
    let top = std::fs::canonicalize(run(&repo, &["rev-parse", "--show-toplevel"])?)?;
    if repo != top {
        return Err(Error::new(
            "invalid_repository",
            "repositoryPath must be a Git checkout root",
        ));
    }
    let roadmap = required(p, "roadmapId")?;
    let node = required(p, "nodeId")?;
    let target = required(p, "targetBranch")?;
    branch(&repo, target)?;
    let key = digest(format!("{roadmap}\n{node}\n{}", repo.display()).as_bytes());
    let route_key = digest(roadmap.as_bytes());
    let integration = format!("codex/roadmap/{}", &route_key[..16]);
    let node_branch = format!("codex/node/{}", &key[..20]);
    let target_ref = format!("refs/heads/{target}");
    let integration_ref = format!("refs/heads/{integration}");
    let target_sha = run(
        &repo,
        &["rev-parse", "--verify", &format!("{target_ref}^{{commit}}")],
    )?;
    let base = match run(
        &repo,
        &[
            "rev-parse",
            "--verify",
            &format!("{integration_ref}^{{commit}}"),
        ],
    ) {
        Ok(base) => base,
        Err(_) => {
            run(
                &repo,
                &[
                    "update-ref",
                    &integration_ref,
                    &target_sha,
                    "0000000000000000000000000000000000000000",
                ],
            )?;
            target_sha
        }
    };
    let root = home.join("workspaces").join(&key[..20]);
    if root.exists() {
        return Err(Error::new(
            "workspace_exists",
            format!(
                "{} already exists; reconcile the original prepare operation",
                root.display()
            ),
        ));
    }
    std::fs::create_dir_all(root.parent().unwrap())?;
    run(
        &repo,
        &[
            "worktree",
            "add",
            "-b",
            &node_branch,
            &root.to_string_lossy(),
            &base,
        ],
    )?;
    Ok(
        json!({"roadmapId":roadmap,"nodeId":node,"repositoryPath":repo,"workspacePath":root,"branch":node_branch,"baseRevision":base,"integrationBranch":integration,"targetBranch":target,"owned":true}),
    )
}

pub fn candidate(home: &Path, workspace: &Value, operation: &str, source: &str) -> Result<Value> {
    let repo = Path::new(required(workspace, "repositoryPath")?);
    let worktree = Path::new(required(workspace, "workspacePath")?);
    clean(worktree)?;
    if head(worktree)? != source {
        return Err(Error::new(
            "version_conflict",
            "Workspace HEAD differs from accepted handoff",
        ));
    }
    if run(worktree, &["branch", "--show-current"])? != required(workspace, "branch")? {
        return Err(Error::new("branch_changed", "Workspace branch changed"));
    }
    let reference = format!("refs/heads/{}", required(workspace, "integrationBranch")?);
    let old = run(
        repo,
        &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
    )?;
    let path = home
        .join("integrations")
        .join(&digest(operation.as_bytes())[..20]);
    if path.exists() {
        return Err(Error::new(
            "operation_uncertain",
            "Candidate already exists; reconcile instead of retrying",
        ));
    }
    std::fs::create_dir_all(path.parent().unwrap())?;
    run(
        repo,
        &["worktree", "add", "--detach", &path.to_string_lossy(), &old],
    )?;
    let ff = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["merge-base", "--is-ancestor", &old, source])
        .status()?
        .success();
    let merge = if ff {
        run(&path, &["merge", "--ff-only", source])
    } else {
        run(&path, &["merge", "--no-edit", "--no-ff", source])
    };
    if let Err(error) = merge {
        return Err(Error::new(
            "integration_conflict",
            format!(
                "{}; candidate preserved at {}",
                error.message,
                path.display()
            ),
        ));
    }
    run(&path, &["diff", "--check", &old, "HEAD"])?;
    let candidate = head(&path)?;
    Ok(
        json!({"candidateRevision":candidate,"previousTarget":old,"targetRef":reference,"repositoryPath":repo,"candidatePath":path,"sourceRevision":source,"requiresValidation":!ff,"state":"candidate"}),
    )
}

pub fn publish(candidate: &Value) -> Result<Value> {
    let repo = Path::new(required(candidate, "repositoryPath")?);
    let path = Path::new(required(candidate, "candidatePath")?);
    clean(path)?;
    if head(path)? != required(candidate, "candidateRevision")? {
        return Err(Error::new(
            "candidate_changed",
            "Candidate changed after verification",
        ));
    }
    run(
        repo,
        &[
            "update-ref",
            required(candidate, "targetRef")?,
            required(candidate, "candidateRevision")?,
            required(candidate, "previousTarget")?,
        ],
    )
    .map_err(|e| Error::new("target_advanced", e.message))?;
    let mut result = candidate.clone();
    result["state"] = json!("integrated");
    Ok(result)
}

pub fn verify_artifacts(body: &Value) -> Result<()> {
    for artifact in body["artifacts"].as_array().into_iter().flatten() {
        let path = PathBuf::from(
            artifact["restoredPath"]
                .as_str()
                .unwrap_or(required(artifact, "path")?),
        );
        if !path.is_absolute() {
            return Err(Error::new(
                "invalid_artifact",
                "Absolute artifact path required",
            ));
        }
        if digest(&std::fs::read(path)?) != required(artifact, "sha256")? {
            return Err(Error::new(
                "artifact_changed",
                "Artifact no longer matches its receipt",
            ));
        }
    }
    Ok(())
}

pub fn land(version: &Value, expected_target: &str) -> Result<Value> {
    let repo = Path::new(required(version, "repositoryPath")?);
    let target = required(version, "targetBranch")?;
    branch(repo, target)?;
    let reference = format!("refs/heads/{target}");
    if run(repo, &["rev-parse", "--verify", &reference])? != expected_target {
        return Err(Error::new(
            "target_advanced",
            "Project branch advanced after milestone approval",
        ));
    }
    let integration = format!("refs/heads/{}", required(version, "integrationBranch")?);
    let commit = required(version, "revision")?;
    if run(repo, &["rev-parse", "--verify", &integration])? != commit {
        return Err(Error::new(
            "integration_advanced",
            "Integration branch changed after milestone validation",
        ));
    }
    let ancestor = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["merge-base", "--is-ancestor", expected_target, commit])
        .status()?
        .success();
    if !ancestor {
        return Err(Error::new("integration_requires_rebase","Milestone cannot fast-forward project branch; prepare and verify a new integration candidate"));
    }
    let list = run(repo, &["worktree", "list", "--porcelain"])?;
    let mut path = None;
    let mut checked_out = None;
    for line in list.lines() {
        if let Some(value) = line.strip_prefix("worktree ") {
            path = Some(PathBuf::from(value));
        }
        if line == format!("branch {reference}") {
            checked_out = path.clone();
        }
    }
    if let Some(path) = checked_out {
        clean(&path)?;
        if head(&path)? != expected_target {
            return Err(Error::new(
                "target_advanced",
                "Checked-out target differs from approval",
            ));
        }
        run(&path, &["merge", "--ff-only", commit])?;
    } else {
        run(repo, &["update-ref", &reference, commit, expected_target])?;
    }
    Ok(json!({"repositoryPath":repo,"targetBranch":target,"revision":commit,"landed":true}))
}
