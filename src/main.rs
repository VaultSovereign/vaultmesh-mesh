use anyhow::{anyhow, Result};
use blake3::Hasher;
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use chrono::Utc;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::Path;

#[derive(Parser)]
#[command(name="vaultmesh")]
#[command(about="VaultMesh CLI — receipts, Merkle, verification")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd
}

#[derive(Subcommand)]
enum Cmd {
    /// Receipt operations
    Receipt {
        #[command(subcommand)]
        cmd: ReceiptCmd
    },
    /// Build a daily Merkle root from receipts in a directory
    Seal {
        /// Date (YYYY-MM-DD)
        #[arg(long)]
        date: String,
        /// Directory containing receipts (each a JSON file)
        #[arg(long, default_value = ".")]
        dir: String,
        /// Output path for root JSON
        #[arg(long)]
        out: String,
    },
    /// Anchor a receipt by computing its Merkle path from a directory
    Anchor {
        /// Path to receipt JSON
        #[arg(long)]
        receipt: String,
        /// Directory containing ALL receipts for the date
        #[arg(long)]
        dir: String,
        /// Date (YYYY-MM-DD)
        #[arg(long)]
        date: String,
        /// Output path for anchored receipt
        #[arg(long)]
        out: String,
    },
    /// Verify a receipt against a published root
    Verify {
        /// Path to receipt JSON
        #[arg(long)]
        receipt: String,
        /// Path to root JSON
        #[arg(long)]
        root: String,
        /// Perform extra checks (capability present, approvals exist)
        #[arg(long, default_value_t = false)]
        strict: bool,
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Receipt {
    id: String,
    ts: String,
    actor: Actor,
    op: Op,
    build: Build,
    env: Env,
    sign: Sign,
    leaf: String,
    merkle: Merkle,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Actor { id: String, cap: Vec<String>, sig: String }

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Op {
    kind: String, target: String,
    #[serde(default)] risk: Option<String>,
    #[serde(default)] change_window: Option<String>,
    #[serde(default)] approvals: Vec<String>,
    plan_hash: String, apply_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Build { repo: String, commit: String, binary_hash: String }

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Env { #[serde(default)] ci: Option<String>, #[serde(default)] runner: Option<String>, #[serde(default)] tf_version: Option<String>, #[serde(default)] plugins: Option<Vec<String>> }

#[derive(Serialize, Deserialize, Clone, Debug)]
struct Sign { alg: String, sig: String, pub_: String }
impl Sign { fn none() -> Self { Self { alg: "none".into(), sig: "".into(), pub_: "".into() } } }

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct Merkle { date: String, path: Vec<String>, root: String }

#[derive(Subcommand)]
enum ReceiptCmd {
    /// Emit a pre-apply receipt from a Terraform plan JSON
    Emit {
        #[arg(long)] kind: String,
        #[arg(long)] target: String,
        #[arg(long)] plan: String,
        #[arg(long)] cap: String,
        #[arg(long)] approve: String,
        #[arg(long)] repo: String,
        #[arg(long)] commit: String,
        #[arg(long, default_value = "dev-binary")] binary_hash: String,
        #[arg(long)] out: String,
    },
    /// Finalize a receipt with post-apply JSON
    Finalize {
        #[arg(long)] receipt: String,
        #[arg(long)] post: String,
        #[arg(long)] out: String,
    }
}

// ---------- Utility ----------
fn read(path: &str) -> Result<Vec<u8>> { Ok(fs::read(path)?) }
fn write(path: &str, s: &str) -> Result<()> { Ok(fs::write(path, s)?) }
fn blake3_hex(bytes: &[u8]) -> String {
    let mut h = Hasher::new();
    h.update(bytes);
    hex::encode(h.finalize().as_bytes())
}

fn hex_concat_ordered(a_hex: &str, b_hex: &str) -> Vec<u8> {
    let (a, b) = if a_hex <= b_hex { (a_hex, b_hex) } else { (b_hex, a_hex) };
    let mut bytes = Vec::with_capacity(a.len()/2 + b.len()/2);
    bytes.extend(hex::decode(a).expect("hex decode a"));
    bytes.extend(hex::decode(b).expect("hex decode b"));
    bytes
}

fn to_value<T: Serialize>(t: &T) -> Value { serde_json::to_value(t).expect("serialize") }

fn remove_leaf_and_merkle(mut v: Value) -> Value {
    if let Value::Object(ref mut m) = v {
        m.remove("leaf");
        m.remove("merkle");
    }
    v
}

fn sort_json(v: Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut b = BTreeMap::new();
            for (k, val) in map {
                b.insert(k, sort_json(val));
            }
            Value::Object(b.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(sort_json).collect()),
        _ => v
    }
}

fn canonical_payload_json<T: Serialize>(t: &T) -> String {
    let v = to_value(t);
    let v = remove_leaf_and_merkle(v);
    let v = sort_json(v);
    serde_json::to_string(&v).unwrap()
}

fn canonical_leaf_hex<T: Serialize>(t: &T) -> String {
    blake3_hex(canonical_payload_json(t).as_bytes())
}

// ---------- Merkle ----------
fn build_merkle(leaves: &[String]) -> (String, HashMap<String, Vec<String>>) {
    if leaves.is_empty() { return ("".into(), HashMap::new()); }
    let mut layer = leaves.to_vec();
    let mut paths: HashMap<String, Vec<String>> = HashMap::new();

    // Initialize paths map
    for l in &layer { paths.entry(l.clone()).or_default(); }

    let mut next_layer;
    while layer.len() > 1 {
        next_layer = Vec::new();
        for chunk in layer.chunks(2) {
            let (left, right) = if chunk.len() == 2 {
                (chunk[0].clone(), chunk[1].clone())
            } else {
                (chunk[0].clone(), chunk[0].clone()) // duplicate odd leaf
            };
            let parent_hex = blake3_hex(&hex_concat_ordered(&left, &right));
            // record sibling for paths
            paths.entry(left.clone()).or_default().push(right.clone());
            paths.entry(right.clone()).or_default().push(left.clone());
            next_layer.push(parent_hex);
        }
        layer = next_layer;
    }
    (layer[0].clone(), paths)
}

fn fold_path_to_root(leaf: &str, path: &[String]) -> String {
    let mut cur = leaf.to_string();
    for sib in path {
        cur = blake3_hex(&hex_concat_ordered(&cur, sib));
    }
    cur
}

// ---------- Main ----------
fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let cli = Cli::parse();
    match cli.cmd {
        Cmd::Receipt { cmd } => match cmd {
            ReceiptCmd::Emit { kind, target, plan, cap, approve, repo, commit, binary_hash, out } => {
                let plan_hash = blake3_hex(&read(&plan)?);
                let id = ulid::Ulid::new().to_string();
                let ts = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
                let mut rec = Receipt {
                    id, ts,
                    actor: Actor { id: "did:placeholder".into(), cap: vec![cap], sig: "".into() },
                    op: Op { kind, target, risk: None, change_window: None, approvals: vec![approve], plan_hash, apply_hash: "".into() },
                    build: Build { repo, commit, binary_hash },
                    env: Env::default(),
                    sign: Sign::none(),
                    leaf: "".into(),
                    merkle: Merkle { date: "".into(), path: vec![], root: "".into() },
                };
                rec.leaf = canonical_leaf_hex(&rec);
                write(&out, &serde_json::to_string_pretty(&rec)?)?;
                println!("EMITTED {}", out);
            }
            ReceiptCmd::Finalize { receipt, post, out } => {
                let mut rec: Receipt = serde_json::from_slice(&read(&receipt)?)?;
                rec.op.apply_hash = blake3_hex(&read(&post)?);
                rec.leaf = canonical_leaf_hex(&rec);
                write(&out, &serde_json::to_string_pretty(&rec)?)?;
                println!("FINALIZED {}", out);
            }
        },
        Cmd::Seal { date, dir, out } => {
            let mut leaves: Vec<String> = Vec::new();
            for entry in fs::read_dir(&dir)? {
                let p = entry?.path();
                if p.extension().and_then(|s| s.to_str()) == Some("json") {
                    let rec: Receipt = serde_json::from_slice(&fs::read(&p)?)?;
                    leaves.push(rec.leaf.clone());
                }
            }
            leaves.sort(); // deterministic
            let (root, _paths) = build_merkle(&leaves);
            let root_doc = json!({
                "date": date,
                "root": root,
                "count": leaves.len()
            });
            write(&out, &serde_json::to_string_pretty(&root_doc)?)?;
            println!("SEALED {}", out);
        }
        Cmd::Anchor { receipt, dir, date, out } => {
            // Build tree to compute path for the given receipt
            let rec_bytes = read(&receipt)?;
            let mut rec: Receipt = serde_json::from_slice(&rec_bytes)?;
            let mut leaves: Vec<String> = Vec::new();
            for entry in fs::read_dir(&dir)? {
                let p = entry?.path();
                if p.extension().and_then(|s| s.to_str()) == Some("json") {
                    let r: Receipt = serde_json::from_slice(&fs::read(&p)?)?;
                    leaves.push(r.leaf.clone());
                }
            }
            leaves.sort();
            let (root, paths) = build_merkle(&leaves);
            let path = paths.get(&rec.leaf).ok_or_else(|| anyhow!("leaf not found in set; ensure dir is the correct date"))?;
            rec.merkle = Merkle { date, path: path.clone(), root: root.clone() };
            write(&out, &serde_json::to_string_pretty(&rec)?)?;
            println!("ANCHORED {}", out);
        }
        Cmd::Verify { receipt, root, strict } => {
            let rec: Receipt = serde_json::from_slice(&read(&receipt)?)?;
            let computed_leaf = canonical_leaf_hex(&rec);
            if computed_leaf != rec.leaf {
                return Err(anyhow!("leaf mismatch: receipt tampered or not canonical"));
            }
            let root_doc: Value = serde_json::from_slice(&read(&root)?)?;
            let root_hex = root_doc.get("root").and_then(|v| v.as_str()).ok_or_else(|| anyhow!("invalid root.json"))?;
            let folded = fold_path_to_root(&rec.leaf, &rec.merkle.path);
            if folded != root_hex {
                return Err(anyhow!("path->root mismatch"));
            }
            if strict {
                if rec.actor.cap.is_empty() { return Err(anyhow!("strict: missing capability")); }
                // optional: require at least 1 approval for demo
                if rec.op.approvals.is_empty() { return Err(anyhow!("strict: missing approvals")); }
                if rec.op.plan_hash.is_empty() || rec.op.apply_hash.is_empty() {
                    return Err(anyhow!("strict: missing plan/apply hashes"));
                }
            }
            println!("VERIFIED ✅");
        }
    }
    Ok(())
}
