use super::{digest, Error, Result};
use serde_json::{json, Value};
use std::path::Path;

pub fn inspect(path: &Path) -> Result<Value> {
    let path = std::fs::canonicalize(path)?;
    if !path.is_file() || path.extension().and_then(|s| s.to_str()) != Some("md") {
        return Err(Error::new(
            "invalid_legacy_source",
            "Select a concrete Markdown task record",
        ));
    }
    let bytes = std::fs::read(&path)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(Error::new(
            "source_too_large",
            "Legacy record exceeds 4 MiB",
        ));
    }
    let body = String::from_utf8(bytes.clone())
        .map_err(|e| Error::new("invalid_legacy_source", e.to_string()))?;
    let mut fields = serde_json::Map::new();
    if body.starts_with("---") {
        for line in body.lines().skip(1).take_while(|line| line.trim() != "---") {
            if !line.starts_with(' ') {
                if let Some((key, value)) = line.split_once(':') {
                    if [
                        "id",
                        "title",
                        "status",
                        "home_repository",
                        "branch_or_pr",
                        "revision",
                    ]
                    .contains(&key)
                    {
                        fields.insert(
                            key.into(),
                            json!(value.trim().trim_matches('"').trim_matches('\'')),
                        );
                    }
                }
            }
        }
    }
    if fields.get("id").and_then(Value::as_str).is_none() {
        return Err(Error::new(
            "invalid_legacy_source",
            "Legacy task record must declare its stable ID",
        ));
    }
    Ok(
        json!({"path":path,"sha256":digest(&bytes),"metadata":fields,"body":body,"readOnly":true,"currentEvidenceAccepted":false}),
    )
}
