use crate::{
    cli::{ServiceAction, WorkflowAction},
    output,
};
use agentwatcher_lib::workflow::{self, service, Error, Request, Result};
use serde_json::{json, Value};
use std::io::Read;
use std::path::Path;

pub fn service_command(home: &Path, action: ServiceAction) {
    let (name, result) = match action {
        ServiceAction::Status => ("service status", Ok(service::status(home))),
        ServiceAction::Start => ("service start", service::ensure(home)),
        ServiceAction::Stop => ("service stop", service::stop(home)),
        ServiceAction::Run => {
            if let Err(error) = service::serve(home) {
                eprintln!("{}: {}", error.code, error.message);
                std::process::exit(1);
            }
            return;
        }
    };
    finish(name, result);
}
pub fn workflow_command(home: &Path, action: WorkflowAction) {
    match action {
        WorkflowAction::Plugin {
            name,
            command,
            file,
        } => {
            let result = (|| -> Result<Value> {
                let text = if file == "-" {
                    let mut text = String::new();
                    std::io::stdin()
                        .take(4 * 1024 * 1024 + 1)
                        .read_to_string(&mut text)?;
                    text
                } else {
                    std::fs::read_to_string(file)?
                };
                if text.len() > 4 * 1024 * 1024 {
                    return Err(Error::new(
                        "request_too_large",
                        "Plugin request exceeds limit",
                    ));
                }
                let request = serde_json::from_str(&text)
                    .map_err(|e| Error::new("invalid_request", e.to_string()))?;
                workflow::invoke_plugin(&name, &command, request)
            })();
            finish("workflow plugin", result);
        }
        WorkflowAction::Restore { file, confirm } => finish(
            "workflow restore",
            service::restore(home, Path::new(&file), confirm),
        ),
        WorkflowAction::Worker { operation } => {
            if let Err(error) = workflow::codex::run(home, &operation) {
                eprintln!("{}: {}", error.code, error.message);
                std::process::exit(1);
            }
        }
        WorkflowAction::Apply { file } => {
            let result = (|| -> Result<Value> {
                let mut bytes = Vec::new();
                if file == "-" {
                    std::io::stdin()
                        .take(4 * 1024 * 1024 + 1)
                        .read_to_end(&mut bytes)?;
                } else {
                    std::fs::File::open(file)?
                        .take(4 * 1024 * 1024 + 1)
                        .read_to_end(&mut bytes)?;
                }
                if bytes.len() > 4 * 1024 * 1024 {
                    return Err(Error::new("request_too_large", "Request exceeds 4 MiB"));
                }
                let request: Request = serde_json::from_slice(&bytes)
                    .map_err(|e| Error::new("invalid_request", e.to_string()))?;
                service::apply(home, &request)
            })();
            finish("workflow apply", result);
        }
        WorkflowAction::Snapshot { project } => finish(
            "workflow snapshot",
            service::snapshot(home, project.as_deref()),
        ),
        WorkflowAction::Watch {
            project,
            after,
            limit,
            wait_ms,
        } => finish(
            "workflow watch",
            service::events(home, project.as_deref(), after, limit, wait_ms),
        ),
        WorkflowAction::Next { project } => {
            finish("workflow next", service::next(home, project.as_deref()))
        }
        WorkflowAction::Guide => finish(
            "workflow guide",
            Ok(
                json!({"schemaVersion":1,"guide":include_str!("../../../workflow-guide.md"),"reflectionPrompt":workflow::REFLECTION_PROMPT}),
            ),
        ),
    }
}
fn finish(name: &str, result: Result<Value>) {
    match result {
        Ok(data) => {
            let human = format!("{}\n", serde_json::to_string_pretty(&data).unwrap());
            output::finish_success(name, data, Some(human));
        }
        Err(error) => output::finish_failure(name, &error.code, error.message),
    }
}
