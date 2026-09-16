// 分类学测试：只验命令解析层，不起二进制、不碰数据源。
// cli.rs 经 #[path] 直接引入，保持定义层零依赖。

#[path = "../src/bin/agentwatcher-cli/cli.rs"]
mod cli;

use clap::{CommandFactory, Parser};
use cli::{Cli, Commands, HandoffAction, OutputFormat, SessionAction};

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
