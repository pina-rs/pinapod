---
pinapod: feat
pinapod-derive: feat
---

# keep optional features no_std and prove it in CI

The runtime crate is documented as `no_std`, but the `fixed` and `solana-address` optional dependencies re-enabled their crates' default features, and `fixed`'s defaults pull in `std`. A dependent enabling either feature therefore silently left the no_std guarantee. Both member dependencies now keep `default-features = false`, matching the workspace posture already used for `wincode` and `solana-program-error`.

The `build:no-default` job now also checks every optional feature combination (`fixed`, `floats`, `solana-address`, `solana-program-error`, `wincode`) against the bare-metal `thumbv7em-none-eabihf` target, where `std` does not exist. A host-side check could never prove the promise, and a future dependency bump cannot quietly break it again. Alongside this, the fuzz suite grew failure-path coverage (rejected-commit atomicity, retry after rejection, updates over corrupted bytes, compact enum patches, four- and eight-byte length prefixes), CI-local coverage artifacts are untracked, `libfuzzer-sys` is pinned, and the devenv flake input that supplies CI tooling is pinned by branch with its trust story documented in SECURITY.md.
