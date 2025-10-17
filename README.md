[![CI](https://github.com/VaultSovereign/vm-mesh/actions/workflows/reuse.yml/badge.svg)](https://github.com/VaultSovereign/vm-mesh/actions)

# vaultmesh (CLI)

Implements:
- `receipt emit|finalize` — create and complete Receipt v0.1
- `seal` — compute daily root from a directory of receipts
- `anchor` — compute a Merkle path for a receipt from a receipt set
- `verify` — verify inclusion and (optionally) basic policy checks

## Build
```bash
cd vm-mesh
cargo build --release
./target/release/vaultmesh --help
```

## Gateway & Sync (Phase II)

The `vm-mesh` node exposes a minimal HTTP gateway and a local content-addressed ledger (CAS) for receipts/provenance.

### Run a local node
```bash
vaultmesh gateway --addr 127.0.0.1:8080
curl -s http://127.0.0.1:8080/v1/health   # → ok
```

API
- GET `/v1/health` → text/plain `ok`
- GET `/v1/ledger/:digest` → stored JSON (by hex BLAKE3 digest)
- POST `/v1/verify` → body `{ receipt, provenance }`
  - Validates schema + signature
  - Ingests both into CAS
  - Returns:

```json
{ "status":"verified", "receipt_digest":"<hex>", "merkle_root":"<hex>" }
```

CAS Layout
- Default path: `${HOME}/.vaultmesh/ledger/*.json` (override with `VAULTMESH_LEDGER_DIR`)
- Digest: BLAKE3 over canonical JSON bytes

### Sync CLI

Push a bundle to a peer:
```bash
vaultmesh sync push \
  --url http://127.0.0.1:8080 \
  --receipt receipt.json \
  --provenance provenance.json
```

Verify a stored receipt at a peer:
```bash
vaultmesh sync verify --url http://127.0.0.1:8080 --digest <hex>
```

Test/CI
- `vm-umbrella/.github/workflows/gateway-smoke.yml` builds, boots the gateway, and probes `/v1/health`.

Roadmap knobs
- Policy hook (OPA) on POST `/v1/verify`
- Peer allowlist/trust model at gateway
- Merkle snapshots + anchor exports
