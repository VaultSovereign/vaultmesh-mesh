use std::fs;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn classify_receipt_and_provenance() {
    let dir = tempdir().unwrap();
    std::env::set_var("VAULTMESH_LEDGER_DIR", dir.path());

    // Minimal valid shapes per current schema
    let receipt = serde_json::json!({
        "actor": {"id":"did:test:actor"},
        "env": {"git_commit":"abc","git_ref":"main"},
        "ts": "2024-01-01T00:00:00Z",
        "subject": {"kind":"demo","digest":"deadbeef"}
    });
    let provenance = serde_json::json!({
        "artifact": "artifact.bin",
        "artifact_hash": "deadbeef",
        "actor": {"id":"did:test:actor"},
        "build": {},
        "ci": {},
        "ts": {"built": "2024-01-01T00:00:00Z"}
    });

    // Write raw files to simulate existing CAS
    let r_path = dir.path().join("deadbeef.json");
    let p_path = dir.path().join("beadfeed.json");
    fs::File::create(&r_path)
        .unwrap()
        .write_all(receipt.to_string().as_bytes())
        .unwrap();
    fs::File::create(&p_path)
        .unwrap()
        .write_all(provenance.to_string().as_bytes())
        .unwrap();

    let entries = vaultmesh::ledger::list().unwrap();
    assert_eq!(entries.len(), 2);
    let kinds: Vec<_> = entries.iter().map(|e| e.kind.as_str()).collect();
    assert!(kinds.contains(&"receipt"));
    assert!(kinds.contains(&"provenance"));
}

