use super::{digest, id, Error, Result};
use rusqlite::Connection;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::{write::SimpleFileOptions, ZipArchive, ZipWriter};

pub fn create(home: &Path, db: &Connection, request: &str) -> Result<Value> {
    let directory = home.join("backups");
    std::fs::create_dir_all(&directory)?;
    let key = digest(request.as_bytes());
    let path = directory.join(format!("{key}.zip"));
    let snapshot = directory.join(format!("{}-{}.sqlite3", key, id("snapshot")));
    db.execute("VACUUM INTO ?1", [snapshot.to_string_lossy().as_ref()])?;
    let database = std::fs::read(&snapshot)?;
    let _ = std::fs::remove_file(&snapshot);
    let mut artifacts = std::collections::BTreeMap::<String, Vec<u8>>::new();
    let mut stmt = db.prepare("SELECT body FROM handoffs")?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for row in rows {
        let body: Value =
            serde_json::from_str(&row).map_err(|e| Error::new("corrupt_record", e.to_string()))?;
        for artifact in body["artifacts"].as_array().into_iter().flatten() {
            let hash = super::required(artifact, "sha256")?;
            let source = artifact["restoredPath"]
                .as_str()
                .or_else(|| artifact["path"].as_str())
                .ok_or_else(|| Error::new("invalid_artifact", "Artifact path missing"))?;
            let bytes = std::fs::read(source)
                .map_err(|e| Error::new("artifact_missing", format!("{source}: {e}")))?;
            if digest(&bytes) != hash {
                return Err(Error::new(
                    "artifact_changed",
                    format!("{source} changed after handoff"),
                ));
            }
            artifacts.insert(hash.into(), bytes);
        }
    }
    let manifest = json!({"schemaVersion":1,"databaseSha256":digest(&database),"artifacts":artifacts.iter().map(|(hash,bytes)|json!({"sha256":hash,"bytes":bytes.len()})).collect::<Vec<_>>(),"repositoriesIncluded":false});
    let temp = directory.join(format!("{key}.part"));
    let mut writer = ZipWriter::new(std::fs::File::create(&temp)?);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    writer
        .start_file("manifest.json", options)
        .map_err(zip_error)?;
    writer.write_all(manifest.to_string().as_bytes())?;
    writer
        .start_file("workflow.sqlite3", options)
        .map_err(zip_error)?;
    writer.write_all(&database)?;
    for (hash, bytes) in artifacts {
        writer
            .start_file(format!("artifacts/{hash}"), options)
            .map_err(zip_error)?;
        writer.write_all(&bytes)?;
    }
    writer.finish().map_err(zip_error)?.sync_all()?;
    if path.exists() {
        std::fs::remove_file(&temp)?;
    } else {
        std::fs::rename(&temp, &path)?;
    }
    Ok(
        json!({"path":path,"sha256":digest(&std::fs::read(&path)?),"schemaVersion":1,"externalArtifactsIncluded":true,"repositoriesIncluded":false}),
    )
}
fn zip_error(e: zip::result::ZipError) -> Error {
    Error::new("invalid_backup", e.to_string())
}
fn read(archive: &mut ZipArchive<std::fs::File>, name: &str) -> Result<Vec<u8>> {
    let file = archive.by_name(name).map_err(zip_error)?;
    if file.size() > 256 * 1024 * 1024 {
        return Err(Error::new(
            "backup_entry_too_large",
            "Backup entry exceeds 256 MiB",
        ));
    }
    let mut bytes = Vec::new();
    file.take(256 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err(Error::new(
            "backup_entry_too_large",
            "Backup entry exceeds limit",
        ));
    }
    Ok(bytes)
}
pub fn unpack(home: &Path, path: &Path) -> Result<(PathBuf, Value)> {
    if path.extension().and_then(|s| s.to_str()) != Some("zip") {
        return Ok((path.into(), json!([])));
    }
    let directory = home.join("restores").join(id("snapshot"));
    std::fs::create_dir_all(&directory)?;
    let mut archive = ZipArchive::new(std::fs::File::open(path)?).map_err(zip_error)?;
    let manifest: Value = serde_json::from_slice(&read(&mut archive, "manifest.json")?)
        .map_err(|e| Error::new("invalid_backup", e.to_string()))?;
    if manifest["schemaVersion"] != 1 {
        return Err(Error::new("schema_too_new", "Unsupported backup manifest"));
    }
    let bytes = read(&mut archive, "workflow.sqlite3")?;
    if digest(&bytes) != manifest["databaseSha256"].as_str().unwrap_or("") {
        return Err(Error::new("invalid_backup", "Database hash differs"));
    }
    let database = directory.join("workflow.sqlite3");
    std::fs::write(&database, bytes)?;
    let artifacts = directory.join("artifacts");
    std::fs::create_dir_all(&artifacts)?;
    let mut restored = Vec::new();
    for item in manifest["artifacts"].as_array().into_iter().flatten() {
        let hash = super::required(item, "sha256")?;
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::new("invalid_backup", "Invalid artifact digest"));
        }
        let bytes = read(&mut archive, &format!("artifacts/{hash}"))?;
        if digest(&bytes) != hash {
            return Err(Error::new("invalid_backup", "Artifact hash differs"));
        }
        let path = artifacts.join(hash);
        std::fs::write(&path, bytes)?;
        restored.push(json!({"sha256":hash,"path":path}));
    }
    Ok((database, json!(restored)))
}
