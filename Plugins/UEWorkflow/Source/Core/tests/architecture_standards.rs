use std::fs;
use std::path::{Path, PathBuf};

fn plugin_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("Core manifest should live under Source/Core")
        .to_path_buf()
}

fn agentwatcher_root() -> PathBuf {
    plugin_root()
        .parent()
        .and_then(Path::parent)
        .expect("UEWorkflow plugin should live under AgentWatcher/Plugins")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    fs::read_to_string(plugin_root().join(relative)).expect(relative)
}

#[test]
fn required_architecture_documents_exist() {
    for relative in [
        "Docs/Architecture/00-overview.md",
        "Docs/Architecture/10-directory-layout.md",
        "Docs/Architecture/20-dependency-matrix.md",
        "Docs/Standards/command-contract.md",
        "Docs/Standards/local-config.md",
        "Docs/Standards/knowledge-scope.md",
    ] {
        assert!(plugin_root().join(relative).is_file(), "{relative} missing");
    }
}

#[test]
fn required_schemas_exist() {
    for relative in [
        "Config/schemas/command.schema.json",
        "Config/schemas/module.schema.json",
        "Config/schemas/result.schema.json",
        "Config/schemas/knowledge-scope.schema.json",
    ] {
        let content = read(relative);
        assert!(
            content.contains("\"$schema\""),
            "{relative} has no schema marker"
        );
        assert!(content.contains("\"title\""), "{relative} has no title");
    }
}

#[test]
fn workspace_members_use_unreal_source_layout() {
    let cargo = read("Cargo.toml");
    for member in [
        "Source/Core",
        "Source/DevFlow",
        "Source/AgentHub",
        "Source/KnowledgeBase",
        "Source/UnrealMaster",
        "Source/Cli",
    ] {
        assert!(cargo.contains(member), "workspace missing {member}");
    }
}

#[test]
fn dependency_direction_matches_architecture_matrix() {
    let core = read("Source/Core/Cargo.toml");
    assert!(
        !core.contains("[dependencies]"),
        "Core must stay contract-only and dependency-free for now"
    );

    for (module, allowed) in [
        ("Source/DevFlow/Cargo.toml", "uwf-core"),
        ("Source/AgentHub/Cargo.toml", "uwf-core"),
        ("Source/KnowledgeBase/Cargo.toml", "uwf-core"),
    ] {
        let manifest = read(module);
        let dependencies = dependencies_section(&manifest);
        assert!(
            dependencies.contains(allowed),
            "{module} must depend on Core"
        );
        assert!(
            !dependencies.contains("uwf-devflow")
                && !dependencies.contains("uwf-agenthub")
                && !dependencies.contains("uwf-knowledgebase"),
            "{module} must not depend on peer business modules"
        );
    }

    let cli = read("Source/Cli/Cargo.toml");
    let cli_dependencies = dependencies_section(&cli);
    assert!(cli_dependencies.contains("uwf-unrealmaster"));
    assert!(!cli_dependencies.contains("uwf-devflow"));
    assert!(!cli_dependencies.contains("uwf-agenthub"));
    assert!(!cli_dependencies.contains("uwf-knowledgebase"));
}

#[test]
fn committed_contract_files_do_not_embed_local_absolute_project_roots() {
    for relative in [
        "UnrealWorkflow.uwplugin.json",
        "AgentWatcher.awplugin.json",
        "AGENTS.md",
        "Adapters/DevFlow/README.md",
        "Adapters/AgentHub/README.md",
        "Adapters/KnowledgeBase/README.md",
        "Docs/Architecture/00-overview.md",
        "Docs/Architecture/10-directory-layout.md",
        "Docs/Standards/local-config.md",
    ] {
        let content = read(relative);
        let local_root_slash = ["F:/", "AiProject"].concat();
        assert!(
            !content.contains(&local_root_slash),
            "{relative} leaks local root"
        );
        let local_root_backslash = ["F:\\", "AiProject"].concat();
        assert!(
            !content.contains(&local_root_backslash),
            "{relative} leaks local root"
        );
        for legacy in private_impl_forbidden_fragments() {
            assert!(!content.contains(legacy), "{relative} leaks {legacy}");
        }
    }
}

