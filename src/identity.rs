use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose, Engine as _};
use dirs::home_dir;
use ed25519_dalek::{Keypair, PublicKey, SecretKey};
use getrandom::getrandom;
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

const DID_WEB_PATH_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}')
    .add(b'~');

const MULTICODEC_ED25519_PREFIX: [u8; 2] = [0xed, 0x01];

#[derive(Serialize, Deserialize)]
struct ActorKeyFile {
    alg: String,
    secret: String,
    #[serde(default)]
    did: Option<String>,
}

#[allow(clippy::missing_errors_doc)]
pub fn resolve_actor_did() -> Result<String> {
    if let Some(env_did) = env::var("VM_ACTOR_DID").ok().and_then(non_empty_trimmed) {
        return Ok(env_did);
    }

    if let Some(web_did) = did_web_from_oidc() {
        return Ok(web_did);
    }

    ensure_local_did_key()
}

fn did_web_from_oidc() -> Option<String> {
    // DID web requires an explicit domain that hosts the DID document; we do not rewrite dots.
    let domain = env::var("VM_DID_WEB_DOMAIN")
        .ok()
        .and_then(non_empty_trimmed)?;
    let jwt = env::var("VM_OIDC_JWT").ok().and_then(non_empty_trimmed)?;

    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() < 2 {
        return None;
    }

    let payload_bytes = decode_jwt_segment(parts[1])?;
    let payload: serde_json::Value = serde_json::from_slice(&payload_bytes).ok()?;

    let sub = payload
        .get("sub")
        .and_then(|v| v.as_str())
        .and_then(|s| non_empty_trimmed(s.to_string()))?;

    let encoded_sub = utf8_percent_encode(&sub, DID_WEB_PATH_SET).to_string();
    Some(compose_did_web(&domain, &["users", &encoded_sub]))
}

fn decode_jwt_segment(segment: &str) -> Option<Vec<u8>> {
    general_purpose::URL_SAFE_NO_PAD
        .decode(segment)
        .or_else(|_| general_purpose::URL_SAFE.decode(segment))
        .ok()
}

fn ensure_local_did_key() -> Result<String> {
    let path = actor_key_path()?;
    let key_dir = path
        .parent()
        .ok_or_else(|| anyhow!("invalid actor key path"))?;

    if !key_dir.exists() {
        fs::create_dir_all(key_dir).with_context(|| format!("creating {}", key_dir.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = fs::Permissions::from_mode(0o700);
            fs::set_permissions(key_dir, perms)
                .with_context(|| format!("setting permissions on {}", key_dir.display()))?;
        }
    }

    if !path.exists() {
        let (secret, did_str) = generate_actor_key()?;
        let file = ActorKeyFile {
            alg: "ed25519".into(),
            secret: general_purpose::STANDARD.encode(secret.as_bytes()),
            did: Some(did_str.clone()),
        };
        write_actor_key(&path, &file)?;
        return Ok(did_str);
    }

    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let file: ActorKeyFile = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        let secret_b64 = String::from_utf8_lossy(&bytes).trim().to_string();
        ActorKeyFile {
            alg: "ed25519".into(),
            secret: secret_b64,
            did: None,
        }
    });

    if file.alg.to_lowercase() != "ed25519" {
        return Err(anyhow!("unsupported actor key algorithm: {}", file.alg));
    }

    let secret_bytes = general_purpose::STANDARD
        .decode(file.secret.as_bytes())
        .map_err(|e| anyhow!("invalid actor key encoding: {e}"))?;
    let secret =
        SecretKey::from_bytes(&secret_bytes).map_err(|e| anyhow!("invalid actor secret: {e}"))?;
    let public = PublicKey::from(&secret);

    let did_str = file
        .did
        .filter(|d| !d.trim().is_empty())
        .unwrap_or_else(|| did_key_from_public(public.as_bytes()));

    Ok(did_str)
}

#[allow(clippy::missing_errors_doc)]
pub fn load_actor_keypair() -> Result<Keypair> {
    let path = actor_key_path()?;
    // Ensure file exists (creates if missing)
    let _ = ensure_local_did_key();
    let bytes = fs::read(&path).with_context(|| format!("reading {}", path.display()))?;
    let file: ActorKeyFile =
        serde_json::from_slice(&bytes).map_err(|e| anyhow!("bad actor.key json: {e}"))?;
    if file.alg.to_lowercase() != "ed25519" {
        return Err(anyhow!("unsupported actor key algorithm: {}", file.alg));
    }
    let secret_bytes = base64::engine::general_purpose::STANDARD
        .decode(file.secret.as_bytes())
        .map_err(|e| anyhow!("invalid actor key encoding: {e}"))?;
    let secret =
        SecretKey::from_bytes(&secret_bytes).map_err(|e| anyhow!("invalid actor secret: {e}"))?;
    let public = PublicKey::from(&secret);
    Ok(Keypair { secret, public })
}

