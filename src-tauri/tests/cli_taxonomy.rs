// 分类学测试：只验命令解析层，不起二进制、不碰数据源。
// cli.rs 经 #[path] 直接引入，保持定义层零依赖。

#[path = "../src/bin/agentwatcher-cli/cli.rs"]
mod cli;

use clap::{CommandFactory, Parser};
use cli::{
    Cli, Commands, HandoffAction, ImagegenAction, OutputFormat, SessionAction, SkillAction,
    SuggestAction,
};

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("agentwatcher-cli").chain(args.iter().copied()))
}

#[test]
fn session_list_parses_filters() {
    let parsed = parse(&[
        "session",
        "list",
        "--status",
        "waiting",
        "--provider",
        "zcode",
        "--provider",
        "opencode",
        "--limit",
        "5",
        "--active-days",
        "3",
        "--workspace",
        "F:\\demo",
    ])
    .expect("filters should parse");
    let Commands::Session {
        action: SessionAction::List {
            provider,
            status,
            workspace,
            limit,
            active_days,
            include_archived,
            exclude_playground,
            no_cache,
        },
    } = parsed.command
    else {
        panic!("expected session list");
    };
    assert_eq!(provider, vec!["zcode".to_string(), "opencode".to_string()]);
    assert_eq!(status.as_deref(), Some("waiting"));
    assert_eq!(workspace.as_deref(), Some("F:\\demo"));
    assert_eq!(limit, Some(5));
    assert_eq!(active_days, Some(3));
    assert!(!include_archived);
    assert!(!exclude_playground);
    assert!(!no_cache, "不带 --no-cache 时默认走缓存");
}

#[test]
fn session_list_parses_full_history_flags() {
    let parsed = parse(&[
        "session",
        "list",
        "--active-days",
        "0",
        "--include-archived",
        "--limit",
        "20000",
    ])
    .expect("full-history flags should parse");
    let Commands::Session {
        action: SessionAction::List {
            active_days,
            include_archived,
            limit,
            ..
        },
    } = parsed.command
    else {
        panic!("expected session list");
    };
    assert_eq!(active_days, Some(0));
    assert!(include_archived);
    assert_eq!(limit, Some(20000));
}

#[test]
fn session_list_parses_no_cache_flag() {
    let parsed = parse(&["session", "list", "--include-archived", "--no-cache"])
        .expect("no-cache flag should parse");
    let Commands::Session {
        action: SessionAction::List {
            no_cache,
            include_archived,
            ..
        },
    } = parsed.command
    else {
        panic!("expected session list");
    };
    assert!(include_archived);
    assert!(no_cache, "--no-cache 必须解析为绕过缓存");
}

#[test]
fn session_show_requires_id_and_parses_content() {
    assert!(parse(&["session", "show"]).is_err());
    let parsed = parse(&["session", "show", "opencode:abc"]).expect("show with id parses");
    let Commands::Session {
        action: SessionAction::Show { id, content },
    } = parsed.command
    else {
        panic!("expected session show");
    };
    assert_eq!(id, "opencode:abc");
    assert!(!content);
    let parsed = parse(&["session", "show", "opencode:abc", "--content"]).expect("--content parses");
    let Commands::Session {
        action: SessionAction::Show { content, .. },
    } = parsed.command
    else {
        panic!("expected session show");
    };
    assert!(content);
}

#[test]
fn session_usage_requires_id() {
    assert!(parse(&["session", "usage"]).is_err());
    let parsed = parse(&["session", "usage", "zcode:abc"]).expect("usage with id parses");
    let Commands::Session {
        action: SessionAction::Usage { id },
    } = parsed.command
    else {
        panic!("expected session usage");
    };
    assert_eq!(id, "zcode:abc");
}

#[test]
fn handoff_export_requires_id() {
    assert!(parse(&["handoff", "export"]).is_err());
    let parsed = parse(&["handoff", "export", "zcode:abc"]).expect("export with id parses");
    let Commands::Handoff {
        action: HandoffAction::Export { id },
    } = parsed.command
    else {
        panic!("expected handoff export");
    };
    assert_eq!(id, "zcode:abc");
}

#[test]
fn unknown_verb_is_rejected() {
    assert!(parse(&["session", "fly"]).is_err());
    assert!(parse(&["telemetry", "list"]).is_err());
}

#[test]
fn format_is_global_and_defaults_to_json() {
    let parsed = parse(&["session", "list"]).expect("bare list parses");
    assert_eq!(parsed.format, OutputFormat::Json);
    let parsed = parse(&["session", "list", "--format", "human"]).expect("human parses");
    assert_eq!(parsed.format, OutputFormat::Human);
    let parsed = parse(&["--format", "human", "session", "list"]).expect("global before subcommand");
    assert_eq!(parsed.format, OutputFormat::Human);
    assert!(parse(&["session", "list", "--format", "yaml"]).is_err());
}

