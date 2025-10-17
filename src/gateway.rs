use axum::{
    extract::Path,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::time::Duration;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::trace::TraceLayer;

use crate::ledger;
use crate::receipt;
use crate::schema;
use crate::sync::merkle_root;
use crate::sync::policy::PEER_GUARD;

pub async fn health() -> &'static str {
    "ok"
}

/// Return stored receipt JSON by digest.
///
/// # Errors
/// Returns an error when the digest is unknown or underlying storage read fails.
pub async fn get_receipt(Path(digest): Path<String>) -> Result<String, (StatusCode, String)> {
    let data = ledger::get_json(&digest).map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    Ok(String::from_utf8_lossy(&data).into_owned())
}

/// Ingest and verify a receipt/provenance bundle pushed by a peer.
///
/// # Errors
/// Returns an error when the bundle is malformed, signature validation fails,
/// or the ledger cannot persist the data.
pub async fn post_verify(Json(body): Json<Value>) -> Result<Json<Value>, (StatusCode, String)> {
    tokio::time::timeout(Duration::from_secs(15), async move { verify_bundle(&body) })
        .await
        .map_err(|_| (StatusCode::REQUEST_TIMEOUT, "request_timeout".to_string()))?
}

fn verify_bundle(body: &Value) -> Result<Json<Value>, (StatusCode, String)> {
    let r_val = body
        .get("receipt")
        .cloned()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing receipt".to_string()))?;
    let p_val = body
        .get("provenance")
        .cloned()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing provenance".to_string()))?;

    schema::validate_receipt(&r_val).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
    schema::validate_provenance(&p_val).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let rcpt: receipt::Receipt = serde_json::from_value(r_val.clone())
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    if !PEER_GUARD.allowed(&rcpt.actor.id) {
        let msg = format!("actor not allowed: {}", rcpt.actor.id);
        return Err((StatusCode::FORBIDDEN, msg));
    }

    receipt::verify_receipt(&rcpt)
        .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()))?;

    let commit = rcpt.env.get("git_commit").cloned();
    let git_ref = rcpt.env.get("git_ref").cloned();
    let r_bytes = serde_json::to_vec(&r_val)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let p_bytes = serde_json::to_vec(&p_val)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let receipt_digest = ledger::add_json("receipt", &r_bytes, commit, git_ref)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let _prov_digest = ledger::add_json("provenance", &p_bytes, None, None)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let mut digests: Vec<String> = ledger::list()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .into_iter()
        .filter(|e| e.kind == "receipt")
        .map(|e| e.digest)
        .collect();
    digests.sort();
    let merkle = merkle_root(&digests);

    Ok(Json(json!({
        "status": "verified",
        "receipt_digest": receipt_digest,
        "merkle_root": merkle
    })))
}

/// Launch the HTTP gateway on the provided socket address.
///
/// # Errors
/// Returns an error when the listener fails to bind or the server terminates unexpectedly.
pub async fn run(addr: &str) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/v1/health", get(health))
        .route("/v1/ledger/:digest", get(get_receipt))
        .route("/v1/verify", post(post_verify))
        .layer(TraceLayer::new_for_http())
        .layer(ConcurrencyLimitLayer::new(64));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
