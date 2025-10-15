# Releasing VaultMesh (CLI)

This repository ships binaries via **GitHub Releases** (no binary artifacts in git).

## Steps

1. **Bump version** (example: `0.2.1`) in `Cargo.toml`:
   ```toml
   [package]
   name = "vaultmesh"
   version = "0.2.1"
   ```

2. **Commit and tag**:
   ```bash
   git commit -am "chore(release): v0.2.1"
   git tag -a v0.2.1 -m "VaultMesh v0.2.1"
   git push && git push --tags
   ```

3. **Actions** will build binaries for **Linux, macOS, Windows**, compute **SHA256SUMS**, and attach all artifacts to the tag’s Release page.

## Consumers

- CI jobs (e.g., `polis` Daily Root Seal) should **download vaultmesh from the Release**, not build from source.
