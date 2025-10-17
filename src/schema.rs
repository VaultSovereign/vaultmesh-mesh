#![allow(
    clippy::missing_errors_doc,
    clippy::explicit_auto_deref,
    clippy::non_std_lazy_statics
)]
use anyhow::{anyhow, Result};
use jsonschema::{Draft, JSONSchema};
use serde_json::json;
use serde_json::Value;

// Minimal schemas to guard structure; refine over time.
pub static RECEIPT_SCHEMA: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://vaultmesh.dev/schema/receipt.json",
        "type": "object",
        "required": ["actor", "env", "ts", "subject"],
        "properties": {
            "actor": {"type": "object", "required": ["id"], "properties": {"id": {"type":"string"}}},
            "env": {"type": "object"},
            "ts": {"type": "string", "format": "date-time"},
            "subject": {
                "type": "object",
                "required": ["kind", "digest"],
                "properties": {"kind": {"type":"string"}, "digest": {"type":"string"}}
            },
            "sign": {"type": ["object", "null"]},
            "provenance": {"type": ["object", "null"]},
            "provenance_ref": {"type": ["object", "null"]}
        },
        "additionalProperties": true
    })
});

pub static PROVENANCE_SCHEMA: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": "https://vaultmesh.dev/schema/provenance.json",
        "type": "object",
        "required": ["artifact", "artifact_hash", "actor", "build", "ci", "ts"],
        "properties": {
            "artifact": {"type": "string"},
            "artifact_hash": {"type": "string"},
            "actor": {"type": "object", "required": ["id"], "properties": {"id": {"type":"string"}}},
            "build": {"type": "object"},
            "ci": {"type": "object"},
            "ts": {"type": "object", "required": ["built"], "properties": {"built": {"type":"string", "format":"date-time"}}}
        },
        "additionalProperties": true
    })
});

pub fn validate_receipt(v: &Value) -> Result<()> {
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft7)
        .compile(&*RECEIPT_SCHEMA)
        .map_err(|e| anyhow!("invalid receipt schema: {e}"))?;
    if let Err(errs) = compiled.validate(v) {
        let mut msgs = Vec::new();
        for e in errs {
            msgs.push(e.to_string());
        }
        return Err(anyhow!("receipt schema violation: {}", msgs.join("; ")));
    }
    Ok(())
}

pub fn validate_provenance(v: &Value) -> Result<()> {
    let compiled = JSONSchema::options()
        .with_draft(Draft::Draft7)
        .compile(&*PROVENANCE_SCHEMA)
        .map_err(|e| anyhow!("invalid provenance schema: {e}"))?;
    if let Err(errs) = compiled.validate(v) {
        let mut msgs = Vec::new();
        for e in errs {
            msgs.push(e.to_string());
        }
        return Err(anyhow!("provenance schema violation: {}", msgs.join("; ")));
    }
    Ok(())
}
