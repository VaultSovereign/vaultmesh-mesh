[![CI](https://github.com/VaultSovereign/vaultmesh-mesh/actions/workflows/reuse.yml/badge.svg)](https://github.com/VaultSovereign/vaultmesh-mesh/actions)

# vaultmesh (CLI)

Implements:
- `receipt emit|finalize` — create and complete Receipt v0.1
- `seal` — compute daily root from a directory of receipts
- `anchor` — compute a Merkle path for a receipt from a receipt set
- `verify` — verify inclusion and (optionally) basic policy checks

## Build
```bash
cd vaultmesh-mesh
cargo build --release
./target/release/vaultmesh --help
```
