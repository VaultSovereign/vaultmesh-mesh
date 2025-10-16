# VaultMesh — Operator Quickstart

## Identity Precedence

VaultMesh resolves the actor identity in this strict order:

1) `VM_ACTOR_DID`
   Absolute override. Example: `did:web:example.com:users:alice`.

2) `VM_DID_WEB_DOMAIN` + `VM_OIDC_JWT` → `did:web:<domain>:users/<sub%>`
   - `VM_DID_WEB_DOMAIN=example.com`
   - `VM_OIDC_JWT=<id_token_with_sub>` (token is used only to read `sub`, not to assert trust)
   - DID document must be published at: `https://<domain>/users/<sub%>/did.json`

3) `VM_ACTOR_KEY_PATH` (or `~/.vaultmesh/actor.key`) → `did:key:z...`
   - If the file doesn’t exist, it is created with secure perms (0700 dir / 0600 file).
   - `VM_ACTOR_KEY_PATH` supports `~` expansion.

Tip: The same public key should appear in your `did.json` when using `did:web`.

## Tooling Overrides

- `VM_TF_VERSION` — specify Terraform version without invoking `terraform version`.

## Emitting & Verifying Receipts (Glue)

```
# Emit a signed receipt for an artifact
vaultmesh glue emit --kind artifact --artifact path/to/binary > receipt.json

# Verify signature + evaluate policy (OPA must be in PATH)
vaultmesh glue verify --receipt receipt.json --policy policy/guard.rego --action apply
```

Receipts include:
- actor.id (did:web or did:key)
- env with ci, ci_url, git_commit, git_ref, and normalized terraform_version
- canonical hashing and sign.pub / sign.sig fields

## Provenance modes

- `--provenance refer` (default): signs a reference (path + blake3 hash) to provenance.json
- `--provenance embed`: embeds full provenance into receipt (larger, single blob)
- `--provenance braid`: refer + provenance stores the final receipt hash for mutual binding

## CI Gates

Recommended pipeline steps:
- `cargo fmt --all -- --check`
- `make clippy`
- `cargo deny check || true`
- Emit receipt → verify (OPA) → upload receipt + SBOM + provenance