#[test]
fn skill_verbs_parse_with_optional_dir() {
    let parsed = parse(&["skill", "install"]).expect("bare install parses");
    let Commands::Skill {
        action: SkillAction::Install { dir },
    } = parsed.command
    else {
        panic!("expected skill install");
    };
    assert!(dir.is_none());

    let parsed = parse(&["skill", "install", "--dir", "C:/tmp/skills"]).expect("--dir parses");
    let Commands::Skill {
        action: SkillAction::Install { dir },
    } = parsed.command
    else {
        panic!("expected skill install");
    };
    assert_eq!(dir.as_deref(), Some("C:/tmp/skills"));

    assert!(parse(&["skill", "list"]).is_ok());
    assert!(parse(&["skill", "status"]).is_ok());
    assert!(parse(&["skill", "status", "--dir", "X"]).is_ok());
    assert!(parse(&["skill", "print"]).is_ok());
    assert!(parse(&["skill", "remove", "--dir", "X"]).is_ok());
    assert!(parse(&["skill", "fly"]).is_err());
}

#[test]
fn skill_doc_commands_resolve_in_clap() {
    // 防文档腐烂：技能文档里写到的每条命令都必须能在 CLI 里解析到。
    // 只抽 `agentwatcher-cli <动词> [<子命令>]` 的路径，参数写法（占位符、方括号）不参与断言。
    let paths = doc_command_paths();
    assert!(
        paths.len() >= 8,
        "技能文档里的命令数量异常（{}），抽取逻辑可能失效：{paths:?}",
        paths.len()
    );
    let root = Cli::command();
    for (verb, sub) in &paths {
        let Some(top) = root.get_subcommands().find(|command| command.get_name() == verb) else {
            panic!("技能文档写了不存在的顶层命令 `{verb}`");
        };
        if let Some(sub) = sub {
            assert!(
                top.get_subcommands().any(|command| command.get_name() == sub),
                "技能文档写了不存在的子命令 `{verb} {sub}`"
            );
        }
    }
}

const SKILL_DOC: &str = include_str!("../src/bin/agentwatcher-cli/skill/SKILL.md");

fn doc_command_paths() -> Vec<(String, Option<String>)> {
    let mut paths = Vec::new();
    for span in SKILL_DOC.split('`') {
        let Some(rest) = span.trim().strip_prefix("agentwatcher-cli ") else {
            continue;
        };
        let mut parts = rest.split_whitespace();
        let Some(verb) = parts.next() else { continue };
        if verb.starts_with('-') {
            continue;
        }
        let sub = parts
            .next()
            .filter(|token| {
                !token.starts_with('-') && !token.starts_with('[') && !token.starts_with('<')
            })
            .map(str::to_string);
        paths.push((verb.to_string(), sub));
    }
    paths
}

#[test]
fn imagegen_generate_parses_prompt_and_options() {
    // prompt 必填：不给就拒绝。
    assert!(parse(&["imagegen", "generate"]).is_err());
    let parsed = parse(&["imagegen", "generate", "a red circle on white background"])
        .expect("imagegen generate parses bare prompt");
    let Commands::Imagegen {
        action:
            ImagegenAction::Generate {
                prompt,
                out,
                timeout,
            },
    } = parsed.command
    else {
        panic!("expected imagegen generate");
    };
    assert_eq!(prompt, "a red circle on white background");
    assert!(out.is_none());
    assert!(timeout.is_none());

    let parsed = parse(&[
        "imagegen",
        "generate",
        "hero image",
        "--out",
        r"F:\assets\hero.png",
        "--timeout",
        "120",
    ])
    .expect("--out/--timeout parse");
    let Commands::Imagegen {
        action: ImagegenAction::Generate { out, timeout, .. },
    } = parsed.command
    else {
        panic!("expected imagegen generate");
    };
    assert_eq!(out.as_deref(), Some(r"F:\assets\hero.png"));
    assert_eq!(timeout, Some(120));
}

#[test]
fn imagegen_help_lists_command_without_model_flag() {
    // 帮助文本必须能发现 imagegen（供宿主 agent 自助发现命令面）；
    // 同时命令面不提供 --model（生图模型由 Codex 云端自动选择，版本不绑定）。
    let help = Cli::command().render_help().to_string();
    assert!(help.contains("imagegen"), "顶层帮助应列出 imagegen：{help}");
    let imagegen_help = Cli::command()
        .find_subcommand("imagegen")
        .expect("imagegen subcommand exists")
        .clone()
        .render_help()
        .to_string();
    assert!(imagegen_help.contains("generate"));
    assert!(!imagegen_help.contains("--model"), "生图命令面不得暴露 --model：{imagegen_help}");
}

