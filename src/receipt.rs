use crate::env_meta::collect_env_metadata;
use crate::identity::resolve_actor_did;
use anyhow::{anyhow, Result};
use base64::Engine as _;
use blake3::Hasher;
use ed25519_dalek::{Keypair, PublicKey, Signature, Signer, Verifier};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
pub struct Receipt {
    pub actor: Actor,
    pub env: BTreeMap<String, String>,
    pub ts: String,
    pub subject: Subject,
    pub sign: Option<Sign>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<Provenance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance_ref: Option<ProvenanceRef>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Actor {
    pub id: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Subject {
    pub kind: String,
    pub digest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Sign {
    #[serde(rename = "pub")]
    pub_: String,
    #[serde(rename = "sig")]
    pub signature: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alg: Option<String>,
}

pub fn build_receipt(subject: Subject) -> Result<Receipt> {
    let actor = Actor {
        id: resolve_actor_did()?,
    };
    let env = collect_env_metadata().entries;
    let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    Ok(Receipt {
        actor,
        env,
        ts,
        subject,
        sign: None,
        provenance: None,
        provenance_ref: None,
    })
}

pub fn hash_canonical(json: &Value) -> String {
    // stable: serde_json already orders BTreeMap deterministically; re-serialize and hash
    let mut hasher = Hasher::new();
    hasher.update(serde_json::to_string(json).unwrap().as_bytes());
    let out = hasher.finalize();
    hex::encode(out.as_bytes())
}

pub fn sign_receipt(mut r: Receipt, kp: &Keypair) -> Result<Receipt> {
    let mut v = serde_json::to_value(&r)?;
    if let Value::Object(ref mut m) = v {
        m.remove("sign");
    }
    let digest_hex = hash_canonical(&v);
    let sig: Signature = kp.sign(digest_hex.as_bytes());
    let pub_b64 = base64::engine::general_purpose::STANDARD.encode(kp.public.as_bytes());
    let sig_b64 = base64::engine::general_purpose::STANDARD.encode(sig.to_bytes());
    r.sign = Some(Sign {
        pub_: pub_b64,
        signature: sig_b64,
        alg: Some("ed25519".to_string()),
    });
    Ok(r)
}

pub fn verify_receipt(r: &Receipt) -> Result<()> {
    let sign = r.sign.as_ref().ok_or_else(|| anyhow!("missing sign"))?;
    let pub_bytes = base64::engine::general_purpose::STANDARD
        .decode(sign.pub_.as_bytes())
        .map_err(|e| anyhow!("bad public b64: {e}"))?;
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(sign.signature.as_bytes())
        .map_err(|e| anyhow!("bad signature b64: {e}"))?;
    let pk = PublicKey::from_bytes(&pub_bytes).map_err(|e| anyhow!("bad public: {e}"))?;
    let sig = Signature::from_bytes(&sig_bytes).map_err(|e| anyhow!("bad signature: {e}"))?;

    let mut v = serde_json::to_value(r)?;
    if let Value::Object(ref mut m) = v {
        m.remove("sign");
    }
    let digest_hex = hash_canonical(&v);
    pk.verify(digest_hex.as_bytes(), &sig)
        .map_err(|e| anyhow!("signature verify failed: {e}"))
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ProvenanceRef {
    pub path: String,
    pub digest: String,
}

/// Minimal supply-chain provenance emitted alongside the receipt.
/// SLSA-lean: focuses on binding artifact -> commit/ref/CI and actor id.
#[derive(Serialize, Deserialize, Clone)]
pub struct Provenance {
    pub artifact: String,
    pub artifact_hash: String,
    pub actor: Actor,
    pub build: Build,
    pub ci: CiInfo,
    pub ts: TsInfo,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct Build {
    pub repo: Option<String>,   // e.g., "org/repo"
    pub commit: Option<String>, // git SHA
    pub r#ref: Option<String>,  // refs/heads/main
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct CiInfo {
    pub name: Option<String>,   // github_actions, gitlab_ci, etc.
    pub url: Option<String>,    // ci_url
    pub runner: Option<String>, // runner hostname/label
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TsInfo {
    pub built: String, // RFC3339
}

pub fn build_provenance(
    artifact_path: &Path,
    artifact_hash_hex: &str,
    actor: &Actor,
    env: &BTreeMap<String, String>,
) -> Provenance {
    // Pick best-effort repo across CI vendors.
    let repo = env
        .get("github_repository")
        .or_else(|| env.get("gitlab_project"))
        .or_else(|| env.get("circle_project"))
        .or_else(|| env.get("buildkite_pipeline"))
        .cloned();
    let commit = env
        .get("git_commit")
        .cloned()
        .or_else(|| env.get("github_sha").cloned())
        .or_else(|| env.get("gitlab_sha").cloned())
        .or_else(|| env.get("circle_sha").cloned())
        .or_else(|| env.get("buildkite_commit").cloned());
    let r#ref = env.get("git_ref").cloned();
    let ci = CiInfo {
        name: env
            .get("ci")
            .cloned()
            .or_else(|| env.get("ci_name").cloned()),
        url: env.get("ci_url").cloned(),
        runner: env.get("runner").cloned(),
    };
    let ts = TsInfo {
        built: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    };
    Provenance {
        artifact: artifact_path.to_string_lossy().to_string(),
        artifact_hash: artifact_hash_hex.to_string(),
        actor: actor.clone(),
        build: Build {
            repo,
            commit,
            r#ref,
        },
        ci,
        ts,
    }
}

pub fn canonical_json_bytes<T: Serialize>(v: &T) -> Vec<u8> {
    serde_json::to_vec(v).expect("serialize")
}

pub fn blake3_hex(data: &[u8]) -> String {
    let mut h = Hasher::new();
    h.update(data);
    hex::encode(h.finalize().as_bytes())
}
