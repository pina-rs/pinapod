---
pinapod: fix
pinapod-derive:
  type: none
  caused_by: ["pinapod"]
---

# widen the dependency bounds

Two bounds stopped a Pinapod consumer from moving to the current Agave and Surfpool trains, even though the code compiles against both.

`solana-address` was capped at `<2.7`. Agave 4.3, which Surfpool 1.7's embedded runtime ships, resolves `solana-address` 2.7 or newer, and Cargo cannot hold two `2.x` versions at once, so the workspace could not resolve. The bound is now `>=2.2, <3.0`.

`fixed` was pinned to `=1.30.0`. `surfpool-core` 1.7 pulls `phoenix-rise-math`, which pins `fixed =1.29.0`, so the two exact pins conflict in a workspace that carries both Pinapod and Surfpool. The requirement is now `^1.29.0`, which admits the 1.29 line the current Agave/Surfpool SDK resolves, the newer 1.30 and 1.31 releases, and future 1.x patches.

The workspace lockfile pins `fixed` 1.30.0 and `solana-address` 2.9.0, so the suite exercises the newest releases inside both ranges that still build on Pinapod's Rust 1.89 minimum; `fixed` 1.31.0 requires Rust 1.93 and stays available to consumers on newer toolchains. The lowest accepted `fixed` (1.29.0) is exercised by a downstream workspace that carries Pinapod and Surfpool 1.7 together.