fn write_actor_key(path: &Path, key: &ActorKeyFile) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .with_context(|| format!("opening {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o600);
        fs::set_permissions(path, perms)
            .with_context(|| format!("setting permissions on {}", path.display()))?;
    }

    let data = serde_json::to_vec_pretty(key)?;
    file.write_all(&data)
        .with_context(|| format!("writing {}", path.display()))?;

    Ok(())
}

fn actor_key_path() -> Result<PathBuf> {
    if let Some(p) = env::var("VM_ACTOR_KEY_PATH")
        .ok()
        .and_then(non_empty_trimmed)
    {
        if p == "~" {
            return home_dir()
                .map(|mut home| {
                    home.push(".vaultmesh/actor.key");
                    home
                })
                .ok_or_else(|| anyhow!("unable to determine home directory"));
        }
        if let Some(stripped) = p.strip_prefix("~/") {
            let mut home =
                home_dir().ok_or_else(|| anyhow!("unable to determine home directory"))?;
            home.push(stripped);
            return Ok(home);
        }
        return Ok(PathBuf::from(p));
    }
    let mut dir = home_dir().ok_or_else(|| anyhow!("unable to determine home directory"))?;
    dir.push(".vaultmesh");
    dir.push("actor.key");
    Ok(dir)
}

fn did_key_from_public(public_key: &[u8]) -> String {
    let mut data = Vec::with_capacity(MULTICODEC_ED25519_PREFIX.len() + public_key.len());
    data.extend_from_slice(&MULTICODEC_ED25519_PREFIX);
    data.extend_from_slice(public_key);
    let encoded = bs58::encode(data).into_string();
    format!("did:key:z{encoded}")
}

fn non_empty_trimmed<S: Into<String>>(input: S) -> Option<String> {
    let s = input.into().trim().to_string();
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// Compose `did:web:<domain>` with optional colon-delimited path segments.
fn compose_did_web(domain: &str, segments: &[&str]) -> String {
    if segments.is_empty() {
        format!("did:web:{domain}")
    } else {
        let joined = segments.join(":");
        format!("did:web:{domain}:{joined}")
    }
}

fn generate_actor_key() -> Result<(SecretKey, String)> {
    let mut seed = [0u8; 32];
    getrandom(&mut seed).map_err(|e| anyhow!("getrandom error: {e}"))?;
    let secret = SecretKey::from_bytes(&seed).map_err(|e| anyhow!("secret key error: {e}"))?;
    seed.zeroize();
    let public = PublicKey::from(&secret);
    let did_str = did_key_from_public(public.as_bytes());
    Ok((secret, did_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_did_key_generation() {
        let mut seed = [0u8; 32];
        seed[0] = 1;
        let secret = SecretKey::from_bytes(&seed).unwrap();
        let public = PublicKey::from(&secret);
        let did = did_key_from_public(public.as_bytes());
        assert!(did.starts_with("did:key:z"));
    }

    #[test]
    fn test_non_empty_trimmed() {
        assert_eq!(non_empty_trimmed(" test "), Some("test".into()));
        assert_eq!(non_empty_trimmed(""), None);
    }

    #[test]
    fn test_compose_did_web() {
        assert_eq!(compose_did_web("example.com", &[]), "did:web:example.com");
        assert_eq!(
            compose_did_web("example.com", &["users", "Alice%2FOrg"]),
            "did:web:example.com:users:Alice%2FOrg"
        );
    }

    #[test]
    fn did_web_users_sub_encoding_matrix() {
        let cases = [
            ("Alice Bob", "Alice%20Bob"),
            ("alice/bob", "alice%2Fbob"),
            ("ALICE@ORG", "ALICE%40ORG"),
            ("team:blue", "team%3Ablue"),
        ];
        for (sub, expected) in cases {
            let encoded = utf8_percent_encode(sub, DID_WEB_PATH_SET).to_string();
            assert_eq!(encoded, expected);
            let did = compose_did_web("example.com", &["users", &encoded]);
            assert_eq!(did, format!("did:web:example.com:users:{encoded}"));
        }
    }
}
