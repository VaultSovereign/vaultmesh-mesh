use serde_json::json;

// Pull the public validators from your crate
use vaultmesh::{receipt, schema};

// Helper: make a minimal valid receipt JSON
fn valid_receipt_json() -> serde_json::Value {
    json!({
        "actor": { "id": "did:key:zTest" },
        "env":   { "ci":"github_actions", "git_commit":"abc", "git_ref":"refs/heads/main" },
        "ts":    "2025-01-01T00:00:00Z",
        "subject": { "kind":"artifact", "digest":"deadbeef" }
    })
}

// Helper: minimal valid provenance JSON
fn valid_provenance_json() -> serde_json::Value {
    json!({
        "artifact": "target/release/vaultmesh",
        "artifact_hash": "deadbeef",
        "actor": { "id":"did:key:zTest" },
        "build": { "repo":"org/repo", "commit":"abc", "ref":"refs/heads/main" },
        "ci": { "name":"github_actions", "url":"https://example/run/1", "runner":"r1" },
        "ts": { "built":"2025-01-01T00:00:00Z" }
    })
}

#[test]
fn receipt_schema_rejects_missing_required_fields() {
    // Missing 'actor'
    let bad = json!({
        "env": {}, "ts":"2025-01-01T00:00:00Z",
        "subject": { "kind":"artifact", "digest":"x" }
    });
    let err = schema::validate_receipt(&bad).unwrap_err();
    assert!(err.to_string().contains("receipt schema violation"));

    // Bad ts format
    let mut v = valid_receipt_json();
    v["ts"] = json!("not-a-datetime");
    let err = schema::validate_receipt(&v).unwrap_err();
    assert!(err.to_string().contains("schema"));
}

#[test]
fn provenance_schema_rejects_missing_required_fields() {
    // Missing 'artifact_hash'
    let bad = json!({
        "artifact":"x",
        "actor":{"id":"did:key:z"},
        "build":{}, "ci":{}, "ts":{"built":"2025-01-01T00:00:00Z"}
    });
    let err = schema::validate_provenance(&bad).unwrap_err();
    assert!(err.to_string().contains("provenance schema violation"));
}

#[test]
fn valid_receipt_and_provenance_pass_validation() {
    schema::validate_receipt(&valid_receipt_json()).expect("receipt valid");
    schema::validate_provenance(&valid_provenance_json()).expect("prov valid");
}

// Bonus: the canonical hash should be order-independent for maps
#[test]
fn canonical_hash_is_stable() {
    let a = valid_receipt_json();
    let mut b = a.clone();

    // reorder env keys to simulate nondeterministic map order
    b["env"] = json!({ "git_ref":"refs/heads/main", "git_commit":"abc", "ci":"github_actions" });

    let da = receipt::hash_canonical(&a);
    let db = receipt::hash_canonical(&b);
    assert_eq!(da, db, "canonical hash must ignore key order differences");
}


