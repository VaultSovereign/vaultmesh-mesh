use serde::Deserialize;
use std::{collections::HashSet, fs, path::PathBuf, sync::LazyLock};

#[derive(Debug, Deserialize)]
pub struct PeerPolicy {
    #[serde(default)]
    pub allow_ids: Vec<String>,
}

impl PeerPolicy {
    fn load_from(path: &PathBuf) -> Option<Self> {
        let data = fs::read_to_string(path).ok()?;
        toml::from_str::<Self>(&data).ok()
    }
}

pub struct PeerGuard {
    allow: Option<HashSet<String>>,
}

impl PeerGuard {
    fn new() -> Self {
        let path = std::env::var("VM_PEERS_TOML")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                let mut p = dirs::home_dir()?;
                p.push(".vaultmesh");
                p.push("peers.toml");
                Some(p)
            });

        if let Some(p) = path {
            if let Some(cfg) = PeerPolicy::load_from(&p) {
                if !cfg.allow_ids.is_empty() {
                    return Self {
                        allow: Some(cfg.allow_ids.into_iter().collect()),
                    };
                }
            }
        }

        Self { allow: None }
    }

    #[must_use]
    pub fn allowed(&self, actor_id: &str) -> bool {
        self.allow.as_ref().is_none_or(|set| set.contains(actor_id))
    }
}

pub static PEER_GUARD: LazyLock<PeerGuard> = LazyLock::new(PeerGuard::new);