#[test]
fn suggest_generate_parses_id_and_avoid() {
    assert!(parse(&["suggest", "generate"]).is_err());
    let parsed = parse(&[
        "suggest",
        "generate",
        "codex://threads/01a1b08c-3b17-7d83-94be-a3ce08ab951a",
    ])
    .expect("suggest generate parses deep-link id");
    let Commands::Suggest {
        action: SuggestAction::Generate { id, avoid, .. },
    } = parsed.command
    else {
        panic!("expected suggest generate");
    };
    assert_eq!(id, "codex://threads/01a1b08c-3b17-7d83-94be-a3ce08ab951a");
    assert!(avoid.is_none());

    let parsed = parse(&["suggest", "generate", "zcode:abc", "--avoid", "跳过它"])
        .expect("--avoid parses");
    let Commands::Suggest {
        action: SuggestAction::Generate { avoid, .. },
    } = parsed.command
    else {
        panic!("expected suggest generate");
    };
    assert_eq!(avoid.as_deref(), Some("跳过它"));
}

#[test]
fn suggest_generate_parses_model_and_effort() {
    let parsed = parse(&[
        "suggest",
        "generate",
        "workbuddy:abc",
        "--model",
        "gpt-6.1-sol",
        "--effort",
        "high",
    ])
    .expect("--model/--effort parse");
    let Commands::Suggest {
        action:
            SuggestAction::Generate {
                id, model, effort, ..
            },
    } = parsed.command
    else {
        panic!("expected suggest generate");
    };
    assert_eq!(id, "workbuddy:abc");
    assert_eq!(model.as_deref(), Some("gpt-6.1-sol"));
    assert_eq!(effort.as_deref(), Some("high"));
}

#[test]
fn suggest_generate_parses_engine_flag() {
    let parsed = parse(&["suggest", "generate", "codex:abc", "--engine", "sample"])
        .expect("--engine parses");
    let Commands::Suggest {
        action: SuggestAction::Generate { engine, .. },
    } = parsed.command
    else {
        panic!("expected suggest generate");
    };
    assert_eq!(engine.as_deref(), Some("sample"));

    // 不传时留空，交给生成器按 auto 选路。
    let parsed = parse(&["suggest", "generate", "codex:abc"]).expect("defaults parse");
    let Commands::Suggest {
        action: SuggestAction::Generate { engine, .. },
    } = parsed.command
    else {
        panic!("expected suggest generate");
    };
    assert!(engine.is_none());
}

#[test]
fn suggest_generate_parses_via_and_session_path() {
    let parsed = parse(&[
        "suggest",
        "generate",
        "claude:01a2b3",
        "--via",
        "codex",
        "--session-path",
        r"C:\Users\me\.claude\projects\C--\01a2b3.jsonl",
    ])
    .expect("--via / --session-path parse");
    let Commands::Suggest {
        action:
            SuggestAction::Generate {
                id,
                via,
                session_path,
                ..
            },
    } = parsed.command
    else {
        panic!("expected suggest generate");
    };
    assert_eq!(id, "claude:01a2b3");
    assert_eq!(via.as_deref(), Some("codex"));
    assert_eq!(
        session_path.as_deref(),
        Some(r"C:\Users\me\.claude\projects\C--\01a2b3.jsonl")
    );

    // 默认跟随会话，不需要显式转写路径。
    let parsed = parse(&["suggest", "generate", "codex:abc"]).expect("defaults parse");
    let Commands::Suggest {
        action:
            SuggestAction::Generate {
                via, session_path, ..
            },
    } = parsed.command
    else {
        panic!("expected suggest generate");
    };
    assert!(via.is_none());
    assert!(session_path.is_none());
}

#[test]
fn suggest_prompt_requires_id() {
    assert!(parse(&["suggest", "prompt"]).is_err());
    let parsed = parse(&["suggest", "prompt", "codex:abc"]).expect("prompt parses");
    let Commands::Suggest {
        action: SuggestAction::Prompt { id },
    } = parsed.command
    else {
        panic!("expected suggest prompt");
    };
    assert_eq!(id, "codex:abc");
    assert!(parse(&["suggest", "fly"]).is_err());
}

#[test]
fn every_subcommand_has_about_text() {
    fn walk(command: &clap::Command) {
        for subcommand in command.get_subcommands() {
            let about = subcommand
                .get_about()
                .map(|text| text.to_string())
                .unwrap_or_default();
            assert!(
                !about.is_empty(),
                "subcommand {:?} must have about text",
                subcommand.get_name()
            );
            walk(subcommand);
        }
    }
    walk(&Cli::command());
}

#[test]
fn session_list_parses_exclude_playground_flag() {
    let parsed = parse(&["session", "list", "--exclude-playground"])
        .expect("exclude-playground flag should parse");
    let Commands::Session {
        action: SessionAction::List { exclude_playground, .. },
    } = parsed.command
    else {
        panic!("expected session list");
    };
    assert!(exclude_playground);

    let parsed = parse(&["session", "list"]).expect("bare list parses");
    let Commands::Session {
        action: SessionAction::List { exclude_playground, .. },
    } = parsed.command
    else {
        panic!("expected session list");
    };
    assert!(!exclude_playground);
}
