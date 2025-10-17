use anyhow::{anyhow, Result};
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Entry {
    pub kind: String,   // "receipt" | "provenance" | "unknown"
    pub digest: String, // hex blake3 of the stored JSON
}

fn ledger_dir() -> Result<std::path::PathBuf> {
    if let Ok(custom) = std::env::var("VAULTMESH_LEDGER_DIR") {
        let dir = std::path::PathBuf::from(custom);
        std::fs::create_dir_all(&dir)?;
        return Ok(dir);
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow!("no home dir"))?;
    let dir = home.join(".vaultmesh").join("ledger");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn add_json(
    _kind_hint: &str,
    bytes: &[u8],
    _commit: Option<String>,
    _git_ref: Option<String>,
) -> Result<String> {
    let digest = crate::receipt::blake3_hex(bytes);
    let path = ledger_dir()?.join(format!("{digest}.json"));
    std::fs::write(&path, bytes)?;
    Ok(digest)
}

pub fn get_json(digest: &str) -> Result<Vec<u8>> {
    let path = ledger_dir()?.join(format!("{digest}.json"));
    let data = std::fs::read(&path)?;
    Ok(data)
}

pub fn list() -> Result<Vec<Entry>> {
    let dir = ledger_dir()?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for ent in std::fs::read_dir(&dir)? {
        let ent = ent?;
        let name = ent.file_name().to_string_lossy().to_string();
        if !name.ends_with(".json") { continue; }
        let digest = name.trim_end_matches(".json").to_string();
        let bytes = std::fs::read(ent.path())?;
        let kind = classify(&bytes);
        out.push(Entry { kind, digest });
    }
    Ok(out)
}

fn classify(bytes: &[u8]) -> String {
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) {
        if crate::schema::validate_receipt(&v).is_ok() {
            return "receipt".into();
        }
        if crate::schema::validate_provenance(&v).is_ok() {
            return "provenance".into();
        }
    }
    "unknown".into()
}
