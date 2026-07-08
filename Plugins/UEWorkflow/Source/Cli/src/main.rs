use std::env;
use std::process::ExitCode;

use uwf_core::CommandRequest;

fn main() -> ExitCode {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let format = flag_value(&raw_args, "--format");
    let json = raw_args.iter().any(|arg| arg == "--json")
        || matches!(
            format.as_deref(),
            Some("json" | "compact-json" | "provider-json")
        );
    let request = command_request(&raw_args);
    let args = positional_args(&raw_args);

    let result = match args.as_slice() {
        [] => Ok(uwf_unrealmaster::help_json()),
        [command] if command == "doctor" => Ok(uwf_unrealmaster::doctor_json()),
        [command] if command == "modules" => Ok(uwf_unrealmaster::modules_json()),
        [command] => Err(uwf_unrealmaster::unknown_top_level_error(command)),
        [group, command] if group == "master" => {
            uwf_unrealmaster::master_command_json(command, &request)
        }
        [group, command] => uwf_unrealmaster::module_command_json(group, command, &request),
        [group, command, ..] if group == "master" => {
            uwf_unrealmaster::master_command_json(command, &request)
        }
        [group, command, ..] => uwf_unrealmaster::module_command_json(group, command, &request),
    };

    match result {
        Ok(output) => {
            if json {
                println!("{output}");
            } else {
                println!("{}", human_summary(&output));
            }
            ExitCode::SUCCESS
        }
        Err(message) => {
            if json {
                println!("{}", uwf_unrealmaster::error_json(&message));
            } else {
                eprintln!("error: {message}");
            }
            ExitCode::from(2)
        }
    }
}

fn positional_args(args: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut skip_next = false;
    for arg in args {
        if skip_next {
            skip_next = false;
            continue;
        }
        if arg == "--json" || arg == "--compact" {
            continue;
        }
        if value_flag(arg) {
            skip_next = true;
            continue;
        }
        if arg.starts_with("--") {
            continue;
        }
        result.push(arg.clone());
    }
    result
}

fn command_request(args: &[String]) -> CommandRequest {
    CommandRequest {
        goal: flag_value(args, "--goal"),
        workspace: flag_value(args, "--workspace"),
        task_id: flag_value(args, "--task-id")
            .or_else(|| flag_value(args, "--task"))
            .or_else(|| flag_value(args, "--id")),
        action: flag_value(args, "--action"),
        confirmation: flag_value(args, "--confirm").or_else(|| flag_value(args, "--confirmation")),
        provider: flag_value(args, "--provider"),
        scope: flag_value(args, "--scope"),
        project: flag_value(args, "--project"),
        primary: flag_value(args, "--primary"),
        host_root: flag_value(args, "--host-root"),
        main_project: flag_value(args, "--main-project"),
        primary_path: flag_value(args, "--primary-path"),
        plugin_dependencies: flag_value(args, "--plugin-deps"),
        compact: args.iter().any(|arg| arg == "--compact")
            || matches!(
                flag_value(args, "--format").as_deref(),
                Some("compact-json" | "provider-json")
            ),
    }
}

fn value_flag(arg: &str) -> bool {
    matches!(
        arg,
        "--goal"
            | "--workspace"
            | "--task-id"
            | "--task"
            | "--id"
            | "--branch"
            | "--action"
            | "--confirm"
            | "--confirmation"
            | "--provider"
            | "--scope"
            | "--project"
            | "--primary"
            | "--host-root"
            | "--main-project"
            | "--primary-path"
            | "--plugin-deps"
            | "--format"
    )
}

fn flag_value(args: &[String], flag: &str) -> Option<String> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].clone())
}

fn human_summary(output: &str) -> String {
    if output.contains("\"status\":\"ok\"") || output.contains("\"status\":\"ready\"") {
        "Unreal Workflow ready. Use --json for the stable machine contract.".to_string()
    } else if output.contains("\"status\":\"planned\"") {
        "Unreal Workflow plan ready. Use --json to inspect stages, risks, and confirmation boundaries.".to_string()
    } else if output.contains("\"status\":\"complete\"") {
        "Unreal Workflow command completed. Use --json to inspect records and artifacts."
            .to_string()
    } else if output.contains("\"status\":\"blocked\"") {
        "Unreal Workflow command blocked by safety rules. Use --json to inspect the required confirmation or blocked action.".to_string()
    } else if output.contains("\"status\":\"failed\"") {
        "Unreal Workflow command failed. Use --json to inspect the error evidence.".to_string()
    } else {
        output.to_string()
    }
}
