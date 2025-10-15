# VaultMesh Keys (ed25519)

This CLI supports ed25519 signing of finalized receipts.

Generate a keypair:

- cargo build --release
- ./target/release/vaultmesh keys generate --out key.json

The generated JSON contains:

- alg: "ed25519"
- public: Base64-encoded 32-byte public key
- secret: Base64-encoded 32-byte secret key

Sign a finalized receipt:

- ./target/release/vaultmesh sign \
  --receipt rec.final.json \
  --key key.json \
  --out rec.signed.json

Verify (strict):

- ./target/release/vaultmesh verify \
  --receipt rec.signed.json \
  --root root-YYYY-MM-DD.json \
  --strict

Notes:

- Canonicalization for the leaf excludes fields: `leaf`, `merkle`, and `sign.sig` (v0.2+). The verifier accepts legacy v0.1 receipts whose leaf included `sign.sig` and prints a warning.
- Do not commit secrets. Store `key.json` in a secure secret store or GitHub Actions secret (`VAULTMESH_KEY_JSON`).

