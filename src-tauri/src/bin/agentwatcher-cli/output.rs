// 输出契约层：json 模式下一条命令只往 stdout 写一个信封文档，
// human 模式的正文由命令自己渲染后经这里统一收尾。

use crate::cli::OutputFormat;
use serde::Serialize;
use std::sync::OnceLock;

static FORMAT: OnceLock<OutputFormat> = OnceLock::new();

pub fn set_format(format: OutputFormat) {
    let _ = FORMAT.set(format);
}

fn is_json() -> bool {
    FORMAT.get().map(|format| *format == OutputFormat::Json).unwrap_or(true)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvelopeError {
    code: String,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Envelope {
    command: String,
    ok: bool,
    data: Option<serde_json::Value>,
    error: Option<EnvelopeError>,
    messages: Vec<String>,
}

fn emit(envelope: Envelope) {
    let text = serde_json::to_string(&envelope).expect("envelope always serializes");
    println!("{text}");
}

/// 成功收尾：json 模式打信封，human 模式打命令渲染好的正文。
pub fn finish_success(command: &str, data: serde_json::Value, human: Option<String>) {
    if is_json() {
        emit(Envelope {
            command: command.to_string(),
            ok: true,
            data: Some(data),
            error: None,
            messages: Vec::new(),
        });
    } else if let Some(text) = human {
        print!("{text}");
    }
}

/// 失败收尾：json 模式把错误装进信封后退出码 1，human 模式打 stderr 后退出码 1。
pub fn finish_failure(command: &str, code: &str, message: String) -> ! {
    if is_json() {
        emit(Envelope {
            command: command.to_string(),
            ok: false,
            data: None,
            error: Some(EnvelopeError {
                code: code.to_string(),
                message,
            }),
            messages: Vec::new(),
        });
    } else {
        eprintln!("{command} 失败（{code}）：{message}");
    }
    std::process::exit(1);
}
