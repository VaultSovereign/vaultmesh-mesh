use blake3::Hasher;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub enum TrustLevel {
    Full,
    ReadOnly,
    Quarantine,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PeerInfo {
    pub id: String,  // did:web / did:key
    pub url: String, // https://peer/v1/ledger
    pub trust: TrustLevel,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct PeerReceiptBundle {
    pub receipt: crate::receipt::Receipt,
    pub provenance: crate::receipt::Provenance,
}

/// Extremely simple integrity fold over digests (upgradeable later).
pub fn merkle_root(digests: &[String]) -> String {
    let mut h = Hasher::new();
    for d in digests {
        h.update(d.as_bytes());
    }
    hex::encode(h.finalize().as_bytes())
}