#[test]
fn public_agentwatcher_docs_do_not_expose_bridge_impl_or_local_roots() {
    let mut files = vec![agentwatcher_root().join("README.md")];
    collect_markdown_files(&agentwatcher_root().join("docs"), &mut files);

    let forbidden = public_doc_forbidden_fragments();
    for path in files {
        let content = fs::read_to_string(&path).expect("public doc should be readable");
        let relative = path
            .strip_prefix(agentwatcher_root())
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        for fragment in &forbidden {
            assert!(
                !content.contains(fragment),
                "{relative} leaks public-doc forbidden fragment"
            );
        }
    }
}

#[test]
fn public_source_files_do_not_expose_private_impl_names() {
    let mut files = Vec::new();
    collect_source_files(
        &agentwatcher_root().join("src-tauri").join("src"),
        &mut files,
    );
    collect_source_files(&plugin_root(), &mut files);

    let mut forbidden = public_doc_forbidden_fragments();
    forbidden.extend(
        private_impl_forbidden_fragments()
            .into_iter()
            .map(str::to_string),
    );

    for path in files {
        let content = fs::read_to_string(&path).expect("public source should be readable");
        let relative = path
            .strip_prefix(agentwatcher_root())
            .unwrap_or(path.as_path())
            .to_string_lossy()
            .replace('\\', "/");
        for fragment in &forbidden {
            assert!(
                !content.contains(fragment),
                "{relative} leaks private implementation fragment"
            );
        }
    }
}

fn collect_markdown_files(root: &Path, files: &mut Vec<PathBuf>) {
    if !root.is_dir() {
        return;
    }
    for entry in fs::read_dir(root).expect("docs directory should be readable") {
        let entry = entry.expect("docs entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_markdown_files(&path, files);
        } else if path.extension().and_then(|value| value.to_str()) == Some("md") {
            files.push(path);
        }
    }
}

fn collect_source_files(root: &Path, files: &mut Vec<PathBuf>) {
    if !root.is_dir() {
        return;
    }
    for entry in fs::read_dir(root).expect("source directory should be readable") {
        let entry = entry.expect("source entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if matches!(name, "target" | "Modules" | "UnrealWorkflowKnowledge") {
                continue;
            }
            collect_source_files(&path, files);
            continue;
        }
        let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if matches!(extension, "rs" | "json" | "toml" | "md") {
            files.push(path);
        }
    }
}

fn public_doc_forbidden_fragments() -> Vec<String> {
    vec![
        ["vscode-agentwatcher", "-bridge"].concat(),
        [
            "agentwatcher.",
            "agentwatcher-vscode",
            "-session",
            "-bridge",
        ]
        .concat(),
        ["agentwatcher-vscode-session", "-bridge"].concat(),
        ["agentwatcher", "-bridge"].concat(),
        ["safe", "1"].concat(),
        ["safe", "2"].concat(),
        ["safe", "3"].concat(),
        ["safe", "4"].concat(),
        ["F:/", "AiProject"].concat(),
        ["F:\\", "AiProject"].concat(),
    ]
}

fn private_impl_forbidden_fragments() -> Vec<&'static str> {
    vec![
        concat!("Unreal", "DevFlow"),
        concat!("UE", "_", "Master", "_", "Agent"),
        concat!("UE5", "_", "KnowledgeBase", "Maker"),
        concat!("UE", "Master", "Agent"),
        concat!("UE5", "KnowledgeBase", "Maker"),
        concat!("I_UNDERSTAND_THIS_CAN_CREATE_", "AESWORLD", "_WORKTREE"),
    ]
}

fn dependencies_section(manifest: &str) -> &str {
    manifest
        .split_once("[dependencies]")
        .map(|(_, rest)| rest)
        .unwrap_or("")
        .split_once("\n[")
        .map(|(dependencies, _)| dependencies)
        .unwrap_or_else(|| {
            manifest
                .split_once("[dependencies]")
                .map(|(_, rest)| rest)
                .unwrap_or("")
        })
}
